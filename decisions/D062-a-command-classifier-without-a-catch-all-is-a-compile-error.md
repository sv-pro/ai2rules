# D62 — A command classifier without a catch-all is a compile error, not an author's choice


**Date:** 2026-08-14. **Closes findings #19 and #20.**

- **What the classifier actually buys.** `command_classes` matches patterns against a shell
  command line, and that is a heuristic: `"curl" http://x`, `curl$IFS'...'` and
  `echo <base64> | base64 -d | sh` all evade the pattern lists, by construction and forever.
  The security property was never "the patterns catch everything". It is **where an unmatched
  command lands** — `default_to`, which every shipped manifest points at an
  approval-required, network-effectful class. Evasion downgrades into the stricter bucket.
- **Which made the field's optionality the real hole.** Omit `default_to` and an unmatched
  command falls back to the *raw* action — for a `bash`-shaped action, typically ambient
  `Process`, which no transition policy denies. Measured: a world declaring classifiers with no
  `default_to` compiles clean, denies `curl http://x` in a tainted session, and **allows**
  `python3 -c 'import socket…'` in the same session. The safety of the whole mechanism rested
  on a field the schema treated as decoration.
- **Also: one classifier per action.** `classify_command` resolves with `.find()`, so a second
  entry for the same action silently never runs — and splitting a long pattern list across two
  blocks is exactly how an author would reach for that. Channels and base actions were already
  duplicate-checked; classifiers were the gap in the same family.
- **The decision.** `validate()` now rejects both. The `default_to` check runs *last* in the
  per-classifier pass, so a manifest with several problems still reports the more specific one
  first.
- **Alternatives rejected.**
  - *Default `default_to` to the most restrictive declared class.* Silently correct, and it
    guesses. "Most restrictive" is a judgement about the author's ontology that the compiler
    should not be making on their behalf, and a manifest that reads as permissive while
    behaving otherwise is its own hazard.
  - *Warn instead of erroring.* Warnings on a governance manifest get read once. The failure
    mode being prevented is arbitrary shell in a tainted session.
  - *Leave it and document the requirement.* This is the third finding in this family where
    prose stated an invariant nothing enforced (see D60, D61). Documentation was already
    available and had not helped.
  - *Accept the breaking change quietly.* It is breaking for any third-party manifest that
    omitted the field — those manifests are exactly the vulnerable ones, and the error text
    names the field and explains the consequence.
- **Related:** D36 (kernel-side classification), D44; finding #17 (class *ordering* is a
  separate hazard in the same surface and remains open);
  `crates/compiler/src/loader.rs`.
