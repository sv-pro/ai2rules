#!/usr/bin/env python3
"""Bootstrap shim (DECISIONS D37): exec the Rust kernel's PreToolUse adapter.

No governance logic lives here — the kernel (`harness cc-hook`) decides against
.claude/cc-world.yaml. Kept in place because hook configs may be snapshotted at
session start; new sessions use world-gate.sh. The Python engine it replaced is
archived in superseded/. Fail-open: no binary → exit 0 (stdin passes through on exec).

The project directory is untrusted input. Never resolve harness from
$CLAUDE_PROJECT_DIR/target; use an explicit absolute HARNESS_BIN/AI2RULES_HARNESS
override, or a standard installer-owned absolute path.
"""
import os, sys

pd = os.environ.get("CLAUDE_PROJECT_DIR", os.getcwd())

def executable(path):
    return path and os.path.isabs(path) and os.path.isfile(path) and os.access(path, os.X_OK)

binary = None
for env_name in ("HARNESS_BIN", "AI2RULES_HARNESS"):
    candidate = os.environ.get(env_name)
    if candidate:
        binary = candidate if executable(candidate) else None
        break

if binary is None:
    candidates = [
        os.path.join(os.path.expanduser("~"), ".local", "bin", "harness"),
        "/usr/local/bin/harness",
        "/opt/ai2rules/bin/harness",
    ]
    binary = next((c for c in candidates if executable(c)), None)

if binary is None:
    sys.exit(0)
os.execv(binary, [binary, "cc-hook",
                  "--world", os.path.join(pd, ".claude", "cc-world.yaml"),
                  "--state", os.path.join(pd, ".claude", "state")])
