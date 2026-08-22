# D7 — Minimal `*`-glob redaction, no dependency

- **Epic:** E4 · **Status:** Accepted
- **Decision:** Redact JSON values whose key/dotted-path matches a manifest
  pattern via a small `*`-wildcard matcher.
- **Alternatives:** Add a glob crate for full `**`/path semantics.
- **Why:** Lean deps; masking keeps keys present and values string-typed so it
  doesn't change representability. Full glob deferred.
