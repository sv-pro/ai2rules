# D42 — Gate context fields are explicit fail-closed inputs


**Date:** 2026-07-23.

- **Context:** A security review found three fail-open defaults at the host-neutral gate boundary:
  missing or malformed `context.taint` became clean, missing or malformed
  `context.source_channel` became trusted user input, and roots-enabled file actions with no
  adapter-resolved `path` skipped spatial scope. These were intended as adapter conveniences, but
  they made thin or drifted adapters silently drop the controls that make the gate meaningful.
- **Decision:** Keep the v1 wire fields, but make them explicit security inputs:
  `context.taint` must be `clean` or `tainted`; `context.source_channel` must be one of the
  recognized source aliases; and when roots are enabled, path-scoped file actions must carry the
  adapter-resolved absolute `path`. Missing or malformed context returns an evaluated `DENY` with a
  specific rule (`missing_taint`, `invalid_taint`, `missing_source_channel`,
  `invalid_source_channel`, or `missing_path`). Non-file actions such as Bash remain path-exempt.
- **Why:** The pure gate cannot reconstruct omitted host context safely. Failing closed at the ABI
  boundary preserves the one-kernel model while forcing adapters to prove what they know instead of
  receiving trusted defaults.
- **Alternatives rejected:** continue defaulting to clean/user-prompt/no-path (the reviewed
  bypass); make malformed requests process errors (would push fail-open/fail-closed policy back
  into each adapter); require paths for every `Read` action (breaks non-filesystem reads such as MCP
  queries in worlds that also use roots).
- **Related:** D24 (host-neutral gate ABI), D36 (kernel-side classification), D37 (live-hook
  cutover), D41 (approval tokens are not grants), roots path-scope hardening.
