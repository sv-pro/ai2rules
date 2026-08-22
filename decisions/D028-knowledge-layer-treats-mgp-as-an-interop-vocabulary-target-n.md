# D28 — Knowledge layer treats MGP as an interop/vocabulary target, not a runtime to adopt (yet)

- **Epic:** Knowledge layer (context-engine) · **Status:** Accepted (direction) · **See:** [`docs/THESIS.md`](../docs/THESIS.md) §4.3, §8
- **Decision:** For the Knowledge layer, treat HKUDS's **Memory Governance Protocol
  (MGP)** as the **interop contract and vocabulary** to align to — its governed-memory
  lifecycle (`Write → Search → Get → Update → Expire → Revoke → Delete → Purge`),
  per-request policy context ("who acts, for whom, under what constraints"), and
  queryable audit map onto what we already mean by *governed recall* — **without**
  adopting its gateway/adapter stack as our runtime now. Align `GLOSSARY.md` and
  context-engine's *external surface* to MGP terms; keep our distinctive move internal
  and independent: the stochastic→deterministic **distillation border** (an LLM
  distills prose into typed Facts / Rules / Capsules at ingestion; deterministic
  governed recall). Speaking MGP on the wire is **gated on a concrete trigger** — a
  second consumer of context-engine that is not our own harness.
- **Alternatives:** (a) **adopt MGP as the knowledge-layer runtime now** — rejected:
  premature (context-engine has no external consumer yet, so importing a
  gateway/adapter stack is cost without a second speaker), and it would subordinate
  our distillation border to someone else's interface before it is proven; (b)
  **ignore MGP, grow vocabulary ad hoc** — rejected: MGP is the clearest existing
  articulation of "governed memory as a protocol," explicitly *peer to MCP*, so
  divergent vocabulary is needless drift; (c) **treat MGP as a competitor** —
  rejected: it standardizes the *interface* to governed memory while our contribution
  is the *distillation border behind it* — composable, not competing (MGP as wire
  contract, distillation as what sits behind it).
- **Why:** aligning vocabulary is near-zero cost and pays off in legibility and a
  clean future integration seam; adopting the protocol implementation is real cost
  that should wait for a real second consumer. Keeps *correctness > completeness*
  (THESIS §4.3) and avoids over-building the least-load-bearing seam.
