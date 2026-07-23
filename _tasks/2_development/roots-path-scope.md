# Task: `roots` — path-scoped capabilities (spatial confinement primitive)

**Owner:** kernel (build). **Status:** ✅ **v1 built** (`feat/roots-path-scope`) — the build end
of the [spatial-scope discovery](../1_discovery/spatial-scope-gap.md). Manifest `roots`
(RootAccess/RootRule/RootsDef) → pure `classify_path`/`path_taints` on `CompiledWorld` →
`gate()` tightens ALLOW (deny/ask/read-only) + `taint_source` read-taint → `cc-hook` resolves the
target path; `resolve_root_paths` keeps `compile` pure. Closed-world allowlist, Bash exempt (no
path), canonicalization at the adapter. 14 new tests (172 workspace, green). **Follow-ups:**
symlink canonicalization (v1 is lexical), a formal PLAN acceptance invariant, per-actor roots.

## Why

The kernel governs `action-type × taint`, never *location*: `GateRequest` carries no path,
so `Read /etc/shadow` is granted identically to `Read ./README`. "Governed" ≠ "confined"
(see the discovery + the *"Governed Is Not Confined"* post). This task adds the missing
axis: a manifest can declare the directories an actor is scoped to, and the kernel decides
file actions by *where they land*, not just *what kind* they are. It is also the safety
prerequisite for running **grant mode** on a live machine — without it, grant auto-approves
`Write ~/.ssh/…` because the path is invisible.

## The contract

### 1. Manifest — a `roots:` block (opt-in; absent ⇒ today's behavior)

A closed-world allowlist, mirroring the action ontology (`ABSENT`): a path outside every
declared root does **not exist** for the actor. One ordered rule list, **longest-prefix
match wins**, each rule an access level + optional `class`; a `default` for unmatched paths.

```yaml
roots:
  default: Ask                     # unmatched path -> Ask (or Deny for a hard jail)
  rules:
    - { path: ".",        access: ReadWrite }              # the project (compile-time -> project dir)
    - { path: "/tmp",     access: ReadWrite }
    - { path: "/usr",     access: Read }                   # tools/libs, read-only
    - { path: "~/.ssh",   access: Deny, class: Credential } # shadows a broader allow
    - { path: "/etc/shadow", access: Deny, class: Secret }
```

`access ∈ {Read, ReadWrite, Ask, Deny}`. A `Deny`/sensitive rule with a longer prefix
shadows a broader `ReadWrite`. `class` (optional) tags the path's `data_class` — finally
wiring the manifest's existing but unbound `Secret`/`Credential` vocabulary.

