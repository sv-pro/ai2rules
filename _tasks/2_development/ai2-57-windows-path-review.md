# AI2-57 / PR #129 — Windows path review fixes

2026-10-25: original review at44c0ec8 reproduced root-relative external target
classified as project-local (ALLOW instead of DENY) and absolute missing/..
not refused. Both fixed at shared hostkit boundary using native path components
and validation before canonicalization. No filesystem effects execute in probes.

Regression tests cover neutral path keys, rooted slash/backslash, ordinary and
verbatim absolute paths, drive-relative semantics, existing-directory parents,
missing parents, tilde, project-local ALLOW and external DENY, Unix symlink/...
Native Windows workspace tests and clippy -D warnings pass; fmt is clean.
Linux hostkit tests pass, including symlink/.. scope. Candidate review and fresh
CI must finish before the user-authorized merge. E8 filesystem isolation is not
claimed complete. Demo and PR150 remain untouched.
