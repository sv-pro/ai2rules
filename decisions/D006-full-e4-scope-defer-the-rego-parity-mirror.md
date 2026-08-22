# D6 — Full E4 scope; defer the Rego parity mirror

- **Epic:** E4 · **Status:** Accepted
- **Decision:** Ship record + redaction + replay + drift report + bundle
  (E4.1–E4.5); defer the cross-implementation Rego mirror (E4.6).
- **Alternatives:** Core only (E4.1–E4.3).
- **Why:** Replay + drift + bundle are what make M1's "deterministic core"
  demonstrable; a second-language parity harness adds little before there's a
  benchmark suite.
