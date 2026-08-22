# D69 — The source channel is declared by the operator, because no host will tell us


**Date:** 2026-08-14. **Closes finding #22/#21.**

- **The complaint was fair.** Both live adapters hardcoded `source_channel: "user_prompt"`, the
  most-trusted channel, for every call — including one the model proposed immediately after
  reading a poisoned file. The gate has careful machinery here (`parse_channel` fails closed on
  an unknown channel, with a comment about thin adapters not upgrading an unknown proposer),
  and both real hosts bypassed all of it with a constant.
- **It cannot be derived, and that is a fact about the hosts rather than a shortcut.** A Claude
  Code PreToolUse event carries `tool_name`, `tool_input`, `session_id`, `cwd`,
  `permission_mode`. An Antigravity one carries `toolCall`, `modelName`, `stepIdx`,
  `workspacePaths`, `transcriptPath`. Both describe *what is about to run*; neither says who
  asked for it. An adapter that claimed to know would be guessing, and a guess that upgrades
  trust is the worst possible direction to guess in.
- **So it is declared.** Both adapters take `--source-channel`, defaulting to `user_prompt` so
  nothing changes for an existing install. Its value is that an unattended or background
  session can be run at a lower trust: measured against the live `.claude/cc-world.yaml`, a
  `Write` is `allow` under the default and **ABSENT** under `--source-channel web_fetch`, while
  a `Read` still passes — the capability matrix shrinking exactly as the manifest says it
  should. An undeclared channel still fails closed.
- **What this does not pretend to be.** With the default left alone, channel trust does no
  work. The control that actually catches "the model read something poisoned and then tried to
  send it somewhere" is data-flow taint, which is enforced regardless of this field. Saying so
  is better than the previous state, where the code implied a control that was inert.
- **Alternatives rejected.**
  - *Infer the proposer from the transcript* (both hosts do provide a path to it). It means
    parsing an undocumented, host-internal format, doing file I/O on the hook's hot path, and
    re-deriving a guess every call — to produce something the host could simply have told us.
    If a host ever reports the proposer, deriving becomes a two-line change.
  - *Default to a lower-trust channel.* Honest-looking and immediately wrong: it would silently
    remove capabilities from every existing install, and "the model proposed it" is true of
    every call in a hook-based integration, so the label conveys nothing while the capability
    loss is real.
  - *Use `stepIdx` as a proxy* (step 0 ≈ closest to the user's request). A heuristic dressed as
    provenance, and trivially wrong for a multi-step task the user explicitly asked for.
  - *Remove `source_channel` from the ABI.* The in-process orchestrator does know real
    provenance per perception, and the field is doing genuine work there.
- **Related:** D24 (the gate ABI), D37, D48; `crates/cli-harness/src/{cc_hook,agy_hook}.rs`.
