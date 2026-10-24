# Advocacy: pitch lines (one main, two reserves)

**Opened 2026-10-21. Positioning note, not a plan of record.** Nobody knows yet where
ai2rules lands, or whether it lands at all. So we keep a small portfolio of pitch
lines instead of one bet, without spreading thin: **one main line** that gets the demo
work now, **two reserve lines** kept alive cheaply, and a **parking lot** for the rest.

## The rule against spreading thin

Every line sells the same engine: deterministic verdicts, taint that cannot be talked
out of, actions that do not exist for the agent, and an audit log that replays. A line
may add only three things of its own:

1. **a story**: a world in the audience's vocabulary and a `harness demo --story <name>`;
2. **a landing**: one page or post in the audience's words;
3. **an adapter** where its hosts need one.

If a line needs kernel work that no other line benefits from, that is a warning sign.
Default time split: about 70% main, 20% reserve 1, 10% reserve 2. Reserve work is
preferably work that also helps the main line.

## Main line — small offices outside IT (accounting first, law second)

- **Who:** accounting offices and small law firms, 2–20 people, with a shared mailbox,
  no IT staff, and either no AI yet or staff pasting documents into public chat tools.
- **Pain, in their words:** "an email arrives with a changed bank account and we pay a
  fraudster"; "client documents leak"; "we want the time savings without the risk".
- **Pitch:** an assistant that reads the mailbox and fills in the books, and **cannot**
  be talked into paying, sending or deleting anything. Whatever a hidden instruction in
  an email says, the action does not exist or needs a person.
- **Demo:** `--story law-firm` (shipped in 0.7.0); `--story accounting` and the receipt
  intake agent are next, tested against `receipt-forge` ground truth.
- **Money, options to test:** setup service plus a monthly fee; a reseller margin
  through accountants' networks; a fixed-price "safe AI intake" package.
- **What exists:** the kernel, the law-firm story, `receipt-forge` (realistic Israeli
  receipts with planted attacks).
- **Missing:** the intake agent; the accounting story; ru/he/en voiceover for one demo;
  first conversations (roles, fears, BEC history).
- **Signal it is working:** one real office runs it on generated documents and asks to
  try it on their own mail.

## Reserve 1 — IT teams adopting coding agents

- **Who:** engineering teams rolling Claude Code, Copilot, Antigravity, OpenCode or
  other MCP-using agents into their development cycle, and the security or platform
  people who must sign off.
- **Pain:** agents with a shell, secrets and MCP tools; prompt injection from issues,
  web pages and dependencies; "what exactly can the agent do here, and can you prove
  it afterwards?"
- **Pitch:** one policy across every coding agent you run, decided deterministically,
  logged, replayable. **Start without risk:** `harness replay-transcripts` shows what
  the policy would have decided on your past sessions, before anything is enforced.
- **Demo:** the one-kernel-many-hosts demo, `replay-transcripts` on a real history, the
  MCP governance benchmark (ai2rules passes 3/3 scenarios, the weak gateway fails all 3).
- **Money, options to test:** open-source core plus paid team features (central policy,
  approval routing, an audit view); integration consulting.
- **What exists:** most of it: `cc-hook`, `agy-hook`, OpenCode, `mcp-gateway`, the gate
  ABI, the benchmark, the blog. This line is the dogfood, so it stays alive almost free.
- **Missing, and it blocks the pitch:** the false-positive rate. Replaying 1,432 real
  calls, the starter world would ask on 55–90% of them, mostly because one web read
  taints the whole session (`_tasks/1_discovery/taint-scope-on-the-hook-path.md`).
  Also: PowerShell is ungoverned, and the shim does not find an npm-installed binary.
- **Signal:** an outside team runs `replay-transcripts` and shares its report.

## Reserve 2 — builders of agent and MCP platforms

- **Who:** vendors and internal platform teams building agent runtimes, MCP gateways or
  vertical agents, who need a policy engine rather than an end-user product.
- **Pitch:** embed a deterministic gate instead of writing your own: a versioned gate
  ABI, a WASM build that runs in the browser or the edge, and a public benchmark to
  measure against.
- **Demo:** the benchmark report; the DeepSeek Harness adapter as an integration
  example; the WASM preview on the site.
- **Money, options to test:** a commercial licence or support contract; paid
  integration work.
- **What exists:** gate ABI v1, `harness-wasm`, `mcp-gateway`, the DeepSeek Harness
  adapter, the benchmark.
- **Missing:** a stable embedding API story (the crate API is not promised to be
  stable); one external integration that is not ours.
- **Signal:** someone asks to embed it.

## Parking lot (not worked on; revisit when a signal appears)

- **Mid-size firms on Microsoft 365 Copilot.** Core Copilot is closed; the seams are
  Copilot Studio agents and the tools they call. Needs the research in
  `_tasks/1_discovery/microsoft-365-copilot-integration.md` first.
- **Other small-office verticals** with the same shape (clinics, real-estate agencies,
  property management): mostly a new story and world, reusing the main line's work.
- **A domain language for non-programmers** to describe their own allowed actions
  (`_tasks/1_discovery/action-vocabulary-and-domain-dsl.md`): could become the product
  of the main line, not a line of its own.
- **Teaching:** a course, workshop or book chapter on governing agents, if the blog
  finds an audience.

## When to change the lineup

Revisit after each real conversation with someone in a line's audience, and at the
latest once a quarter. Promote a reserve if it shows its signal before the main line
does; drop a line that has had no conversation in two quarters.
