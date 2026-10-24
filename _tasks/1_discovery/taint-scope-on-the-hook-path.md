# Discovery: on the hook path, one web read closes the whole session

**Found 2026-10-21, empirically.** `harness replay-transcripts` (#108) replayed 1,432 real
Claude Code tool calls through `cc-hook` with the starter manifest. With the governance it
would have imposed, the agent would not have been usable for daily work, and taint is a
large part of why. This item is the backlog record for narrowing taint to what actually
needs closing, so the hook can be turned on for real.

Builds on [`pact-granularity-mismatch.md`](pact-granularity-mismatch.md). PACT L2
(argument-level taint) is already in the kernel and in `agent-core`'s own loop, **but not
on the hook path**, which is where dogfooding happens.

## The finding

Taint is a single per-session value on the hook path. `GateRequest.context.taint` is one
scalar, the hook keeps one `taint-<session>` sidecar, and the starter world declares no
argument roles. So once a session reads anything from outside, every externally
effectful call is denied for the rest of the session: invariant 7, the flat monitor
PACT's Theorem 3 says must over-block.

The starter world makes this worse than it needs to be. `Bash_unclassified`, the default
for any shell command that is neither network nor destructive, is declared
`side_effect: Network`. After one `WebFetch`, `git status`, `cat` and `cargo test` are
denied as tainted egress.

## Evidence (replay of the maintainer's sessions, 2026-09-21 to 2026-10-21)

| | Windows (896 calls\*) | WSL (338 calls) |
|---|---|---|
| hook would ask | 55% | 90% |
| hook would deny | 6% | 3% |
| silent | 40% | 7% |

\* 198 Windows-path `missing_path` artifacts were excluded: Windows sessions replayed by a
Linux build.

- `taint_invariant` denials: 43 on Windows. **38 of them are `Bash_unclassified`**, ordinary
  local shell after a web read. The rest are a second `WebFetch` or `WebSearch` after the
  first one.
- `approval_required` asks: 460 on Windows and 275 on WSL, almost all `Bash_unclassified`
  (`cd`, `git`, `grep`, `cat`, `sed`). That is a separate noise source: classification,
  not taint.

## Plan: four steps, each measured with `replay-transcripts`

1. **Fix what taint applies to (manifest only).** Give the starter world a local-shell class
   (`git status/diff/log/show`, `ls`, `cat`, `head`, `tail`, `grep`, `rg`, `find`, `wc`,
   `cargo build/test/fmt/clippy`, …) with a non-egress side effect, so taint blocks only
   real egress (`curl`, `wget`, `ssh`, `scp`, `git push`, publishing). Network and
   destructive patterns are matched first, so `cd x && curl …` is still caught. Also
   reconsider whether `Bash_unclassified` should stay `approval_required`.
2. **Scope by destination and by source (manifest, plus host parsing in the adapter).**
   - **Destination:** an egress allowlist per world. A tainted session may still reach the
     declared trusted destinations (docs sites, package registries, the user's own GitHub
     org); everything else stays closed. This is "roots for the network".
   - **Source:** not every read taints. Declared trusted hosts (docs.rs, an internal wiki)
     do not taint the session. This extends today's path-based `taint_source` to hosts.
3. **Argument instead of session on the hook path (PACT L2), the main step.**
   - Add an optional `arg_taint` to `GateRequest.context`, a backward-compatible v1 field
     (D24/D80 discipline).
   - The adapter records what is clean-origin: user prompts (the `UserPromptSubmit` hook)
     and content read before the session was tainted.
   - An authority-bearing argument (`WebFetch.url`, a recipient, a network command's
     target) counts as clean only if its value is **proven** to come from there. Whatever
     cannot be proven stays under the ambient floor, so the check is sound by
     construction: deterministic and no LLM, the enforcement half of PACT §3.4 that we
     already adopt.
   - The starter world declares the roles: `WebFetch.url: Target`, and a `Target` for
     network-class shell.
   - The effect: a docs URL the user named still loads after reading a page, while a URL
     lifted from the page does not.
4. **Ask instead of deny on interactive egress (needs a decision record).** In interactive
   mode, tainted egress to a non-allowlisted destination becomes `ASK` with the reason
   ("this session read X; send to Y?"), and stays `DENY` in background. This relaxes the
   non-overridable invariant 7, so it is a deliberate choice for `DECISIONS.md`, not a
   patch. The approval binds to the exact call (`AuthorizationInstance`).

## Done when

Replaying the same sessions shows the share of `ask` / `deny` falling at each step, with
**zero** new admitted egress of values that came from untrusted content. Measure the second
part with crafted sessions: an injected URL must still be denied after every step.

`replay-transcripts` can measure step 3 on past sessions too: transcripts carry the user
prompts and the tool results. It needs to learn to feed them to the producer.

## Related

- `pact-granularity-mismatch.md`: the L2 kernel work this extends.
- #108 `replay-transcripts`: the measuring instrument. #109: the `missing_path` fix found
  by the same replay.
- Separate findings from the same replay, not covered here: `PowerShell` is ungoverned on
  Windows (115 calls passed through as ABSENT); Claude's memory writes under
  `~/.claude/projects/*/memory` are denied by the starter world's control-plane rule.
