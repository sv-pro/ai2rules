/**
 * ai2rules execution-seam probes against DeepSeek Harness.
 * Pinned commit: 141eb6fef83422698aef7a981029e843e8161534 (dsh-0.1.0-rc.8).
 *
 * GENUINE end-to-end probes through two distinct routes:
 *   E1-E4  ordinary filesystem tool — mounts the REAL SandboxedFileSystem
 *          (dsh-fs-sandbox = LocalFileSystem + the shared writableRoots fence)
 *          with the REAL tool-fs suite and performs actual disk writes; proves
 *          at RUNTIME that the fs tool is fenced to the session workspace and
 *          cannot reach a path standing in for $DSH_HOME under
 *          workspace-write / read-only.
 *   E5     Code Mode serialized sub-call — a run_code sub-call to a guarded
 *          tool is still judged by the monotonic guard; Code Mode is not a
 *          route around the pipeline.
 *
 * To run: place beside the runner's own tests at commit 141eb6f; it imports
 * only packages already in the dsh workspace. `npx vitest run <file>`.
 */
import { mkdtempSync, mkdirSync, readFileSync, realpathSync, existsSync, rmSync } from 'node:fs'
import { tmpdir, homedir } from 'node:os'
import { join, resolve } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { Context } from '@deepseek-ai/cordis'
import { CallId } from '@deepseek-ai/dsh-llm'
import SystemPrompt from '@deepseek-ai/dsh-system-prompt'
import ToolRuntime, { RUN_CODE_NAME, defineTool } from '@deepseek-ai/dsh-tools'
import type { ToolExecutionResult } from '@deepseek-ai/dsh-tools'
import SandboxedFileSystem from '@deepseek-ai/dsh-fs-sandbox'
import SandboxPolicyService from '@deepseek-ai/dsh-sandbox-policy'
import * as FsPolicy from '@deepseek-ai/dsh-fs-observation-policy'
import * as ToolFs from '@deepseek-ai/dsh-tool-fs'
import { CodeRuntime } from '@deepseek-ai/dsh-code-runtime'
import type { CodeRunRequest, CodeRunResult } from '@deepseek-ai/dsh-code-runtime'

let callCounter = 0

/** A fake agent whose session carries a cwd + a foldable event log (the write tool folds sandbox mode). */
function seamAgent(cwd: string): object {
  const events: Array<{ type: string; data?: Record<string, unknown> }> = [{ type: 'turn/start' }]
  return {
    id: 'seam-agent',
    session: {
      header: { version: 0, id: 'seam-session', createdAt: 0, cwd },
      events,
      append: (type: string, data: Record<string, unknown>) => { events.push({ type, data }) },
    },
  }
}

function call(ctx: Context, name: string, args: unknown, cwd: string): Promise<ToolExecutionResult> {
  return ctx.tools.execute({
    signal: new AbortController().signal,
    callId: CallId(`seam-${++callCounter}`),
    name,
    arguments: args,
    agent: seamAgent(cwd) as never,
  })
}

function text(r: ToolExecutionResult): string {
  return r.content.filter(b => b.type === 'text').map(b => (b as { text: string }).text).join('')
}

const dirs: string[] = []

// Base the fixture tree OUTSIDE the OS temp root, because writableRoots always
// includes /tmp and os.tmpdir(); a $DSH_HOME stand-in under tmp would be
// legitimately writable and prove nothing. os.homedir() is where $DSH_HOME
// (`~/.dsh`) actually lives, so it is the faithful stand-in and is outside the
// temp roots. realpath so containment compares canonical paths.
function fixtureBase(): { workspace: string; governorHome: string } {
  const base = realpathSync(mkdtempSync(join(homedir(), 'ai2r-seam-')))
  dirs.push(base)
  const workspace = join(base, 'workspace')
  const governorHome = join(base, 'dsh-home') // sibling of workspace, outside it and outside /tmp
  mkdirSync(workspace, { recursive: true })
  mkdirSync(governorHome, { recursive: true })
  return { workspace, governorHome }
}
afterEach(() => { for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true }) })

/** Mount the real sandboxed local FS + tool-fs at a given deployment default mode. */
async function seamTree(mode: 'workspace-write' | 'read-only', fallbackRoot: string) {
  const ctx = new Context()
  await ctx.plugin(SystemPrompt)
  await ctx.plugin(ToolRuntime)
  await ctx.plugin(SandboxPolicyService, { mode, workspaceRoot: fallbackRoot })
  await ctx.plugin(SandboxedFileSystem, { cwd: fallbackRoot })
  await ctx.plugin(FsPolicy)
  await ctx.plugin(ToolFs)
  return ctx
}

