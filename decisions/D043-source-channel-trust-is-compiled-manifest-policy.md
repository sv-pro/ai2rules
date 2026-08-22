# D43 — Source-channel trust is compiled manifest policy


**Date:** 2026-07-23.

- **Context:** A security review found that `harness gate` accepted explicit
  `context.source_channel`, but then mapped it through hard-coded `SourceChannel` enum defaults.
  That meant a manifest row such as `workspace_files: Untrusted, taint:true` could be silently
  upgraded to the enum's legacy workspace-file trust, allowing actions the manifest's capability
  matrix intended to hide.
- **Decision:** Compile `channels:` into `CompiledWorld` as the runtime source-channel policy.
  Gate requests resolve their wire `source_channel` through that compiled table; the resulting
  manifest trust drives capability checks, and the manifest channel taint is joined into the
  carried taint before the kernel decides. Unknown channel names and duplicate aliases are rejected
  at manifest validation; undeclared runtime channels fail closed as `invalid_source_channel`.
  Manifests with no `channels:` keep legacy defaults for compatibility.
- **Why:** Channel trust is world data, not enum physics. The pure gate can stay host-neutral while
  still enforcing the policy the operator authored in the manifest.
- **Alternatives rejected:** keep enum trust as the runtime authority (the reviewed bypass); make
  every caller hand-build trusted `Provenance` (duplicates policy outside the world); require every
  legacy or test manifest to declare channels immediately (unnecessary compatibility break).
- **Related:** D24 (host-neutral gate ABI), D25 (read-taint source model), D42 (explicit gate
  context), trust pins / channel reclassification model.
