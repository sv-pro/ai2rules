// AI2-111 spike: two layers in, one D3 world out, then the world checked by the
// real kernel over every case of its finite label space.
//
//   node compile-world.mjs [report.json]
//
// Layer 1, the tools: the lab's task (labs/webmcp/src/tasks.js) names the tools
// the agent uses, with their side-effect class and arguments.
// Layer 2, the business: the facts cedar-check derived (report.json), from
// profile.yaml or from the Cedar policies; it asserts the two agree.
//
// Part 1 compiles the two layers into a world and compares it byte for byte
// with what the lab itself compiles (worldYaml), for the three matrix tasks and
// for the live assistant. Equal bytes mean an equal matrix: the matrix is a
// function of the world.
//
// Part 2 checks the compiled live world without running any agent: every tool
// (allowed, forbidden, unknown) x mode x ambient label x argument labels goes
// through `harness gate`, and four properties are asserted on the answers.
import { execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const LAB = path.resolve(HERE, '..', '..', '..', 'labs', 'webmcp');
const { TASKS } = await import(new URL(`file:///${path.join(LAB, 'src', 'tasks.js').replace(/\\/g, '/')}`));
const { worldYaml, harnessBin, actionName } = await import(new URL(`file:///${path.join(LAB, 'src', 'defenses', 'd2-ai2rules.js').replace(/\\/g, '/')}`));
const { APPROVALS, assistantTask } = await import(new URL(`file:///${path.join(LAB, 'live', 'session.js').replace(/\\/g, '/')}`));

// `--control`: part 2 runs against the D2 world (no labels, no approvals)
// instead, as a positive control. P1 and P2 must fail there, or the check
// proves nothing.
const CONTROL = process.argv.includes('--control');
const reportArg = process.argv.slice(2).find((a) => !a.startsWith('--'));
const report = JSON.parse(readFileSync(reportArg ?? path.join(HERE, 'report.json'), 'utf8'));
if (!report.profile_equals_cedar) throw new Error('profile.yaml and the Cedar policies derive different facts');
const facts = report.cedar_enumerated.derived;
const tool = (cap) => `${cap.origin}/${cap.tool}`;

// The business layer applied to a task. A person can approve only when one is
// present: an unattended run (the matrix) has nobody to ask, so the approval
// rule is carried by `interactive` and the world for background runs leaves it
// out. (In background mode the kernel turns ASK into DENY, so keeping it would
// refuse every order: fail-closed, and the lab's matrix world does not have it.)
function applyBusiness(task, { interactive }) {
  const caps = task.capabilities.filter((c) => !facts.forbidden.includes(tool(c)));
  const leaky = caps.filter((c) => facts.private_stays_home[tool(c)] === false);
  if (leaky.length) throw new Error(`no world for a policy that lets private data out through ${leaky.map(tool).join(', ')}`);
  return {
    task: { ...task, capabilities: caps, sensitiveOrigins: task.origins.filter((o) => facts.sensitive_systems.includes(o)) },
    approvals: interactive ? caps.filter((c) => facts.approvals.includes(tool(c))).map((c) => actionName(c.origin, c.tool)) : [],
  };
}

// ---- part 1: the compiled world equals the lab's, byte for byte ----

const targets = [
  ...Object.values(TASKS).map((t) => ({ name: `matrix ${t.id}`, task: t, interactive: false, labApprovals: [] })),
  { name: 'live assistant', task: assistantTask(['crm']), interactive: true, labApprovals: APPROVALS },
];
let mismatches = 0;
console.log('Part 1: profile + tools -> D3 world, compared with the lab\'s own');
for (const t of targets) {
  const { task, approvals } = applyBusiness(t.task, t);
  const ours = worldYaml(task, { labels: true, approvals });
  const lab = worldYaml(t.task, { labels: true, approvals: t.labApprovals });
  const same = ours === lab;
  if (!same) mismatches += 1;
  console.log(`  ${same ? 'same' : 'DIFF'}  ${t.name}  (${ours.split('\n').length} lines)`);
}

// ---- part 2: the live world, checked by the kernel over its whole label space ----

const bin = harnessBin();
const { task: live, approvals } = applyBusiness(assistantTask(['crm']), { interactive: true });
const dir = mkdtempSync(path.join(os.tmpdir(), 'ai2-111-'));
const worldPath = path.join(dir, 'world.yaml');
writeFileSync(worldPath, CONTROL ? worldYaml(live, { labels: false, approvals: [] }) : worldYaml(live, { labels: true, approvals }));

const PUB = { integrity: 'trusted', confidentiality: 'public' };
const PRIV = { integrity: 'trusted', confidentiality: 'private' };
const SAMPLE = { string: 'x', integer: 1 };
const gate = (toolName, args, mode, flow) =>
  JSON.parse(execFileSync(bin, ['gate', '--world', worldPath], {
    input: JSON.stringify({ v: 1, tool: toolName, arguments: args, context: { session_id: 'ai2-111', mode, taint: 'clean', source_channel: 'user_prompt', flow } }),
  }).toString());

// Every argument either carries a label the host vouched for, or none (it then
// falls back to the ambient in the kernel).
function labelings(names) {
  let out = [{}];
  for (const n of names) out = out.flatMap((l) => [l, { ...l, [n]: PUB }, { ...l, [n]: PRIV }]);
  return out;
}

const known = live.capabilities;
const unknown = ['crm__add_note', 'shop__post_review', 'shop__get_shipping_estimate', 'attacker__save_deal'];
const failures = [];
let requests = 0;
const started = Date.now();
for (const mode of ['interactive', 'background']) {
  for (const ambient of [PUB, PRIV]) {
    for (const cap of known) {
      const a = actionName(cap.origin, cap.tool);
      const names = Object.keys(cap.args ?? {});
      const args = Object.fromEntries(names.map((n) => [n, SAMPLE[cap.args[n]]]));
      const source = live.sensitiveOrigins.includes(cap.origin);
      for (const labels of labelings(names)) {
        requests += 1;
        const res = gate(a, args, mode, { ambient, args: labels });
        const carriesPrivate = names.some((n) => (labels[n] ?? ambient).confidentiality === 'private');
        const where = `${a} ${mode} ambient=${ambient.confidentiality} args=${JSON.stringify(Object.fromEntries(Object.entries(labels).map(([k, v]) => [k, v.confidentiality])))}`;
        // P1: private data never reaches a tool on another system.
        if (!source && carriesPrivate && !(res.decision === 'DENY' && res.rule === 'confidentiality_violation')) failures.push(`P1 ${where}: ${res.decision} ${res.rule}`);
        // P2: money moves only with a person's yes; unattended, never.
        if (approvals.includes(a) && !carriesPrivate) {
          const want = mode === 'interactive' ? 'ASK' : 'DENY';
          if (res.decision !== want) failures.push(`P2 ${where}: ${res.decision} (wanted ${want})`);
        }
        // P4: a read of the confidential system is allowed and labels its result private.
        if (source && cap.class === 'read_only' && !(res.decision === 'ALLOW' && res.context?.flow?.result?.confidentiality === 'private')) failures.push(`P4 ${where}: ${res.decision} result=${JSON.stringify(res.context?.flow?.result)}`);
      }
    }
    // P3: a tool outside the world does not exist, in any context.
    for (const a of unknown) {
      requests += 1;
      const res = gate(a, { x: 'x' }, mode, { ambient, args: {} });
      if (res.decision !== 'ABSENT') failures.push(`P3 ${a} ${mode}: ${res.decision} ${res.rule}`);
    }
  }
}
console.log(`\nPart 2${CONTROL ? ' (CONTROL: the D2 world, P1 and P2 must fail)' : ''}: the live world through the real kernel, ${requests} requests in ${Date.now() - started} ms`);
for (const p of ['P1 private data never reaches another system', 'P2 orders need a person (ASK), unattended DENY', 'P3 forbidden and unknown tools are ABSENT', 'P4 the CRM read is ALLOWed and labelled private']) {
  const id = p.slice(0, 2);
  const bad = failures.filter((f) => f.startsWith(id));
  console.log(`  ${bad.length ? 'FAIL' : 'ok  '}  ${p}${bad.length ? ` (${bad.length})` : ''}`);
}
for (const f of failures.slice(0, CONTROL ? 3 : 10)) console.log(`    ${f}`);
if (CONTROL) {
  const caught = ['P1', 'P2'].every((id) => failures.some((f) => f.startsWith(id)));
  console.log(caught ? '  control ok: the check fails on a world without labels and approvals' : '  CONTROL FAILED: the check passed a weak world');
  process.exit(caught && !mismatches ? 0 : 1);
}
process.exit(mismatches || failures.length ? 1 : 0);
