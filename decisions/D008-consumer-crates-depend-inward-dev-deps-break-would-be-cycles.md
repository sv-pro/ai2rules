# D8 — Consumer crates depend inward; dev-deps break would-be cycles

- **Epic:** cross-cutting · **Status:** Accepted
- **Decision:** Replay/spec/approvals live where their inputs are: `trace-store`
  depends on `world-kernel` + `compiler`; `world-kernel` uses `compiler`/
  `executor`/`tempfile` as **dev-deps** for tests/demos only.
- **Alternatives:** Keep `trace-store` storage-only with replay elsewhere; avoid
  any cross-crate test deps.
- **Why:** The kernel depends on neither `trace-store` nor `executor`, so there's
  no cycle; the dependency graph still flows inward to `harness-types`.
