#!/usr/bin/env python3
"""Bootstrap shim (DECISIONS D37): exec the Rust kernel's PreToolUse adapter.

No governance logic lives here — the kernel (`harness cc-hook`) decides against
.claude/cc-world.yaml. Kept in place because hook configs may be snapshotted at
session start; new sessions use world-gate.sh. The Python engine it replaced is
archived in superseded/. Fail-open: no binary → exit 0 (stdin passes through on exec).
"""
import os, shutil, sys

pd = os.environ.get("CLAUDE_PROJECT_DIR", os.getcwd())
# Kill-switch parity with world-gate.sh (rollback layer #1): a gate-off sentinel
# disables governance for the next call. Additive fallback path — no --grant here.
if os.path.exists(os.path.join(pd, ".claude", "gate-off")) or \
   os.path.exists(os.path.expanduser("~/.claude/gate-off")):
    sys.exit(0)
candidates = [os.environ.get("HARNESS_BIN"),
              os.path.join(pd, "target", "release", "harness"),
              os.path.join(pd, "target", "debug", "harness"),
              shutil.which("harness")]
binary = next((c for c in candidates if c and os.path.isfile(c) and os.access(c, os.X_OK)), None)
if binary is None:
    sys.exit(0)
os.execv(binary, [binary, "cc-hook",
                  "--world", os.path.join(pd, ".claude", "cc-world.yaml"),
                  "--state", os.path.join(pd, ".claude", "state")])