describe('E1-E4 — mandatory execution seam: ordinary filesystem tool (genuine disk effect)', () => {
  it('E1: workspace-write ALLOWS a write inside the session workspace (file really appears on disk)', async () => {
    const { workspace } = fixtureBase()
    const ctx = await seamTree('workspace-write', workspace)
    const target = join(workspace, 'inside.txt')

    const result = await call(ctx, 'write', { file_path: target, content: 'hello' }, workspace)
    expect(result.isError).toBeFalsy()
    expect(existsSync(target)).toBe(true)          // genuine effect: byte hit the disk
    expect(readFileSync(target, 'utf8')).toBe('hello')
    await ctx.fiber.dispose()
  })

  it('E2: workspace-write DENIES a write to a path standing in for $DSH_HOME (no file created)', async () => {
    const { workspace, governorHome } = fixtureBase() // governorHome stands in for ~/.dsh, outside workspace and /tmp
    const ctx = await seamTree('workspace-write', workspace)
    const target = join(governorHome, 'cordis.patch.yml')

    const result = await call(ctx, 'write', { file_path: target, content: '- { disable: approval }' }, workspace)
    expect(result.isError).toBe(true)
    expect(text(result)).toContain('[sandbox: file access denied under workspace-write mode]')
    expect(existsSync(target)).toBe(false)          // genuine non-effect: governor config untouched
    await ctx.fiber.dispose()
  })

  it('E3: read-only DENIES every write, even inside the workspace', async () => {
    const { workspace } = fixtureBase()
    const ctx = await seamTree('read-only', workspace)
    const target = join(workspace, 'inside.txt')

    const result = await call(ctx, 'write', { file_path: target, content: 'x' }, workspace)
    expect(result.isError).toBe(true)
    expect(text(result)).toContain('[sandbox: file access denied under read-only mode]')
    expect(existsSync(target)).toBe(false)
    await ctx.fiber.dispose()
  })

  it('E4: /tmp itself is writable under workspace-write (matches writableRoots = [workspaceRoot, /tmp, tmpdir])', async () => {
    const { workspace } = fixtureBase()
    const ctx = await seamTree('workspace-write', workspace)
    const target = join(resolve(tmpdir()), `ai2r-tmp-write-${Date.now()}.txt`)
    const result = await call(ctx, 'write', { file_path: target, content: 'ok' }, workspace)
    try {
      expect(result.isError).toBeFalsy()
      expect(existsSync(target)).toBe(true)
    } finally {
      if (existsSync(target)) rmSync(target, { force: true })
      await ctx.fiber.dispose()
    }
  })
})

/** A scriptable in-repo CodeRuntime whose program body is set per-test. */
class ScriptRuntime extends CodeRuntime {
  readonly language = 'typescript'
  readonly isolation = 'fake'
  behavior: (r: CodeRunRequest) => Promise<CodeRunResult> = () => Promise.resolve({ logs: [] })
  run(request: CodeRunRequest): Promise<CodeRunResult> { return this.behavior(request) }
}

describe('E5 — mandatory execution seam: Code Mode serialized sub-call', () => {
  it('a run_code sub-call to a guarded tool is DENIED by the monotonic guard (sub-calls are not a bypass)', async () => {
    const ctx = new Context()
    await ctx.plugin(SystemPrompt)
    await ctx.plugin(ToolRuntime, { mode: 'code' })
    await ctx.plugin(ScriptRuntime)
    const runtime = ctx.codeRuntime as ScriptRuntime

    const bodyRan: string[] = []
    ctx.tools.register(defineTool({
      name: 'protected_effect',
      description: 'a protected effect tool reachable from code',
      parameters: { command: { type: 'string', required: true } },
      output: { schema: { type: 'string' }, render: (_a, v: string) => [{ type: 'text', text: v }] },
      execute(args: { command: string }) { bodyRan.push(args.command); return Promise.resolve(`ran:${args.command}`) },
    }))
    // A monotonic guard denies the effect regardless of how it is reached.
    ctx.tools.guard(exec => exec.name === 'protected_effect' ? 'denied by policy: protected_effect' : undefined)

    // The program invokes the tool as a serialized sub-call and reports the result.
    let subCallResult = ''
    runtime.behavior = async (request) => {
      const tools = request.bindings[0]!.functions
      try {
        const out = await tools.protected_effect!({ command: 'rm -rf /' })
        subCallResult = 'ALLOWED:' + String(out)
      } catch (e) {
        subCallResult = 'REJECTED:' + String((e as Error).message)
      }
      return { logs: [], value: subCallResult }
    }

    const result = await ctx.tools.execute({
      signal: new AbortController().signal,
      callId: CallId('cm-1'),
      name: RUN_CODE_NAME,
      arguments: { code: 'program', description: 'invoke protected_effect from code' },
      agent: seamAgent('/workspace') as never,
    })

    // The outer run_code completes, but the SUB-CALL was denied by the guard
    // and its body never ran — Code Mode is not a route around the pipeline.
    expect(result.isError).toBe(false)
    expect(subCallResult).toContain('REJECTED')
    expect(subCallResult).toContain('denied by policy: protected_effect')
    expect(bodyRan).toEqual([])
    await ctx.fiber.dispose()
  })
})
