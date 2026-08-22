# D66 — The audit log scans values for secrets, and refuses to guess


**Date:** 2026-08-14. **Closes finding #17.**

- **The hole.** Redaction matched a field's **key** or dotted path against the manifest's
  `observability.redact` patterns. Every secret that arrives inside an ordinary value had no
  matching key and went to the trace verbatim: a bearer token in `command`, an `api_key` in a
  `url`, a password in a `git clone` URL, a private key echoed into a file.
- **Why the obvious fix is wrong.** Adding `command`, `url` and `body` to the default redact
  patterns would mask the whole value. A trace in which every command reads `[REDACTED]` is not
  an audit log; it is a log that something happened. The value has to survive and the secret
  inside it has to go, which means scanning the string and masking only the offending span.
- **The detectors are few, and that is the design.** Each is either an issuer-defined shape
  (`ghp_`, `github_pat_`, `AKIA`/`ASIA`, `AIza`, `sk-`/`sk-ant-`, `xox*-`, `glpat-`, `npm_`, a
  JWT header segment, a PEM private-key envelope) or a syntactic position that is a secret by
  definition (an `Authorization`/`Cookie` header value, `?token=`, the password half of URL
  userinfo). High-entropy heuristics and bare hex runs are excluded.
- **Because a redactor that guesses fails twice.** It corrupts the audit record, and it teaches
  the reader that `[REDACTED]` is noise. A test asserts the negative case explicitly — ordinary
  commands, `task-force`, `?page=2`, `my_secret=` outside a query position all pass through
  byte-identical.
- **A safety net, not a replacement.** Naming your secrets in `observability.redact` is still
  better than hoping a scanner recognises them; this catches what the manifest did not name.
- **Alternatives rejected.**
  - *Add `command`/`url`/`body` to the default patterns.* Destroys the audit value of exactly
    the fields an auditor most needs.
  - *Entropy scoring over every string.* Flags base64 payloads, hashes, UUIDs and minified
    code; the false-positive rate is the failure mode.
  - *A regex engine.* Not in the offline crate set, and for a security control that must be
    read and trusted, an explicit scanner is easier to audit than a pattern table.
  - *Mask at write time in the store only.* Redaction belongs on the value before it reaches
    any sink, including bundles and replay.
- **Related:** E4.2, invariant 15; `crates/trace-store/src/{secrets,redact}.rs`.
