/**
 * ai2rules-gate — DeepSeek Harness (dsh) native-tool governance adapter.
 * ai2rules issue #55; DECISIONS D24 / D34 / D35 / D36 / D72.
 *
 * A native cordis plugin that subscribes to dsh's authoritative
 * `system-prompt/assemble` waterfall AND its `tools/pre-execute` waterfall.
 * Discovery is shaped by `harness project`; every invocation is decided by
 * `harness gate` (D24/D72) — the same compiled world governs both boundaries and
 * the same engine governs Claude Code
 * (`cc-hook`), OpenCode, and the MCP gateway. No policy, taint, or command
 * classification logic lives here; this is transport + session-state plumbing
 * only (the one-kernel / thin-adapter rule, D35). The plugin sends the RAW dsh
 * tool name (`exec.name`) — the kernel classifies bash shapes from the world
 * manifest's `command_classes` (D36) and returns the effective action.
 *
 * Verdict → dsh PreToolDecision (the seam's native allow/deny/ask channel):
 *
 *   ALLOW    -> { kind: 'allow' }               dsh runs the tool
 *   DENY     -> { kind: 'deny', reason }         binding rejection before effect
 *   ASK      -> { kind: 'ask',  reason }         dsh's ctx.approval one-shot prompt
 *   ABSENT   -> omitted from prompt assembly; stale/direct invocation still denied
 *   REPLAN   -> { kind: 'deny', reason }         best-effort: no faithful pre-execute REPLAN primitive
 *   <unknown>-> { kind: 'deny', reason }         unmappable verdict -> fail closed, ABI mismatch noted
 *
 * FAIL-CLOSED (issue #55, the whole point of this spike). Once this plugin is
 * MOUNTED it is "engaged": every path that cannot produce a trustworthy ALLOW
 * DENIES the governed effect. That includes a missing/!executable harness
 * binary, a non-zero gate exit (malformed/unreadable manifest = 2, internal = 1),
 * unparseable stdout, an unknown decision, and a taint/usage sidecar that cannot
 * be persisted. This is the deliberate INVERSE of the OpenCode bootstrap shim's
 * fail-open: a bootstrap shim that has not yet engaged the gate may fall through
 * (the session was never governed), but an engaged adapter must not silently
 * allow a governed effect on gate unavailability.
 *
 *   The only sanctioned bypass is the explicit operator switch AI2RULES_DISABLE=1,
 *   which means "do not govern this session at all" — the not-activated case,
 *   distinct from an engaged-but-broken gate.
 *
 * Monotonic session taint AND budget usage are persisted in a JSON sidecar
 * (AI2RULES_STATE, default <cwd>/.dsh-ai2rules-state.json). Taint never lowers;
 * usage is the kernel's post-call counters carried to the next call.
 *
 * Env:
 *   AI2RULES_HARNESS         absolute path to the `harness` binary (required to govern)
 *   AI2RULES_WORLD           WorldManifest path (default ./deepseek-world.yaml)
 *   AI2RULES_STATE           taint+usage sidecar path (default ./.dsh-ai2rules-state.json)
 *   AI2RULES_MODE            "interactive" (default) | "background" -> context.mode
 *                            (the kernel collapses ASK->DENY in background)
 *   AI2RULES_SOURCE_CHANNEL  provenance channel name in the world's channels: table
 *                            (default "user_cli")
 *   AI2RULES_DISABLE         "1" to bypass governance entirely (the not-activated case)
 *
 * This module intentionally imports nothing from dsh: the `apply(ctx)` contract
 * is structural, so the plugin compiles and mounts against any cordis Context
 * that exposes the two documented waterfalls. That keeps the adapter a leaf
 * with no dsh package/version dependency.
 */
import { spawnSync } from 'node:child_process'
import { accessSync, constants, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, isAbsolute, join } from 'node:path'

/** dsh's pre-dispatch decision (structural mirror of dsh-tools' PreToolDecision). */
type PreToolDecision =
  | { kind: 'allow' }
  | { kind: 'deny'; reason: string }
  | { kind: 'ask'; reason?: string }

