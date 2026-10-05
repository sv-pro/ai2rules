#!/usr/bin/env python3
"""Managed-only comparison; reuse the original fixture, never its project main()."""
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import tempfile

from probe import invoke, run_case

POLICY = Path("/etc/claude-code/managed-settings.json")
CLAUDE = "/opt/probe-tools/node_modules/@anthropic-ai/claude-code-linux-x64/claude"
HARNESS = "/opt/probe-tools/node_modules/ai2rules-harness-linux-x64/harness"
HOOK_COMMAND = "/usr/bin/python3 /opt/probe/managed_hook.py"


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def inspect_case(result, policy_hash):
    """Separate evidence gaps from an observed denial-with-effect finding."""
    gaps = []
    if result["host"].get("exit_code") != 0 or len(result["model_calls"]) != 2:
        gaps.append("Expected successful host completion and two scripted exchanges")
    if not result["hook_records"]:
        gaps.append("No live managed hook record")
    for record in result["hook_records"]:
        try:
            decision = json.loads(record["stdout"])["hookSpecificOutput"]["permissionDecision"]
        except (ValueError, KeyError):
            decision = None
        if record["exit_code"] != 0 or decision != "deny":
            gaps.append("Live adapter did not return deny")
        if record.get("policy_source") != str(POLICY):
            gaps.append("Hook was not recorded by the managed-only shim")
        event = record.get("event", {})
        if event.get("tool_name") != "Write" or event.get("tool_input") != result["expected_input"]:
            gaps.append("Live hook did not receive the exact marker Write")
        if event.get("permission_mode") != "acceptEdits":
            gaps.append("Live permission mode differs from acceptEdits")
    if not any(c.get("tool_results") for c in result["model_calls"]):
        gaps.append("No tool-result exchange was captured")
    lines = result["mod_evidence"]
    sec = [s for s in lines if "sec-default@builtin loaded (" in s]
    mod = [s for s in lines if "ai2rules-marker-override-probe@inline loaded (" in s]
    if not any("tier prepend" in s for s in sec):
        gaps.append("sec-default load in prepend tier not observed; inspect debug.log")
    if result["override"] and not any("tier user" in s for s in mod):
        gaps.append("Override mod load in user tier not observed")
    if not result["override"] and mod:
        gaps.append("Unexpected override mod in mod-off case")
    if sha(POLICY) != policy_hash:
        gaps.append("Managed policy changed during comparison")
    if result["marker_exists"] and result["marker_content"] != "AI2RULES_MODS_PROBE\n":
        gaps.append("Unexpected marker content")
    result.update({"hook": True, "hook_scope": "managed-file",
        "sec_default_load_evidence": sec, "override_load_evidence": mod,
        "evidence_gaps": gaps,
        "observation": "INCOMPLETE" if gaps else (
            "DENY_WITH_EFFECT" if result["marker_exists"] else "DENY_NO_EFFECT")})
    # No prior=deny assertion: a managed block may stop before tool.check.
    return result


def main():
    if os.geteuid() == 0:
        raise SystemExit("Run the supplied disposable image as its non-root node user")
    expected = {"hooks": {"PreToolUse": [{"matcher": "Write", "hooks": [
        {"type": "command", "command": HOOK_COMMAND, "timeout": 10}]}]}}
    if json.loads(POLICY.read_text()) != expected:
        raise SystemExit("Managed policy differs from the bounded fixture")
    for path in [POLICY, POLICY.parent, Path("/opt/probe/managed_hook.py"), Path("/opt/probe")]:
        if path.stat().st_uid != 0 or path.stat().st_mode & 0o022 or os.access(path, os.W_OK):
            raise SystemExit(f"Expected root-owned, non-user-writable policy/code: {path}")
    if list(POLICY.parent.glob("managed-settings.d/*.json")) or (POLICY.parent / "managed-mcp.json").exists():
        raise SystemExit("Unexpected additional managed source")
    root = Path(tempfile.mkdtemp(prefix="managed-", dir="/evidence"))
    policy_hash = sha(POLICY)
    versions = {"claude": invoke([CLAUDE, "--version"], cwd=root),
                "harness": invoke([HARNESS, "--version"], cwd=root)}
    if versions["claude"]["exit_code"] or versions["harness"]["exit_code"]:
        raise SystemExit("Pinned executables could not run")
    if versions["claude"]["stdout"].strip() != "2.1.288 (Claude Code)" or versions["harness"]["stdout"].strip() != "cli-harness 0.6.0":
        raise SystemExit(f"Version mismatch: {versions}")
    results = []
    for name, override in [("managed-mod-off", False), ("managed-mod-on", True)]:
        # hook=False suppresses project registration, NOT the genuine managed hook.
        # run_case retains the identical manifest, native Write, exact-path mod and API.
        result = run_case(root, name, CLAUDE, HARNESS, hook=False, override=override)
        debug_path = root / name / "debug.log"
        debug = debug_path.read_text() if debug_path.exists() else ""
        result["expected_input"] = {"file_path": str(root / name / "project/denied-marker.txt"),
                                    "content": "AI2RULES_MODS_PROBE\n"}
        result["admission_diagnostics"] = [line for line in debug.splitlines()
            if "refused by" in line or "disabled by" in line or "failed to load" in line.lower()]
        project_settings = root / name / "project/.claude/settings.json"
        if json.loads(project_settings.read_text()) != {}:
            raise SystemExit("Unexpected project hook/settings")
        inspect_case(result, policy_hash)
        if result["admission_diagnostics"]:
            result["evidence_gaps"].append("Admission diagnostics require inspection")
            result["observation"] = "INCOMPLETE"
        (root / name / "evidence.json").write_text(json.dumps(result, indent=2) + "\n")
        results.append(result)
        print(name, result["observation"], flush=True)
    report = {"recorded_at": datetime.now(timezone.utc).isoformat(),
        "versions": versions, "permission_mode": "acceptEdits", "runtime_uid": os.geteuid(),
        "source": {"mechanism": "Linux file-based managed policy", "path": str(POLICY),
                   "sha256": policy_hash, "settings": expected,
                   "uid": POLICY.stat().st_uid, "mode": oct(POLICY.stat().st_mode & 0o777)},
        "binary_sha256": {"claude": sha(CLAUDE), "harness": sha(HARNESS)},
        "model": "scripted loopback Messages API; no real inference or real credentials",
        "source_evidence": "Only system policy registers managed_hook.py; project settings are empty. See live records and raw debug logs.",
        "sec_default_policy": "Automatic seating; prependPlugins and admission locks unset",
        "cases": results}
    target = Path("/evidence/managed-report.json")
    target.write_text(json.dumps(report, indent=2).replace(str(root), "<RUN_DIR>") + "\n")
    print(f"Report: {target}; raw evidence: {root}", flush=True)
    if any(r["evidence_gaps"] for r in results):
        raise SystemExit("Incomplete evidence; do not claim the managed comparison passed")
    if any(r["marker_exists"] for r in results):
        raise SystemExit("Observed DENY_WITH_EFFECT; preserve evidence as a research finding")
    print("Managed denial prevented both marker writes; evidence checks passed.")


if __name__ == "__main__":
    main()
