#!/usr/bin/env python3
"""PreToolUse gate — a first slice of the CLI Agent Harness kernel ported onto the
Claude Code host (PLAN.md E13.2).

It reads a Claude Code PreToolUse event on stdin and a small WorldManifest
(.claude/cc-world.json) and ports three of the kernel's signature behaviours:

  1. ABSENT-for-native  — deny native tools not in the projected set (opt-in).
  2. Taint floor        — once the context is tainted (a tainted file/web result
                          was read), deny tools that can reach the network/egress.
  3. ASK                — pause for approval before destructive commands.

Design notes mirroring the real kernel:
  * Decisions are a pure function of (event, manifest, taint state) — no LLM.
  * The gate is ADDITIVE: it only ever returns "deny"/"ask"; it never returns
    "allow" (which would bypass your normal permission prompts). Anything it does
    not match falls through to Claude Code's usual flow.
  * It FAILS OPEN: any parse/IO error exits 0 (allow), so a broken manifest can
    never brick a session.
  * Taint is MONOTONIC and per-session, persisted in a sidecar file under
    .claude/state/ (gitignored) — the cross-turn analogue of TaintContext.

Config path: $CC_WORLD_CONFIG, else $CLAUDE_PROJECT_DIR/.claude/cc-world.json,
else ./.claude/cc-world.json. State dir: $CLAUDE_PROJECT_DIR/.claude/state.
"""
import sys, os, re, json

NATIVE = {"Bash", "Read", "Edit", "Write", "MultiEdit", "NotebookEdit",
          "Glob", "Grep", "WebFetch", "WebSearch"}


def emit(decision, reason):
    """Emit a PreToolUse decision and exit. Only used for deny/ask."""
    print(json.dumps({"hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": decision,
        "permissionDecisionReason": reason,
    }}))
    sys.exit(0)


def usage():
    sys.stderr.write(
        "world-gate.py is a Claude Code PreToolUse hook: it is invoked as\n"
        "`python3 world-gate.py` with a PreToolUse event JSON on stdin — not by\n"
        "hand, and not with `bash` (this is Python, not a shell script).\n\n"
        "Exercise it instead with:\n"
        "  bash .claude/hooks/test-gate.sh\n"
        "  echo '{\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"ls\"}}' "
        "| python3 .claude/hooks/world-gate.py\n"
    )


def main():
    if any(a in ("-h", "--help") for a in sys.argv[1:]) or sys.stdin.isatty():
        usage()
        sys.exit(0)
    raw = sys.stdin.read()
    try:
        ev = json.loads(raw)
    except Exception:
        sys.exit(0)  # fail open

    tool = ev.get("tool_name", "")
    ti = ev.get("tool_input", {}) or {}
    if not isinstance(ti, dict):
        ti = {}
    sid = str(ev.get("session_id", "default"))

    # Opt-in event log for debugging/demos (off by default): set CC_GATE_DEBUG=1
    # or `touch .claude/state/debug-on`. Captures the raw event JSON of every tool
    # call the gate sees — useful for inspecting parent vs subagent sessions.
    _pd = os.environ.get("CLAUDE_PROJECT_DIR", os.getcwd())
    if os.environ.get("CC_GATE_DEBUG") or os.path.exists(os.path.join(_pd, ".claude", "state", "debug-on")):
        try:
            _sd = os.path.join(_pd, ".claude", "state")
            os.makedirs(_sd, exist_ok=True)
            with open(os.path.join(_sd, "debug.log"), "a") as _f:
                _f.write(raw.rstrip("\n") + "\n")
        except Exception:
            pass

    proj_dir = os.environ.get("CLAUDE_PROJECT_DIR", os.getcwd())
    cfg_path = os.environ.get("CC_WORLD_CONFIG") or os.path.join(proj_dir, ".claude", "cc-world.json")
    try:
        with open(cfg_path) as f:
            cfg = json.load(f)
    except Exception:
        sys.exit(0)  # no manifest → allow all

    state_dir = os.path.join(proj_dir, ".claude", "state")
    taint_file = os.path.join(state_dir, "taint-" + re.sub(r"[^A-Za-z0-9_.-]", "_", sid))
    tainted = os.path.exists(taint_file)

    cmd = str(ti.get("command", "") or "")
    url = str(ti.get("url", "") or "")
    path = ""
    for k in ("file_path", "path", "notebook_path"):
        if ti.get(k):
            path = str(ti[k])
            break

    # 1) ABSENT-for-native — only when a projected set is declared.
    proj = cfg.get("projected_tools")
    if isinstance(proj, list) and tool in NATIVE and tool not in proj:
        emit("deny", f"ABSENT: '{tool}' is not projected into this world "
                     f"(declare it in .claude/cc-world.json to make it exist).")

    # 2) Taint floor — tainted context cannot drive egress.
    if tainted:
        eg = cfg.get("egress", {}) or {}
        if tool in eg.get("tools", []):
            emit("deny", f"taint floor: context is tainted; '{tool}' can reach the "
                         f"network and is blocked (rule no_tainted_network).")
        if tool == "Bash" and any(p in cmd for p in eg.get("bash_patterns", [])):
            emit("deny", "taint floor: context is tainted; this command performs "
                         "network egress and is blocked (rule no_tainted_network).")

    # 3) ASK — approval before destructive actions.
    ask = cfg.get("ask", {}) or {}
    if tool in ask.get("tools", []):
        emit("ask", f"approval required before '{tool}'.")
    if tool == "Bash" and any(p in cmd for p in ask.get("bash_patterns", [])):
        emit("ask", "approval required: potentially destructive command.")

    # Side effect: taint the session (monotonic) for the *next* call.
    ts = cfg.get("taint_sources", {}) or {}
    will_taint = tool in ts.get("tools", [])
    if not will_taint and tool in ("Read", "Glob", "Grep", "NotebookEdit") and path:
        will_taint = any(prefix in path for prefix in ts.get("read_paths", []))
    if will_taint and not tainted:
        try:
            os.makedirs(state_dir, exist_ok=True)
            agent = ev.get("agent_type", "main")
            with open(taint_file, "w") as f:
                f.write(f"tainted by {tool} (agent: {agent}): {path or url}\n")
        except Exception:
            pass  # never block on a state write failure

    sys.exit(0)  # fall through to normal permission flow


if __name__ == "__main__":
    main()
