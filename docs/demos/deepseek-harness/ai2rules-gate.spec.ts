/**
 * ai2rules-gate adapter spike (issue #55) — run against the REAL dsh
 * ToolRegistry pipeline AND the REAL `harness gate` binary + deepseek-world.yaml.
 *
 * Proves the verdict mapping (ALLOW/DENY/ASK/ABSENT) and — the point of #55 —
 * the FAIL-CLOSED contract when the engaged gate is unavailable or errors.
 *
 * Env the runner must set (absolute paths):
 *   AI2R_HARNESS  -> built `harness` binary
 *   AI2R_WORLD    -> docs/demos/deepseek-harness/deepseek-world.yaml
 */
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { Context } from '@deepseek-ai/cordis'
import { CallId } from '@deepseek-ai/dsh-llm'
import SystemPrompt from '@deepseek-ai/dsh-system-prompt'
import ToolRuntime from '@deepseek-ai/dsh-tools'
import type { ToolExecutionResult } from '@deepseek-ai/dsh-tools'
import ApprovalService from '@deepseek-ai/dsh-user-approval'
import { apply as gateApply, name as gateName, inject as gateInject } from './ai2rules-gate.ts'

const HARNESS = process.env.AI2R_HARNESS as string
const WORLD = process.env.AI2R_WORLD as string

const dirs: string[] = []
function stateFile(): string {
  const d = mkdtempSync(join(tmpdir(), 'ai2r-gate-'))
  dirs.push(d)
  return join(d, 'state.json')
}
afterEach(() => { for (const d of dirs.splice(0)) rmSync(d, { recursive: true, force: true }) })

let callCounter = 0
function makeCall(ctx: Context, ran: string[]) {
  const tool = (n: string) => ({
    name: n, description: `test ${n}`,
    parameters: { type: 'object' as const, properties: { command: { type: 'string' as const } } },
    output: { schema: { type: 'string' as const }, render: (_a: unknown, v: string) => [{ type: 'text' as const, text: v }] },
    async execute(args: { command?: string }): Promise<string> { ran.push(n); return `ran:${n}:${args.command ?? ''}` },
  })
  return { tool, call: (name: string, args: unknown, sessionId = 'S1'): Promise<ToolExecutionResult> =>
    ctx.tools.execute({
      signal: new AbortController().signal, callId: CallId(`c-${++callCounter}`), name, arguments: args,
      agent: { id: sessionId, session: { id: sessionId, header: { cwd: '/workspace' }, events: [{ type: 'turn/start' }, { type: 'user/message' }], append: () => ({}) } } as never,
    }) }
}

async function tree(env: Record<string, string | undefined>, opts: { approval?: 'allow' | 'reject' } = {}) {
  const prev: Record<string, string | undefined> = {}
  for (const [k, v] of Object.entries(env)) { prev[k] = process.env[k]; if (v === undefined) delete process.env[k]; else process.env[k] = v }
  const ctx = new Context()
  await ctx.plugin(SystemPrompt)
  await ctx.plugin(ToolRuntime)
  if (opts.approval !== undefined) {
    await ctx.plugin(ApprovalService)
    ctx.on('approval/request', () => Promise.resolve(opts.approval === 'allow' ? 'allowed-once' : 'rejected'))
  }
  await ctx.plugin({ name: gateName, inject: gateInject, apply: gateApply })
  const ran: string[] = []
  const { tool, call } = makeCall(ctx, ran)
  return { ctx, ran, tool, call, restore: () => { for (const [k, v] of Object.entries(prev)) { if (v === undefined) delete process.env[k]; else process.env[k] = v } } }
}
function text(r: ToolExecutionResult): string { return r.content.filter(b => b.type === 'text').map(b => (b as { text: string }).text).join('') }

