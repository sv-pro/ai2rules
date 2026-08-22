# D44 — Shell command classifiers fail closed on unmatched commands


**Date:** 2026-07-23.

- **Context:** A security review found that the D36 command classifiers were still
  substring-pattern lists over raw shell strings. Shell-equivalent whitespace (`curl\t...`,
  `sudo\t...`) and unlisted egress/destructive programs could stay as the raw `Bash`/`run_command`
  `Process` action, bypassing both the network taint floor and approval.
- **Decision:** Keep command classification as compiled world data, but make shell classifiers
  fail closed. `CommandClassDef` now supports `default_to`, used when the command argument is
  missing/malformed or no pattern matches. Shipped shell worlds route unmatched Bash/run_command to
  an approval-required, `Network`-effectful unclassified action; tainted unmatched shell is denied
  by the hard taint floor, and clean unmatched shell asks. Pattern whitespace now matches shell
  whitespace, so `curl\t...`, `rm\t-rf`, and `sudo\t...` hit their intended classes.
- **Why:** We cannot prove arbitrary shell strings are local or harmless with a finite pattern list.
  The safe default is to make unknown shell non-ambient while preserving structured/scoped commands
  such as `run_tests`, `git_status`, and host-native read/write tools.
- **Alternatives rejected:** keep extending the pattern list (always incomplete); treat unmatched
  shell as plain `Process` (the reviewed bypass); route every shell call through a new OS sandbox in
  this patch (larger substrate work and not required to close the current gate bypass).
- **Related:** D24 (host-neutral gate ABI), D36 (kernel-side classification), D37 (live-hook
  cutover), D42 (explicit fail-closed gate inputs), D43 (compiled manifest policy).
