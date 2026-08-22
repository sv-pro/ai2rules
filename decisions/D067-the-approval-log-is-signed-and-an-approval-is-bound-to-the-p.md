# D67 — The approval log is signed, and an approval is bound to the policy that granted it


**Date:** 2026-08-14. **Closes the last P2 of the security sweep.**

- **The hole.** The approval store is append-only JSONL, and it is the one file in this system
  whose contents *grant* something. Anything able to write it could append a token already in
  the `Approved` state and manufacture a human decision that never happened; `load` folded it
  into the token set without a murmur. Separately, an approval bound to `world_id` but not to
  the compiled manifest survived a rewrite of the very rules it was granted under — a world
  keeps its id while its policy changes completely.
- **Every line carries an HMAC-SHA256**, keyed by a 32-byte secret read from `/dev/urandom` on
  first use and kept beside the log at `0600`, opened `O_NOFOLLOW`, with mode and ownership
  re-checked on every open. Forging a grant now requires the key rather than merely write
  access.
- **A line that does not verify fails the whole load, rather than being skipped.** A store that
  has been modified by something without the key is not a store to keep answering from, and the
  failure direction is safe: no approvals means the human is asked again. There is deliberately
  no lenient path for an unsigned line either — from the verifier's side, "unsigned" and
  "forged" are the same thing.
- **HMAC is implemented here rather than imported.** The offline crate set has `sha2` and not
  `hmac`. It is RFC 2104 in about twenty lines, checked against three RFC 4231 known-answer
  vectors including the longer-than-block-size key that exercises the digest branch. A
  hand-rolled MAC without known-answer tests would be worse than no MAC, because it would look
  like protection. MAC comparison is constant-time; a `==` on the hex would turn forgery into
  32 cheap searches.
- **`manifest_hash` joins the binding.** `world_id` says *which* world, `manifest_hash` says
  *which version of it*, and an approval now dies when either moves.
- **Location beats cryptography, and the code now says so.** The CLI keeps the store in a
  tempdir; a comment proposed moving it to `.agents/` in a deployed tool, which is inside the
  project being governed and therefore exactly wrong. Same mistake as D58 (the kernel inside
  `node_modules`) and D57 (the control plane): anything the enforcement depends on must live
  outside what it enforces upon. The MAC is what remains true when that assumption fails.
- **Alternatives rejected.**
  - *Rely on filesystem permissions alone.* They are the primary defence and they are exactly
    what fails in the scenario the finding describes — a store reachable by the governed
    project.
  - *Sign the whole file rather than each line.* Breaks append-only writing: every append would
    rewrite and re-sign the log, and a crash mid-write would void every past approval.
  - *A hash chain instead of a MAC.* Detects reordering and truncation but not appending, since
    anyone can continue an unkeyed chain. Appending is the attack.
  - *Derive the key from a passphrase or machine id.* A machine id is not a secret, and a
    passphrase prompt in a hook that must run non-interactively is a non-starter.
  - *Skip bad lines and carry on.* Turns a detected forgery attempt into a silent partial load.
- **Known limits.** The key sits beside the log, so an attacker with read access to that
  directory can forge freely — the MAC defends against write-without-read, which is the
  realistic case for a governed project directory, not against full compromise. Windows ACLs
  are not checked; only the MAC binds there. And `/dev/urandom` is the only CSPRNG wired up, so
  non-unix platforms refuse to create a store rather than invent a weak key.
- **Related:** D57, D58 (the same "outside what it enforces upon" rule), E6.2–E6.4;
  `crates/trace-store/src/{integrity,approval}.rs`.