describe.runIf(HARNESS && WORLD)('ai2rules-gate adapter (#55) — real kernel, real dsh pipeline', () => {
  const base = () => ({ AI2RULES_HARNESS: HARNESS, AI2RULES_WORLD: WORLD, AI2RULES_STATE: stateFile(), AI2RULES_MODE: 'interactive', AI2RULES_DISABLE: undefined })

  it('ALLOW: a clean read runs', async () => {
    const t = await tree(base())
    t.ctx.tools.register(t.tool('read'))
    const r = await t.call('read', {})
    expect(r.isError).toBeFalsy(); expect(t.ran).toEqual(['read'])
    t.restore()
  })

  it('ALLOW: clean bash curl is classified bash_network and runs', async () => {
    const t = await tree(base())
    t.ctx.tools.register(t.tool('bash'))
    const r = await t.call('bash', { command: 'curl http://x' })
    expect(r.isError).toBeFalsy(); expect(t.ran).toEqual(['bash'])
    t.restore()
  })

  it('DENY: tainted web_fetch hits the taint floor and is blocked', async () => {
    const env = base()
    writeFileSync(env.AI2RULES_STATE!, JSON.stringify({ taint: { S1: 'tainted' }, usage: {}, cause: {} }))
    const t = await tree(env)
    t.ctx.tools.register(t.tool('web_fetch'))
    const r = await t.call('web_fetch', {})
    expect(r.isError).toBe(true); expect(text(r)).toContain('[ai2rules] DENY'); expect(t.ran).toEqual([])
    t.restore()
  })

  it('ASK: bash rm -rf routes to ctx.approval; allowed-once -> runs', async () => {
    const t = await tree(base(), { approval: 'allow' })
    t.ctx.tools.register(t.tool('bash'))
    const r = await t.call('bash', { command: 'rm -rf /tmp/x' })
    expect(r.isError).toBeFalsy(); expect(t.ran).toEqual(['bash'])
    t.restore()
  })

  it('ASK: bash rm -rf; rejected -> blocked, body never runs', async () => {
    const t = await tree(base(), { approval: 'reject' })
    t.ctx.tools.register(t.tool('bash'))
    const r = await t.call('bash', { command: 'rm -rf /tmp/x' })
    expect(r.isError).toBe(true); expect(t.ran).toEqual([])
    t.restore()
  })

  it('ABSENT: a registered tool not in the world is denied at invocation', async () => {
    const t = await tree(base())
    t.ctx.tools.register(t.tool('todo_write')) // real dsh tool, but not in deepseek-world.yaml
    const r = await t.call('todo_write', {})
    expect(r.isError).toBe(true); expect(text(r)).toContain('[ai2rules] ABSENT'); expect(t.ran).toEqual([])
    t.restore()
  })

  it('FAIL-CLOSED: engaged but harness binary missing -> DENY', async () => {
    const t = await tree({ ...base(), AI2RULES_HARNESS: '/nonexistent/harness' })
    t.ctx.tools.register(t.tool('read'))
    const r = await t.call('read', {})
    expect(r.isError).toBe(true); expect(text(r)).toContain('gate unavailable'); expect(t.ran).toEqual([])
    t.restore()
  })

  it('FAIL-CLOSED: gate exit 2 (bad world) -> DENY', async () => {
    const t = await tree({ ...base(), AI2RULES_WORLD: '/nonexistent/world.yaml' })
    t.ctx.tools.register(t.tool('read'))
    const r = await t.call('read', {})
    expect(r.isError).toBe(true); expect(text(r)).toContain('exit 2'); expect(t.ran).toEqual([])
    t.restore()
  })

  it('BYPASS: AI2RULES_DISABLE=1 runs ungoverned (the not-activated case)', async () => {
    const t = await tree({ ...base(), AI2RULES_DISABLE: '1', AI2RULES_HARNESS: '/nonexistent/harness' })
    t.ctx.tools.register(t.tool('web_fetch'))
    const r = await t.call('web_fetch', {})
    expect(r.isError).toBeFalsy(); expect(t.ran).toEqual(['web_fetch'])
    t.restore()
  })
})
