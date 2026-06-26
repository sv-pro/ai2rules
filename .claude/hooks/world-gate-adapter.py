#!/usr/bin/env python3
"""PreToolUse host adapter for the CLI Agent Harness gate ABI (DECISIONS D24/D26).

A THIN adapter — it contains NO governance logic. It maps a Claude Code PreToolUse
event onto a GateRequest, calls the real kernel via `harness gate --world
.claude/cc-world.yaml`, and maps the verdict back to a PreToolUse decision. Every
decision is the kernel's; this file only does host-shape translation:

  1. classify Bash by command shape -> Bash / Bash_network / Bash_destructive (D25),
  2. restore/persist monotonic taint in the per-session sidecar,
  3. shell out to `harness gate` and read the GateResponse,
  4. map decision -> PreToolUse (DENY->deny, ASK->ask, else passthrough), fail-open.

Successor to world-gate.py (which reimplemented the kernel in Python, contradicting
D22/D24). Validated in the E13.7 container SUT; the live host hook is left untouched
until a deliberate cutover (D26).

Config (env, all optional):
  HARNESS_BIN  path to `harness` (else $CLAUDE_PROJECT_DIR/target/{release,debug}/harness, else PATH)
  CC_WORLD     world manifest path (else $CLAUDE_PROJECT_DIR/.claude/cc-world.yaml)
"""
import json
import os
import re
import subprocess
import sys

# Bash command classification (D25). These PATTERNS are host-syntactic, not policy:
# the *policy* for each resulting action lives in cc-world.yaml.
EGRESS = ("curl ", "wget ", "nc ", "ncat ", "telnet ", "ssh ", "scp ", "sftp ")
DESTRUCTIVE = ("rm -rf", "rm -fr", "sudo ", "mkfs", "dd if=", ":(){")


def fail_open():
    """Any error -> allow (exit 0): a broken adapter must never brick a session."""
    sys.exit(0)


def emit(decision, reason):
    """Emit a PreToolUse decision (only used for deny/ask) and exit."""
    print(json.dumps({"hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": decision,
        "permissionDecisionReason": reason,
    }}))
    sys.exit(0)


def find_harness():
    if os.environ.get("HARNESS_BIN"):
        return os.environ["HARNESS_BIN"]
    proj = os.environ.get("CLAUDE_PROJECT_DIR", os.getcwd())
    for rel in ("target/release/harness", "target/debug/harness"):
        cand = os.path.join(proj, rel)
        if os.path.exists(cand):
            return cand
    return "harness"  # on PATH


def classify(tool, ti):
    """Map a host tool + input onto a manifest action name (the only place a host
    quirk — Bash being one tool with many effects — is normalized)."""
    if tool != "Bash":
        return tool
    cmd = str(ti.get("command", "") or "")
    if any(p in cmd for p in EGRESS):
        return "Bash_network"
    if any(p in cmd for p in DESTRUCTIVE):
        return "Bash_destructive"
    return "Bash"


def main():
    if sys.stdin.isatty():
        sys.stderr.write(
            "world-gate-adapter.py is a PreToolUse hook: feed a PreToolUse event "
            "JSON on stdin (see .claude/hooks/test-gate-adapter.sh).\n"
        )
        sys.exit(0)
    try:
        ev = json.loads(sys.stdin.read())
    except Exception:
        fail_open()

    tool = ev.get("tool_name", "")
    ti = ev.get("tool_input", {}) or {}
    if not isinstance(ti, dict):
        ti = {}
    sid = str(ev.get("session_id", "default"))

    proj = os.environ.get("CLAUDE_PROJECT_DIR", os.getcwd())
    world = os.environ.get("CC_WORLD") or os.path.join(proj, ".claude", "cc-world.yaml")
    state_dir = os.path.join(proj, ".claude", "state")
    taint_file = os.path.join(state_dir, "taint-" + re.sub(r"[^A-Za-z0-9_.-]", "_", sid))
    tainted = os.path.exists(taint_file)

    req = {
        "v": 1,
        "tool": classify(tool, ti),
        "arguments": ti,
        "context": {
            "session_id": sid,
            "mode": "interactive",
            "taint": "tainted" if tainted else "clean",
        },
    }

    try:
        proc = subprocess.run(
            [find_harness(), "gate", "--world", world],
            input=json.dumps(req),
            capture_output=True,
            text=True,
            timeout=10,
        )
    except Exception:
        fail_open()
    if proc.returncode != 0:
        fail_open()  # gate could not evaluate -> additive fail-open
    try:
        res = json.loads(proc.stdout)
    except Exception:
        fail_open()

    # Persist the kernel-computed monotonic taint for the next call.
    if res.get("context", {}).get("taint") == "tainted" and not tainted:
        try:
            os.makedirs(state_dir, exist_ok=True)
            with open(taint_file, "w") as f:
                f.write(f"tainted by {tool} ({req['tool']})\n")
        except Exception:
            pass  # never block on a state-write failure

    decision = res.get("decision", "ALLOW")
    # Additive: only ever deny/ask. ALLOW/ABSENT/REPLAN fall through to Claude
    # Code's normal permission flow (the adapter never auto-allows).
    if decision == "DENY":
        emit("deny", res.get("reason", ""))
    if decision == "ASK":
        emit("ask", res.get("reason", ""))
    sys.exit(0)  # passthrough


if __name__ == "__main__":
    main()
