#!/usr/bin/env python3
"""SubagentStop hook (PLAN.md E13.6) — make cross-agent taint propagation explicit
and observable.

Claude Code shares ONE session_id across an in-process agent tree, so taint keyed
by session_id (see world-gate.py) already propagates parent<->subagent through the
shared sidecar. This hook hardens that:

  * If a *distinct* parent session is identifiable (isolated/background agents that
    get their own session_id), union the child's taint into the parent's sidecar.
  * Surface taint to the model + user when a subagent finishes, so the floor isn't
    silent (audit / observability).

Honors the same opt-in debug log as world-gate.py (CC_GATE_DEBUG / debug-on).
Fails open: any error exits 0.
"""
import sys, os, re, json

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
try:
    from _gatelib import is_tainted as _is_tainted, load_pins as _load_pins, load_cfg as _load_cfg
except Exception:  # fail open: degrade to existence-based taint if helper is missing
    _is_tainted = None


def state_dir():
    pd = os.environ.get("CLAUDE_PROJECT_DIR", os.getcwd())
    return os.path.join(pd, ".claude", "state")


def taint_path(sd, sid):
    return os.path.join(sd, "taint-" + re.sub(r"[^A-Za-z0-9_.-]", "_", str(sid)))


def main():
    raw = sys.stdin.read()
    try:
        ev = json.loads(raw)
    except Exception:
        sys.exit(0)

    sd = state_dir()
    if os.environ.get("CC_GATE_DEBUG") or os.path.exists(os.path.join(sd, "debug-on")):
        try:
            os.makedirs(sd, exist_ok=True)
            with open(os.path.join(sd, "debug.log"), "a") as f:
                f.write(raw.rstrip("\n") + "\n")
        except Exception:
            pass

    sid = ev.get("session_id")
    # Defensive: field name for the parent link isn't guaranteed across versions.
    parent = ev.get("parent_session_id") or ev.get("parent_id") or ev.get("parent_tool_use_session_id")
    # Same taint truth as world-gate.py: a ledger covered entirely by valid
    # trust_pins is NOT tainted (D29). Degrade to existence only if helper missing.
    pd = os.environ.get("CLAUDE_PROJECT_DIR", os.getcwd())
    tf = taint_path(sd, sid)
    if _is_tainted is not None:
        child_tainted = bool(sid) and _is_tainted(tf, _load_pins(_load_cfg(pd)), pd)
    else:
        child_tainted = bool(sid) and os.path.exists(tf)

    # Distinct-session case (isolated/background): union child taint into parent.
    if child_tainted and parent and parent != sid:
        try:
            os.makedirs(sd, exist_ok=True)
            with open(taint_path(sd, parent), "a") as f:
                f.write(f"propagated from subagent session {sid}\n")
        except Exception:
            pass

    # Observability: don't let the taint floor be silent.
    if child_tainted:
        print(json.dumps({
            "systemMessage": "⚠ Session tainted: a subagent read untrusted data. "
                             "Network egress is now blocked by the world-gate taint floor.",
            "hookSpecificOutput": {
                "hookEventName": "SubagentStop",
                "additionalContext": "This session is TAINTED (a subagent ingested "
                "untrusted data). Per the deterministic taint floor, egress tools "
                "(WebFetch, curl/wget, etc.) will be denied until the taint sidecar "
                "is cleared.",
            },
        }))
    sys.exit(0)


if __name__ == "__main__":
    main()