**`taint_source` (restores the deferred read-taint).** A rule may set `taint_source: true` —
reading under that path **taints the session** (path-aware read-taint, the capability D25/D37
deferred; see the discovery's "second face"). This is the manifest primitive that brings back
"read an untrusted file → session tainted → egress denied," now *declared* instead of
hard-coded, and deterministic. Example: `{ path: "./inbox", access: Read, taint_source: true }`.
So one primitive closes **both** the grant-mode blast radius (writable roots) *and* the lost
read-taint edge (taint-source roots).

### 2. `GateRequest` — thread the action's target path

Add the resolved path(s) the action touches. **The adapter canonicalizes; the kernel stays
pure** (see decision C). For file tools the path comes from `arguments` (`file_path`, …).

### 3. Kernel — pure prefix decision

Given a canonical path: longest-prefix-match against `roots.rules` → apply that rule's
`access` (map to `ALLOW`/`ASK`/`DENY`, `ReadWrite` gating write-effect actions); no match →
`default`. Pure comparison + lattice, **no I/O, no LLM** — fits the border.

## Design decisions (the real choices)

| # | Decision | Recommendation |
|---|---|---|
| A | **Closed vs open world.** Allowlist (outside a root ⇒ denied, matches `ABSENT`) vs denylist (open, block sensitive paths). | **Closed** (allowlist), but ship the default manifest with a permissive, usable root set (project + `/tmp` + read-only system paths) so it isn't hostile. `default: Ask` for a soft jail, `Deny` for a hard one — a knob. |
| B | **Bash is undecidable.** A shell command can touch arbitrary paths; you can't statically extract them. | **Do NOT path-scope Bash in v1.** Bash keeps its `command_classes` + taint governance; *spatial* confinement of Bash is an **OS-sandbox** job (`docker/`, "Running Claude Safely"). Say this loudly — path-scope covers *structured* file tools only. |
| C | **Canonicalization needs I/O** (resolve symlinks/`..`/`~`) but the kernel is pure. | **Canonicalize in the adapter** (cc-hook/executor — the I/O boundary), pass the *canonical* path into `GateRequest`; kernel does pure prefix-match. Note the symlink-TOCTOU caveat (a symlink created between check and use); lexical-only normalization is purer but symlink-evadable. |
| D | **Global vs per-actor roots.** | Global roots in v1; per-actor/per-capability roots is a later refinement. |
| E | **Compile-time resolution.** `.` and `~` in declared roots. | Resolve `.` relative to the project dir and expand `~` **at compile time** into the `CompiledWorld` (immutable), so runtime is pure comparison. |

## Scope / non-goals (v1)

- **In:** `roots:` schema (access + `class` + `taint_source`); canonical path into
  `GateRequest`; kernel prefix decision for **structured file tools** (Read/Write/Edit/…);
  read-taint on `taint_source` roots (restores the D25/D37-deferred capability);
  backward-compat (no `roots:` ⇒ unchanged).
- **Out:** Bash spatial scoping (decision B — OS sandbox); per-actor roots (D); auto-tainting
  every `Secret`-class read (confidentiality axis — a separate design; `taint_source` is the
  opt-in bridge).

## Crate changes

- **harness-types:** `Root`/`RootRule` + `roots` on `WorldManifest`; compiled `PathScope` on
  `CompiledWorld`; a canonical `path` field on the perception/`GateRequest` context.
- **compiler:** compile + validate `roots` (resolve `.`/`~`, canonicalize declared roots),
  longest-prefix index; hash it into the manifest hash.
- **world-kernel:** the pure prefix-match → disposition; a new acceptance invariant.
- **executor / cc-hook (adapter):** extract the target path from tool arguments, canonicalize
  (I/O), populate `GateRequest`.
- **harness-preview:** `GateRequest`/`gate()` carry the path; keep native/WASM parity.
- **docs:** THESIS §5 (new §5 primitive), GLOSSARY (`root`, `path-scope`), PLAN invariant.

## Acceptance

- New invariant (next free number): **"no file action resolves outside a declared root
  without an explicit decision"** — path confinement as a checkable property, green in CI.
- `Read /etc/shadow` / `Write ~/.ssh/id_rsa` → `DENY`/`ASK` per the manifest, while in-root
  Read/Write still `ALLOW` — the exact probe from the discovery, now asserted on.
- **Grant-mode payoff test:** with `--grant` + roots, in-root Write grants (no prompt),
  out-of-root Write denies — proving grant is safe on a live machine once this lands.
- Backward compatible: a manifest with no `roots:` behaves exactly as today.

## The border-fit note

Deterministic (path/prefix comparison + lattice), design-time authored (roots are written by
a human, frozen by the compiler), no LLM on the gate. Like PACT's enforcement layer, it's
adoptable as-is — and it's the primitive that makes `install-governance.sh --grant` mean
"jail to this project," the v1.5 for the productization wedge.

## Related

[spatial-scope discovery](../1_discovery/spatial-scope-gap.md) ·
`wm-modifications-mechanism` (the scope axis rhymes) · `docs/demos/replace-permissions/` ·
MCP "roots" (prior art — its absence here is what named the gap) ·
`running-claude-safely` (the OS-confinement axis this complements).
