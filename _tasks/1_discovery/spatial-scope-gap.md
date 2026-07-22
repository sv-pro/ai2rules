# Discovery: governance is action × taint, not location — the manifest has no spatial scope

**Found 2026-07-22, empirically**, by probing the live grant-mode dogfood
(`dogfood/live-grant-mode`) from a second Claude Code instance. The question that
surfaced it: *does the harness confine the agent to the repo it's wired in, or can a
governed session touch any directory?*

## The finding

The kernel decides on `{tool, arguments, taint}`. **`GateRequest` carries no cwd, no
root, no path** — `{tool, arguments, context{session_id, mode, taint}}`. So the harness
governs *what kind of action* and *how trust flows*, **never *where***. There is no notion
of an "own dir": the project directory matters only at **initialization** (where the
`.claude/` hook is wired, i.e. which sessions get governed), not at **decision time**.

## Evidence

- **Manifest schema has nothing spatial** — no `roots:`, no path field. `data_classes`
  includes `Secret`/`Credential`, but nothing binds a *path* to a class.
- **`GateRequest` has no path** — confirmed by reading the type and by empirical probe.
- **Out-of-repo == in-repo:** `Read /etc/shadow` and `Write ~/.bashrc` are granted
  *identically* to reading `./README.md` or writing `./src/x`. The kernel can't tell them
  apart because it never sees the location.
- **Corollary (the subtle one):** a *local* file read is **clean** — it doesn't arm the
  taint floor (only untrusted channels: `mcp_output`, `web_fetch`, do). So reading a local
  secret and then egressing it is **not** blocked by the floor. The floor defends against
  *injection* (untrusted-in → effectful-out), **not** exfiltration of your own files.

## A second face: read-taint was live, then deferred (D25 / D37)

A deeper probe (a second Claude instance, same day) turned up a sharper, related fact.
`side_effect_taint()` (`gate.rs:203`) taints only `Network | External | Memory` — local
reads never taint, which is why `Read /etc/shadow` leaves the session clean. But the
**June-era Python engine tainted reads and recorded the path**: superseded state files read
literally `tainted by Read (agent: main): …/agent-governance-toolkit/README.md`. So
**path-aware read-taint was live in June and is gone now** — removed at the **D37** Rust
cutover, filed as **D25 "path-based read-taint deferred."** "Deferred" undersells it: a
capability that was *in service* was dropped from working behavior.

**Framing (per "decisions outrank code"): this is a decision review of D25/D37**, not just a
missing feature — a live protection was set aside.

**Possible demo impact — to verify, not yet confirmed.** Narratives of the form "read an
untrusted ticket → session tainted → egress denied" depend on read-taint. The `.claude/hooks/
superseded/demo-injection-egress.sh` demo narrates exactly that. **Flag it for verification.**
*Note the flagship `poisoned_knowledge_demo` is NOT affected*: it taints via the `mcp_output`
channel (still `Untrusted+taint`), not via file-read — so the cross-layer demo still holds.

## Why it's a gap, not a bug

Fully consistent with the thesis — govern the *action ontology* and *provenance*. Spatial
confinement is simply a **primitive that doesn't exist yet**. The important reframe:
**"governed" (trust-flow) ≠ "confined" (filesystem scope)** — two orthogonal axes. The
harness does the first. Confinement today comes only from the OS sandbox (`docker/`,
"Running Claude Safely"), not the manifest.

## The fix (shape)

1. Add a **`roots:` / workspace-scope primitive** to the `WorldManifest` (MCP itself has
   "roots" for exactly this — its *absence* here is the tell).
2. **Thread the action's target path into `GateRequest`.**
3. The kernel compares path vs declared roots: `Read`/`Write` `ALLOW` under a root,
   `ASK`/`DENY` outside; paths under sensitive roots carry their `data_class`, finally
   wiring the unused `Secret`/`Credential` vocabulary.
   Deterministic (path/prefix comparison + lattice), design-time authored, no LLM — it fits
   the stochastic→deterministic border cleanly.

## Impact on grant mode / productization

Grant mode auto-approves `Write` *by type* → it would **auto-approve `Write ~/.ssh/…` with
no prompt**, because the path is invisible to the gate. So:
- **additive-by-default is safe**; **grant is safe only** where the agent may already touch
  the whole filesystem, **or** once path-scope lands.
- Path-scope is what makes "govern this project" mean **"jail to this project"** — the v1.5
  feature for the `install-governance.sh` wedge.

## Home

`_tasks/1_discovery`. Post: **"Governed Is Not Confined"**. Related: `running-claude-safely`
(OS confinement — the *other* axis), `permission-list-cant-see-taint` (taint),
`docs/demos/replace-permissions/`, `wm-modifications-mechanism`. Deterministic-and-design-time,
so — like PACT — adoptable without putting an LLM on the gate.
