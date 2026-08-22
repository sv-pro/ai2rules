# D74 — External finality is a receipt state after independent staged commit


**Context.** D73 closes authorization replay, but direct execution still collapses
deterministic admission, authorization, and external finality into one call. Real
payments, messages, broker orders, and cloud changes can time out after acceptance;
neither `ALLOW` nor an executor error tells us whether the effect exists. Retrying
that uncertainty can duplicate an irreversible action.

**Decision.** Introduce a minimal `StagedEffect`: a sealed, durable, explicitly
non-effective artifact binding proposal and admission identity, trusted principal,
kernel-classified action and complete normalized arguments, effect/resource,
world and schema epochs, `AuthorizationInstance`, effect mode, consequence class,
actuator operation, idempotency key, and expiry.

An independent commit coordinator reloads the signed artifact, revalidates its
hash and current compiled epochs, reconstructs the canonical effect, validates
consequence and actuator routing, and syncs `AttemptStarted` before it consumes
authorization or invokes the privileged actuator. Every attempt produces a signed
causal `ExecutionReceipt`. A started attempt without a terminal result, including
a timeout after possible downstream acceptance, is `ambiguous` and never retried
automatically. Duplicate terminal commits return a duplicate receipt and create at
most one fake downstream effect across restart.

Receipt outcomes (`committed`, `simulated`, `failed`, `rejected`, `duplicate`,
`ambiguous`) are execution-finality states, **not new kernel verdicts**. SIMULATE
uses the same commit validations and consumes its exact authorization but the
actuator creates no external effect.

**Alternatives.** Executing immediately after `ALLOW` was rejected for final
effects because admission is not finality. Retrying after timeout was rejected
because absence of a response is not evidence of absence. A distributed
transaction framework was rejected as premature; the spike proves the artifact,
reservation, actuator, and receipt seams first. Treating finality as new policy
verdicts was rejected because it would mix two different state machines.

**Consequence.** The prototype offers at-most-once/fail-closed finality with a
fake independently-held actuator and durable causal evidence. Production use still
requires protected actuator credentials and downstream idempotency. The API is not
yet wired into the default agent loop; AI2-7 can now exercise it in TradingWorld.
