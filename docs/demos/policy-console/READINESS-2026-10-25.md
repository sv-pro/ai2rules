# Northwind PR #150: main integration — 25 October 2026

## Follow-up: after PR #129

Reintegrated main `b190047d8250b6d31e7d0fc64e2af88bfc063267` after PR #129.
The only conflict was HANDOFF.md; both records were retained. Claude Code's
`7b59686` Windows changes are included: Python scripts passed with `-c`, and
hook identity comparison handles verbatim prefixes and Windows case folding.
The runtime has no additional changes relative to that main.

Rebuilt the Linux runtime offline and repeated the complete isolated run:
**76/76 assertions passed**. [New binary/source hashes and assertion list](readiness-evidence/after-pr129-summary.json).
The initial local WSL attempt encountered CRLF shell files from Windows checkout;
normalizing those working-copy line endings to LF allowed the run (no source
logic change). GitHub's Linux checkout uses LF.

Also ran setup in Git Bash against the native Windows runtime from PR #129 in a
new temporary project, then executed the preflight wiring check: passed. Tested
verbatim/case-equivalent hook identity and a distinct-file negative control.
No presentation files, services, settings or hosts entries were modified.
This is not an interactive Claude Code rehearsal; the presentation gate below
remains distinct. The user now authorizes merge after fresh CI and conflict checks.

---

**Code checks passed; live presentation readiness is a separate gate.** The PR
now targets `main`, integrating `810c1a6f0f00605f235ab527618c430809c79387`.
No demo checkout, existing receiver/console, host settings or hosts file changed.
The isolated working branch is `codex/ai2-62-pr150-main`; updates go to the
existing PR branch `codex/ai2-62-northwind-readiness`. Nothing was merged.

## Integration and review

There were no published inline review comments. Integration exposed three
conflicts in the demo README, setup and skill. Resolved them by retaining port
rendering, fresh-session instructions and the explicit project-policy integrity
limit, while preserving main's other changes (including #130's tool vocabulary).

The current `harness init` adds outcome logging and splits the hook exec over
two physical lines. The old setup's end-of-line substitution no longer matched,
so it could not add `--enforce-absent`. Setup now recognizes the generated command,
adds ABSENT enforcement and only adds a log argument when one is missing. It
preserves the PreToolUse/PostToolUse/PostToolUseFailure registrations from init.

Preflight parses logical exec arguments, checks exact world/state/log paths and
required flags, and compiles/projects with the executable actually selected by
the hook (including HARNESS_BIN). An unrelated executable must not pass merely
because a different HARNESS on PATH can compile the world. The same checks
accept a valid multiline exec and remain read-only.

The launcher treats any HTTP response, including 404, as an occupied port before
setup. Its readiness polling still requires HTTP success. Both paths bypass
proxies. Custom ports and the selected binary are passed explicitly to setup
and preflight.

## Verification

Built the unchanged Rust runtime from main with pinned Rust 1.97.1 on Linux
(WSL Ubuntu 24.04), `cargo build -p cli-harness --offline --locked`. Binary and
source-world SHA-256 hashes are in [summary.json](readiness-evidence/2026-10-25/summary.json).
The runner uses temporary projects, a temporary HOME, private loopback ports,
explicit curl test DNS mappings and no Claude/model/credentials.

**76 assertions passed**, covering:

- Real init/setup and skill URL rendering with custom ports.
- Healthy and multiline hooks, with every preflight checked for no file changes.
- Missing hook/skill/grant/ABSENT/log, wrong world/state/log, disabled hooks,
  project kill switch, invalid world, relative or non-harness binary override,
  and a console serving another project.
- Current main world: Contractor ABSENT, Developer ALLOW and taint, deletion
  ASK then ABSENT, tainted upload DENY even after Allow, local report Edit ALLOW,
  fresh-session requirement, and app-tool projection. PowerShell-shaped events
  exercise the Linux adapter only; they are not Windows execution.
- Exact decision-log/console-feed equality; archive intact; no upload executed.
- Launcher refuses an occupied 404 port without altering a sentinel project;
  reaches manual handoff with a broken proxy and custom ports; cleans up both
  servers it created.

This run replaces the old two-world comparison with one current-main world;
its count is not directly comparable to the historical 84 assertions. Historical
controls can still be supplied explicitly with --original-preflight and
--comparison-world (the old --pr130-world alias remains accepted). Normal runs
need neither Git history nor an unmerged dependency. The CI `demos` job now runs
these assertions against the binary it builds.

```bash
cargo build -p cli-harness --offline --locked
python3 scripts/check-policy-console-demo.py \
  --harness "$PWD/target/debug/harness" \
  --binary-commit "$(git rev-parse HEAD)" \
  --evidence /tmp/northwind-readiness
```

[Retained evidence](readiness-evidence/2026-10-25/) replaces generated temp roots
and the build binary path with placeholders. Native decisions and verdicts are
retained; the previous dated evidence is unchanged.

## Remaining presentation gate

Interactive `/northwind-demo`, model choices, permission/question widgets,
actual Windows/Git Bash execution and the presentation-host settings were not
exercised. Those require a fresh-session rehearsal on the chosen presentation
host before presenting. They are not evidence supplied by this automated test,
and this PR does not certify the live stand. The user explicitly excluded that
stand from this work.

No new production policy, runtime logic, path boundary or deferred-PR changes.
The demo still permits edits to its own world/hook; a skill instruction is not
an enforced filesystem boundary. Publication/merge is not part of this run.
