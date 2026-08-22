# D73 — ASK authorization is an exact, durable, single-use claim made before effect


**Context.** The original `ApprovalToken` was durably signed and bound to action,
arguments, manifest, descriptor, provenance, and effect mode, but resumption used
the non-atomic sequence `is_granted → execute → mark_executed`. Two runtimes could
therefore observe the same Approved token, both execute, and only then record
completion. A crash in that window also resurrected the grant after restart. The
artifact did not name a principal, resource, expiry, or consumption budget.

**Decision.** The canonical artifact is now `AuthorizationInstance` (with
`ApprovalToken` retained as a source-compatible alias). It binds a trusted-runtime
principal, normalized action and complete JSON arguments, a stable resource label,
world id + compiled-manifest epoch, effective-descriptor schema epoch, provenance,
effect mode, exclusive expiry, and exactly one remaining use. A versioned canonical
effect envelope is serialized deterministically and SHA-256 hashed.

The signed append-only approval store serializes all mutations with a cross-process
lock, reloads state under that lock, and appends `Consumed` before the external
effect. Mismatches append signed `ConsumptionRejected` evidence with a specific
reason. A crash after consumption burns the authorization: recovery is
at-most-once and fail-closed, not an unsafe promise of exactly-once external I/O.
After a successful claim the trusted orchestrator re-runs the pure kernel, so the
artifact may satisfy `ASK` but cannot turn `DENY` or `ABSENT` into authority.

The public Gate ABI remains decision-only. Its `approval_token` is still a
correlation id (D41); a request cannot self-assert a grant. Store verification and
consumption belong to a trusted runtime boundary with durable state.

**Alternatives.** Keeping lookup and completion as separate operations was rejected
because it leaves both concurrent replay and restart replay. Consuming after the
effect was rejected for the same reason. Reserving and later rolling back on error
was rejected because a crash could make rollback unknowable; burning the claim is
the only deterministic fail-closed recovery without a transactional effect sink.
Moving verification into `harness gate` was rejected because it would add I/O and
host identity assumptions to the pure, reusable decision ABI.

**Consequence.** One authorization admits at most one attempt at its exact effect.
Substitution, cross-principal/session reuse, expiry, epoch drift, concurrent reuse,
and restart replay fail closed with durable evidence. Callers needing retry must
obtain a fresh human authorization; later staged-effect work can pair the same
claim with a transactional sink rather than weakening this invariant.
