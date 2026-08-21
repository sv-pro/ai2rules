# StagedEffect and independent external commit

Status: **prototype shipped by AI2-8**. Decision: `DECISIONS.md` **D74**.

## Contract

```text
proposal + deterministic admission
        ↓
StagedEffect (non-effective, signed durable evidence)
        ↓
commit(staged artifact, current world, AuthorizationInstance)
        ↓
durable idempotency reservation → authorization consumption
        ↓
privileged actuator → ExecutionReceipt
```

`StagedEffect` binds the proposal/admission identities, trusted principal,
kernel-classified action, complete normalized arguments and effect digest,
resource, world and schema epochs, authorization identity, effect mode,
consequence class, exact actuator operation, idempotency key, and expiry. Its
versioned SHA-256 seals all of those fields. Persisting it performs no operation.

The commit coordinator is independent of model reasoning. Under a cross-process
lock it reloads signed evidence, checks that the caller presented the exact
persisted artifact, verifies current compiled epochs and artifact expiry,
reconstructs the canonical effect binding, checks actuator routing, and durably
records `AttemptStarted`. Only then does it atomically consume the bound
authorization and call the actuator.

## Finality states are not verdicts

`ExecutionReceipt.outcome` is one of:

- `committed` — downstream application returned a known result;
- `simulated` — the commit path ran, but the actuator produced no effect;
- `failed` — the actuator reported a known pre-finality failure;
- `rejected` — artifact, epoch, authorization, or routing validation failed;
- `duplicate` — a terminal receipt already owns the idempotency key;
- `ambiguous` — application may have happened, so automatic retry is refused.

These do not add `COMMITTED`, `FAILED`, or `AMBIGUOUS` to the kernel's
`ALLOW/DENY/ABSENT/ASK/REPLAN` vocabulary. Admission and external finality answer
different questions.

## Crash and retry semantics

The coordinator writes and syncs `AttemptStarted` before consuming authorization
or invoking the actuator. If the process disappears after that record, recovery
reports `ambiguous` and will not invoke the actuator again. A timeout after
downstream acceptance follows the same rule. This is at-most-once/fail-closed;
it deliberately does not claim distributed exactly-once delivery.

The fake privileged actuator also records applied idempotency keys, illustrating
the stronger deployment pattern: both coordinator and downstream sink enforce
idempotency. A production actuator would require a protected process/account and
a downstream idempotency contract.

## Scope

The prototype is a reusable Rust API in `trace-store`; it is not yet wired into
the default model loop or a provider-specific approval UI. It uses no real
payment, mail, broker, or cloud credentials and is not a workflow engine.