/** The subset of a dsh ToolExecution this adapter reads. */
interface ToolExecutionLike {
  readonly name: string
  readonly arguments: unknown
  readonly agent?: { id?: string; session?: { id?: string; header?: { cwd?: string } } }
}

interface ToolSchemaLike {
  readonly name: string
  readonly description?: string
  readonly parameters: unknown
}

interface PromptAssemblyLike {
  sections: Array<{ name: string; text: string }>
  tools: ToolSchemaLike[]
  [key: string]: unknown
}

interface AssembleContextLike { readonly scope?: unknown }

/** The minimal cordis Context surface the adapter binds to. */
interface ContextLike {
  on(event: 'tools/pre-execute', handler: (exec: ToolExecutionLike, next: () => Promise<PreToolDecision>) => Promise<PreToolDecision>): unknown
  on(event: 'system-prompt/assemble', handler: (assembly: PromptAssemblyLike, context: AssembleContextLike, next: () => Promise<PromptAssemblyLike>) => Promise<PromptAssemblyLike>): unknown
}

interface GateResponse {
  decision?: string
  action?: string
  rule?: string | null
  reason?: string
  context?: { taint?: string; usage?: Record<string, number> }
  approval?: { token?: string; required?: boolean } | null
  manifest_hash?: string
}

interface ProjectionResponse {
  v?: number
  tools?: ToolSchemaLike[]
  absent?: string[]
  manifest_hash?: string
  schema_hash?: string
}

interface Sidecar {
  taint: Record<string, 'clean' | 'tainted'>
  usage: Record<string, Record<string, number>>
  cause: Record<string, string>
}

const DISABLED = () => process.env.AI2RULES_DISABLE === '1'

function isExecutable(path: string | undefined): path is string {
  if (path === undefined || !isAbsolute(path)) return false
  try { accessSync(path, constants.X_OK); return true } catch { return false }
}

function statePath(): string {
  return process.env.AI2RULES_STATE ?? join(process.cwd(), '.dsh-ai2rules-state.json')
}

function worldPath(): string {
  return process.env.AI2RULES_WORLD ?? join(process.cwd(), 'deepseek-world.yaml')
}

function executionMode(): 'interactive' | 'background' {
  return process.env.AI2RULES_MODE === 'background' ? 'background' : 'interactive'
}

function sourceChannel(): string {
  return process.env.AI2RULES_SOURCE_CHANNEL ?? 'user_cli'
}

function scopeSessionId(scope: unknown): string {
  if (scope !== null && typeof scope === 'object') {
    const value = scope as { id?: unknown; session?: { id?: unknown } }
    if (typeof value.session?.id === 'string') return value.session.id
    if (typeof value.id === 'string') return value.id
  }
  return 'dsh-session'
}

function failClosedAssembly(assembly: PromptAssemblyLike, sessionId: string, reason: string): PromptAssemblyLike {
  evidence(`session=${sessionId} discovery -> EMPTY (fail-closed): ${reason}`)
  return {
    ...assembly,
    tools: [],
    // A code-mode SDK is another discovery surface. If projection cannot prove
    // it, remove the transport instructions as well as the wire tool schemas.
    sections: assembly.sections.filter(section => section.name !== 'tools:sdk' && section.name !== 'tools:code-only'),
  }
}

function loadSidecar(): Sidecar {
  try {
    const raw = JSON.parse(readFileSync(statePath(), 'utf8')) as Partial<Sidecar>
    return { taint: raw.taint ?? {}, usage: raw.usage ?? {}, cause: raw.cause ?? {} }
  } catch {
    return { taint: {}, usage: {}, cause: {} }
  }
}

/** Persist the sidecar; returns false if it could not be written (caller fails closed). */
function saveSidecar(state: Sidecar): boolean {
  try {
    const p = statePath()
    mkdirSync(dirname(p), { recursive: true })
    writeFileSync(p, JSON.stringify(state))
    return true
  } catch {
    return false
  }
}

/**
 * Emit one correlation-evidence line: session/tool -> request -> verdict ->
 * decision. Write-through to the host process stdout (not `ctx.logger`, whose
 * cordis proxy is callable-only) so a terminal deployment shows the trail; a
 * host that wants structured audit can grep the `[ai2rules-gate]` tag.
 */
