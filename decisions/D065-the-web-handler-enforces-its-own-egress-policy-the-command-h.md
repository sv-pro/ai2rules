# D65 — The web handler enforces its own egress policy; the command handler still cannot


**Date:** 2026-08-14. **Closes findings #11 and #12.**

- **#12: `WebHandler` ignored the spec's `NetworkPolicy`.** A spec built with
  `NetworkPolicy::Disabled` still made the request. The asymmetry with `CommandHandler` is the
  whole point: a subprocess opens its own sockets and the handler cannot police them, so D47
  makes it fail closed. The web handler *is* the thing performing the egress, so it can enforce
  the policy — and now does. `Disabled` refuses; `AllowHosts` matches the URL's real host.
- **The URL parser is hand-rolled, deliberately.** A security decision keyed on "the host" must
  make visible what it thinks the host is. The bypass that matters is **userinfo**:
  `https://docs.example@evil.example/` has host `evil.example`, and any check that asks whether
  the URL *contains* an allowed host reads it as allowed. Userinfo is split at the **last** `@`,
  IPv6 literals are unbracketed, trailing dots and case are normalised, and each of those has a
  test.
- **Loopback, link-local and private ranges need naming explicitly.** An allowlist entry may
  name `127.0.0.1` — a local dev server is a legitimate target — but a broad or suffix entry
  must not silently reach `169.254.169.254`. This is SSRF hygiene, not policy invention.
- **The finding's second half: the policy is never configured.** `ExecEnv.network` defaults to
  `Disabled` and *nothing in the codebase sets it to anything else*, so enforcing it correctly
  turns web fetch off until a caller grants egress. That is the right default and it is worth
  stating plainly: before this change the field was inert, so nobody noticed it was unset. The
  manifest has no vocabulary for an egress allowlist yet — the caller must supply `ExecEnv`.
  Blast radius is the in-process agent loop (demos and tests), not the deployed hook adapters,
  which are decision-only and never execute.
- **#11: a timed-out command left its descendants running.** `child.kill()` signals the direct
  child only, so `sleep 300 &` survived its parent's timeout. Worse, a surviving descendant
  inherits the stdout/stderr pipes, so the reader threads never saw EOF and `out_reader.join()`
  could block **forever** — the timeout path, whose entire job is to bound a command, could
  hang the executor instead. Measured: the two regression tests take 31.8s and fail without the
  fix, 2.0s and pass with it.
- **The child gets its own process group and the group is killed.** `SIGKILL` rather than
  `SIGTERM`: this path has already waited out the command's whole timeout budget, so there is
  no grace period left to offer.
- **`nix` rather than `libc`, to keep the workspace free of `unsafe`.** `killpg` via raw `libc`
  would be three lines and one `unsafe` block; it would also be the first `unsafe` in 20k lines
  of a security tool. The safe wrapper costs a dependency and keeps the property.
- **Windows is not fixed and says so.** Bounding a process tree there needs a Job Object the
  executor does not create, so a timed-out command may still leave descendants. Stated in the
  code rather than left for someone to discover.
- **Alternatives rejected.**
  - *Pull in a URL crate for parsing.* Reasonable, and it hides the userinfo rule behind a
    dependency in the one place a reviewer most needs to see it.
  - *Denylist known-bad hosts instead of an allowlist.* The policy type is already an allowlist;
    a denylist would invert the failure direction to open.
  - *`SIGTERM` then `SIGKILL` after a grace period.* Doubles the worst-case time on a path that
    exists to enforce an upper bound the command has already blown through.
- **Related:** D47 (the command handler's fail-closed), finding #10;
  `crates/executor/src/handlers/{web,command}.rs`.
