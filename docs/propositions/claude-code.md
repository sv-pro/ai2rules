# Improvement propositions — ai2rules (Claude Code review)

A prioritized shortlist of what could be improved in the project, authored from a
Claude Code working session on **2026-06-30**. The Tier 1 items are architectural
inconsistencies observed *directly* while working in the repo (each cites the file
that evidences it), not speculative nitpicks.

Propositions are numbered **P1–P7** for reference. Status legend: 📋 proposed.

---

## Tier 1 — architectural integrity (the thesis's own credibility)

### P1 — The live Claude Code gate is the Python *reimplementation*, not the kernel 📋
**What.** `.claude/settings.json` wires `python3 .claude/hooks/world-gate.py` as the
PreToolUse hook. That script re-derives taint / egress / ABSENT in Python — a parallel
engine — while the real Rust `harness cc-hook` / `harness gate` (the actual kernel) is
built, tested, and sits **unused** as a demo artifact.

**Why it matters.** It contradicts the project's own load-bearing decisions: D24 (hosts
integrate through a thin adapter calling the *real* kernel, never a reimplementation) and
D33 ("one Rust binary, run the actual moat, not a parallel Python engine"). The session
governing *this very repo* isn't running the kernel the project sells.

**Fix.** Point `settings.json` at `harness cc-hook --world .claude/cc-world.yaml`; retire
`world-gate.py` and `world-gate-adapter.py` from the live path. Highest-leverage change —
it makes the dogfood real and collapses P2 and P3 in one move.

**Evidence.** `.claude/settings.json:9`, `crates/cli-harness/src/cc_hook.rs`.

### P2 — Two world manifests for one world: `cc-world.json` *and* `cc-world.yaml` 📋
**What.** The Python path reads `.claude/cc-world.json`; the Rust path reads
`.claude/cc-world.yaml`. Same Claude Code world, two files, two schemas, hand-synced.

**Why it matters.** Guaranteed drift — a policy edit to one silently diverges from the other.

**Fix.** Keep the `.yaml` (the compiler's real schema); delete the `.json`. Falls out of P1.

**Evidence.** both files present under `.claude/`.

### P3 — The D25 bash classifier is copied four times 📋
**What.** The egress/destructive command classification lives in four places:
`world-gate.py`, `world-gate-adapter.py`, `crates/cli-harness/src/cc_hook.rs`, and
`.opencode/plugin/ai2rules-gate.ts`.

**Why it matters.** Concrete proof it's a hazard: a substring false-positive bug (a
network-tool token matching inside a larger word) existed in three of the four copies;
fixing it meant editing each separately, and **`world-gate-adapter.py` still carries the
unfixed copy.** A "thin adapter" should not hold policy-adjacent logic in four languages.

**Fix.** After P1, the Rust `classify()` is the single Rust source; the TS plugin is the
only unavoidable cross-language mirror — document it as a deliberate mirror, or expose a
`harness classify` subcommand it can call so there is one source of truth.

**Evidence.** `grep -l` for the classifier hits all four files above.

---

## Tier 2 — honesty about the boundary

### P4 — The bash classifier is syntactic and trivially evadable 📋
**What.** Classification matches patterns against the raw command string. That is
bypassable by construction (`/usr/bin/<tool>`, `$(printf …)`, quoting, env expansion,
`base64 | sh`, …).

**Why it matters.** It is fine *as defense-in-depth*, but it is **not** a hard boundary.
The real egress boundary must be the effect layer — the `docker/` egress proxy / network
namespace — not command-string sniffing. Presenting the classifier as the boundary would
overclaim in the THESIS / blog.

**Fix.** State the limitation explicitly in D25 / `docs/THESIS.md`, and position the egress
proxy + sandbox as the enforced boundary, with the classifier as advisory routing only.

**Evidence.** `word_match` / `cmd_matches` operate on the command string; `docker/`
contains the egress proxy that is the actual boundary.

---

## Tier 3 — drift & hygiene

### P5 — Nothing tests that a host actually *loads* the adapter 📋
**What.** Gate verdicts are thoroughly tested, but no test confirms Claude Code runs the
hook or OpenCode loads the plugin (the `.opencode/plugin/` singular-vs-plural load is still
unconfirmed live).

**Fix.** One manual-verified smoke check per host, recorded in the demo runbooks.

### P6 — Stale metadata 📋
**What.** `AGENTS.md` still says **"total 104"** tests (actual: 123). The
`cli-agent → ai2rules` rename is half-done (the GitLab path still resolves via a `cli-agent`
redirect; some docs lag).

**Fix.** Refresh the test-count line; finish the rename sweep.

**Evidence.** `AGENTS.md:73`.

### P7 — Dead Python prototypes 📋
**What.** The superseded Python path (`safe-mcp-proxy` `feat/mcp-gateway`, the prototype
paths under `repos/`) now read as live code.

**Fix.** Remove or clearly archive them now that the Rust path is canonical.

---

## Through-line

The project preaches **"one kernel, thin adapters,"** yet its own live Claude Code
governance is still a **Python re-implementation** reading a **second manifest** with a
**duplicated classifier**. P1 is the keystone: wiring the live hooks to `harness cc-hook`
retires Python, removes a manifest (P2), and deletes two classifier copies (P3) at once.

**Suggested sequence:** P1 → (P2, P3 fall out) → P6 (cheap) → P4 (doc/positioning) →
P5 → P7.