function evidence(line: string): void {
  // eslint-disable-next-line no-console
  console.log(`[ai2rules-gate] ${line}`)
}

/**
 * The dsh cordis plugin. Namespace form (named exports), so a composition mounts
 * it with `ctx.plugin(Ai2rulesGate, config?)` or via a `cordis.yml` entry.
 */
export const name = 'ai2rules-gate'
export const inject: string[] = ['tools', 'systemPrompt']

export function apply(ctx: ContextLike): void {
  ctx.on('system-prompt/assemble', async (assembly, context, next): Promise<PromptAssemblyLike> => {
    const assembled = await next()
    if (DISABLED()) return assembled

    const sessionId = scopeSessionId(context.scope)
    const harness = process.env.AI2RULES_HARNESS
    if (!isExecutable(harness)) {
      return failClosedAssembly(assembled, sessionId,
        'governance projector unavailable (harness binary missing or not executable)')
    }
    const run = spawnSync(harness, ['project', '--world', worldPath()], {
      input: JSON.stringify({
        v: 1,
        context: { source_channel: sourceChannel() },
        tools: assembled.tools,
      }),
      encoding: 'utf8',
      timeout: 30_000,
    })
    if (run.error !== undefined) return failClosedAssembly(assembled, sessionId, `project process failed to start: ${run.error.message}`)
    if (run.status !== 0) return failClosedAssembly(assembled, sessionId, `project failed (exit ${run.status}): ${(run.stderr || '').trim()}`)

    let projected: ProjectionResponse
    try { projected = JSON.parse(run.stdout) as ProjectionResponse } catch {
      return failClosedAssembly(assembled, sessionId, 'project returned unparseable output')
    }
    if (projected.v !== 1 || !Array.isArray(projected.tools) || !Array.isArray(projected.absent)
      || typeof projected.manifest_hash !== 'string' || typeof projected.schema_hash !== 'string') {
      return failClosedAssembly(assembled, sessionId, 'project returned an invalid projection response')
    }
    const offered = new Set(assembled.tools.map(tool => tool.name))
    if (projected.tools.some(tool => typeof tool?.name !== 'string' || !offered.has(tool.name))) {
      return failClosedAssembly(assembled, sessionId, 'project returned a tool that was not offered by this assembly')
    }

    const absent = new Set(projected.absent)
    let sections = assembled.sections
    // `tools:sdk` is an unstructured second catalog in code/both presentation.
    // This adapter never parses or rewrites host prompt text. If the compiled
    // world does not expose the reserved run_code transport, remove its SDK and
    // collapse instruction. Native tools in `both` remain available normally;
    // code-only mode therefore narrows to an empty surface, fail closed.
    if (absent.has('run_code')) {
      sections = sections.filter(section => section.name !== 'tools:sdk' && section.name !== 'tools:code-only')
    }
    evidence(`session=${sessionId} discovery offered=${assembled.tools.length} visible=${projected.tools.length} absent=${projected.absent.length} manifest=${projected.manifest_hash} schema=${projected.schema_hash.slice(0, 12)}`)
    return { ...assembled, tools: projected.tools, sections }
  })

  ctx.on('tools/pre-execute', async (exec, next): Promise<PreToolDecision> => {
    // The not-activated case: operator opted this session out of governance.
    if (DISABLED()) return next()

    const sessionId = exec.agent?.session?.id ?? 'dsh-session'
    const deny = (reason: string): PreToolDecision => {
      evidence(`session=${sessionId} tool=${exec.name} -> DENY (fail-closed): ${reason}`)
      return { kind: 'deny', reason: `[ai2rules] ${reason}` }
    }

    // ENGAGED but the gate binary is unavailable -> fail closed (issue #55).
    const harness = process.env.AI2RULES_HARNESS
    if (!isExecutable(harness)) {
      return deny('governance gate unavailable (harness binary missing or not executable); '
        + 'set AI2RULES_HARNESS, or AI2RULES_DISABLE=1 to run ungoverned')
    }

    const sidecar = loadSidecar()
    const tainted = sidecar.taint[sessionId] === 'tainted'
    const usage = sidecar.usage[sessionId] ?? { commands_run: 0, network_calls: 0, file_writes: 0, tokens_used: 0 }

    const req = {
      v: 1,
      tool: exec.name, // RAW host tool name — classification is the kernel's job (D36)
      arguments: (exec.arguments ?? {}) as Record<string, unknown>,
      path: null,
      context: {
        session_id: sessionId,
        mode: executionMode(),
        taint: tainted ? 'tainted' : 'clean',
        source_channel: sourceChannel(),
        approval_token: null,
        usage,
      },
    }

    // Run the pure kernel out-of-process (D34). spawnSync keeps the waterfall
    // handler simple; the gate is a short-lived single-shot process.
    const run = spawnSync(harness, ['gate', '--world', worldPath()], {
      input: JSON.stringify(req),
      encoding: 'utf8',
      timeout: 30_000,
    })

    if (run.error !== undefined) return deny(`gate process failed to start: ${run.error.message}`)
    if (run.status === 2) return deny(`gate rejected the world/request (exit 2): ${(run.stderr || '').trim()}`)
    if (run.status !== 0) return deny(`gate internal error (exit ${run.status}): ${(run.stderr || '').trim()}`)

    let verdict: GateResponse
    try {
      verdict = JSON.parse(run.stdout) as GateResponse
    } catch {
      return deny('gate returned unparseable output')
    }

    // Persist the kernel-computed monotonic taint + post-call usage BEFORE
    // acting on an ALLOW: a counter that silently fails to persist is a budget
    // that silently stops counting, so a persistence failure fails the call
    // closed (D59 applied to budgets; ABI §6 steps 5-6).
    const postTaint = verdict.context?.taint === 'tainted' ? 'tainted' : (tainted ? 'tainted' : 'clean')
    const postUsage = verdict.context?.usage ?? usage
    const taintChanged = postTaint === 'tainted' && !tainted
    const usageChanged = JSON.stringify(postUsage) !== JSON.stringify(usage)
    if (taintChanged || usageChanged) {
      sidecar.taint[sessionId] = postTaint
      sidecar.usage[sessionId] = postUsage
      if (taintChanged) sidecar.cause[sessionId] = `tainted by ${exec.name} (${verdict.action ?? exec.name})`
      if (!saveSidecar(sidecar)) {
        return deny('could not persist session taint/usage sidecar; refusing the call it would have charged')
      }
    }

    const action = verdict.action ?? exec.name
    const hash = verdict.manifest_hash ?? '?'
    const tail = `action=${action} rule=${verdict.rule ?? 'none'} manifest=${hash}`

    switch (verdict.decision) {
      case 'ALLOW':
        evidence(`session=${sessionId} tool=${exec.name} -> ALLOW ${tail}`)
        return next()
      case 'DENY':
        evidence(`session=${sessionId} tool=${exec.name} -> DENY ${tail}`)
        return { kind: 'deny', reason: `[ai2rules] DENY (${action}): ${verdict.reason ?? 'blocked by governance'}` }
      case 'ASK':
        evidence(`session=${sessionId} tool=${exec.name} -> ASK ${tail}`)
        return { kind: 'ask', reason: `[ai2rules] approval required (${action}): ${verdict.reason ?? ''}`.trim() }
      case 'ABSENT':
        // A stale assembly or direct/sub-call may still name a now-revoked tool;
        // discovery never grants authority, so invocation remains binding.
        evidence(`session=${sessionId} tool=${exec.name} -> ABSENT ${tail}`)
        return { kind: 'deny', reason: `[ai2rules] ABSENT (${action}): ${verdict.reason ?? 'action is not in this world'}` }
      case 'REPLAN':
        // dsh's pre-execute seam has allow/deny/ask only — no faithful
        // non-effecting "continue without executing" primitive. Deny is the
        // safe best-effort; the mismatch is documented in the runbook (#55).
        evidence(`session=${sessionId} tool=${exec.name} -> REPLAN->deny ${tail}`)
        return { kind: 'deny', reason: `[ai2rules] REPLAN (${action}): ${verdict.reason ?? 'replan requested'} (no faithful dsh REPLAN primitive; denied)` }
      default:
        return deny(`unmappable gate decision ${JSON.stringify(verdict.decision)} (ABI/host mismatch)`)
    }
  })
}

export default { name, inject, apply }
