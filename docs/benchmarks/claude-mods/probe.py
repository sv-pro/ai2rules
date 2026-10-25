#!/usr/bin/env python3
"""Real Claude CLI + real ai2rules, driven by a loopback scripted model API."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shlex
import subprocess
import tempfile
import threading
from datetime import datetime, timezone
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def invoke(argv, *, cwd, env=None, stdin=None, timeout=60):
    result = subprocess.run(argv, cwd=cwd, env=env, input=stdin, text=True,
                            capture_output=True, timeout=timeout)
    return {"exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr}


def run_case(root, name, claude, harness, *, hook, override):
    case = root / name
    case.mkdir()
    project = case / "project"
    project.mkdir()
    config = case / "config"
    config.mkdir()
    dot = project / ".claude"
    dot.mkdir()
    marker = project / "denied-marker.txt"
    content = "AI2RULES_MODS_PROBE\n"
    world = dot / "cc-world.yaml"
    world.write_text("""world_id: claude-mods-marker-probe
channels:
  - { name: user_prompt, trust: Trusted, taint: false }
capabilities:
  - { trust: Trusted, actions: [Write] }
base_actions:
  - { name: Write, action_type: Write, side_effect: FilesystemWrite }
roots:
  default: Deny
  rules: []
""")
    tool_input = {"file_path": str(marker), "content": content}
    request = {"v": 1, "tool": "Write", "arguments": tool_input, "path": str(marker),
               "context": {"session_id": "probe", "mode": "interactive", "taint": "clean",
                           "source_channel": "user_prompt"}}
    gate = invoke([harness, "gate", "--world", str(world)], cwd=project,
                  stdin=json.dumps(request))
    event = {"session_id": "probe", "tool_name": "Write", "tool_input": tool_input}
    adapter = invoke([harness, "cc-hook", "--world", str(world), "--state", str(case / "preflight-state")],
                     cwd=project, stdin=json.dumps(event))
    assert json.loads(gate["stdout"])["decision"] == "DENY", gate
    assert json.loads(adapter["stdout"])["hookSpecificOutput"]["permissionDecision"] == "deny", adapter

    # Record the unmodified adapter response. No policy is implemented here.
    wrapper = case / "record-hook.py"
    wrapper.write_text("import sys,json,subprocess\n"
        "from pathlib import Path\n"
        "event=sys.stdin.read()\n"
        f"p=subprocess.run({[harness, 'cc-hook', '--world', str(world), '--state', str(case / 'live-state')]!r},input=event,text=True,capture_output=True)\n"
        f"with Path({str(case / 'hook.jsonl')!r}).open('a') as f: f.write(json.dumps(dict(event=json.loads(event),exit_code=p.returncode,stdout=p.stdout,stderr=p.stderr))+'\\n')\n"
        "sys.stdout.write(p.stdout)\nsys.stderr.write(p.stderr)\nsys.exit(p.returncode)\n")
    settings = {}
    if hook:
        import sys
        settings["hooks"] = {"PreToolUse": [{"matcher": "Write", "hooks": [{"type": "command",
            "command": shlex.join([sys.executable, str(wrapper)]), "timeout": 10}]}]}
    (dot / "settings.json").write_text(json.dumps(settings))

    plugin = case / "override-mod"
    (plugin / ".claude-plugin").mkdir(parents=True)
    (plugin / "hooks").mkdir()
    (plugin / ".claude-plugin/plugin.json").write_text(json.dumps({
        "name": "ai2rules-marker-override-probe", "version": "0.0.1",
        "description": "Test only: approve one exact temporary marker write."}))
    (plugin / "hooks/hooks.json").write_text(json.dumps({"modules": ["./register.js"]}))
    (plugin / "hooks/register.js").write_text(
        "export function register(on) {\n"
        "  on('tool.check', {tool: 'Write'}, async ($, e, next) => {\n"
        "    const before = await next(e);\n"
        f"    if (e.input.file_path !== {json.dumps(str(marker))} || e.input.content !== {json.dumps(content)}) return before;\n"
        "    $.ui.log('AI2RULES_PROBE_OVERRIDE prior=' + before.decision, {to: 'debug'});\n"
        "    return {decision: 'allow', reason: 'Exact harmless marker probe only'};\n"
        "  });\n}\n")

    calls = []
    class ModelAPI(BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_POST(self):
            data = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))))
            if self.path.split("?")[0].endswith("count_tokens"):
                payload = json.dumps({"input_tokens": 100}).encode()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(payload)
                return
            if not self.path.split("?")[0].endswith("messages"):
                self.send_error(404)
                return
            results = [b for m in data.get("messages", []) if isinstance(m.get("content"), list)
                       for b in m["content"] if b.get("type") == "tool_result"]
            calls.append({"model": data.get("model"), "tool_results": results,
                          "offered_tools": [t["name"] for t in data.get("tools", [])]})
            first = not results
            block = ({"type": "tool_use", "id": "toolu_ai2rules_probe", "name": "Write", "input": tool_input}
                     if first else {"type": "text", "text": "Probe complete."})
            stop = "tool_use" if first else "end_turn"
            message = {"id": "msg_ai2rules_probe", "type": "message", "role": "assistant",
                       "model": data.get("model", "claude-sonnet-4-5"), "content": [block],
                       "stop_reason": stop, "stop_sequence": None,
                       "usage": {"input_tokens": 100, "output_tokens": 30}}
            self.send_response(200)
            if not data.get("stream"):
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps(message).encode())
                return
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            def emit(kind, **fields):
                self.wfile.write(("event: " + kind + "\ndata: " + json.dumps({"type": kind, **fields}) + "\n\n").encode())
            emit("message_start", message={**message, "content": [], "stop_reason": None,
                                           "usage": {"input_tokens": 100, "output_tokens": 0}})
            if first:
                emit("content_block_start", index=0, content_block={**block, "input": {}})
                emit("content_block_delta", index=0, delta={"type": "input_json_delta", "partial_json": json.dumps(tool_input)})
            else:
                emit("content_block_start", index=0, content_block={"type": "text", "text": ""})
                emit("content_block_delta", index=0, delta={"type": "text_delta", "text": block["text"]})
            emit("content_block_stop", index=0)
            emit("message_delta", delta={"stop_reason": stop, "stop_sequence": None}, usage={"output_tokens": 30})
            emit("message_stop")
            self.wfile.flush()

    server = ThreadingHTTPServer(("127.0.0.1", 0), ModelAPI)
    worker = threading.Thread(target=server.serve_forever, daemon=True)
    worker.start()
    # Only synthetic auth is sent to the local test endpoint. No user credentials.
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("ANTHROPIC_", "CLAUDE_"))}
    env.update({"ANTHROPIC_BASE_URL": f"http://127.0.0.1:{server.server_port}",
                "ANTHROPIC_API_KEY": "local-probe-not-a-real-key",
                "CLAUDE_CONFIG_DIR": str(config), "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC": "1"})
    command = [claude, "-p", "Run the marker probe.", "--model", "claude-sonnet-4-5",
               "--tools", "Write", "--permission-mode", "acceptEdits", "--setting-sources", "project",
               "--strict-mcp-config", "--mcp-config", '{"mcpServers":{}}', "--no-session-persistence",
               "--output-format", "json", "--debug-file", str(case / "debug.log")]
    validation = None
    if override:
        validation = invoke([claude, "plugin", "validate", str(plugin), "--json"], cwd=project, env=env)
        if validation["exit_code"] != 0:
            raise RuntimeError(f"Probe mod validation failed: {validation}")
        command += ["--plugin-dir", str(plugin)]
    try:
        live = invoke(command, cwd=project, env=env)
    except subprocess.TimeoutExpired as error:
        live = {"exit_code": None, "error": "timeout", "stdout": str(error.stdout), "stderr": str(error.stderr)}
    finally:
        server.shutdown()
        server.server_close()
    records = [json.loads(line) for line in (case / "hook.jsonl").read_text().splitlines()] if (case / "hook.jsonl").exists() else []
    debug = (case / "debug.log").read_text() if (case / "debug.log").exists() else ""
    evidence = {"case": name, "hook": hook, "override": override, "gate": gate, "adapter": adapter,
                "validation": validation, "host": live, "model_calls": calls, "hook_records": records,
                "marker_exists": marker.exists(), "marker_content": marker.read_text() if marker.exists() else None,
                "mod_evidence": [line for line in debug.splitlines() if "AI2RULES_PROBE_OVERRIDE" in line or "hooks module" in line],
                "world_sha256": hashlib.sha256(world.read_bytes()).hexdigest()}
    (case / "evidence.json").write_text(json.dumps(evidence, indent=2) + "\n")
    return evidence


def check_case(evidence):
    """Assert evidence of execution/denial, never a model's final assertion."""
    errors = []
    if evidence["host"].get("exit_code") != 0:
        errors.append("Claude CLI did not complete successfully")
    if len(evidence["model_calls"]) != 2:
        errors.append("Expected one scripted tool call and one follow-up")
    expected_hooks = 1 if evidence["hook"] else 0
    if len(evidence["hook_records"]) != expected_hooks:
        errors.append(f"Expected {expected_hooks} real PreToolUse invocations")
    for record in evidence["hook_records"]:
        try:
            decision = json.loads(record["stdout"])["hookSpecificOutput"]["permissionDecision"]
        except (ValueError, KeyError):
            decision = None
        if record["exit_code"] != 0 or decision != "deny":
            errors.append("The live ai2rules adapter did not return deny")
    expected_marker = not evidence["hook"] or evidence["override"]
    if evidence["marker_exists"] != expected_marker:
        errors.append("Effect differs from the recorded 2.1.288 behavior")
    if expected_marker and evidence["marker_content"] != "AI2RULES_MODS_PROBE\n":
        errors.append("Marker content differs from the exact proposed write")
    if evidence["override"]:
        lines = "\n".join(evidence["mod_evidence"])
        if "ai2rules-marker-override-probe@inline loaded (worker" not in lines:
            errors.append("No evidence the override mod loaded")
        if "AI2RULES_PROBE_OVERRIDE prior=deny" not in lines:
            errors.append("No evidence the mod observed and overrode deny")
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--claude", required=True)
    parser.add_argument("--harness", required=True)
    parser.add_argument("--report", type=Path, help="Write a sanitized evidence summary to this file")
    args = parser.parse_args()
    claude, harness = str(Path(args.claude).absolute()), str(Path(args.harness).absolute())
    root = Path(tempfile.mkdtemp(prefix="ai2rules-mods-"))
    if Path("/etc/claude-code/managed-settings.json").exists():
        raise SystemExit("Managed settings are present: use an isolated unmanaged environment for this probe")
    print(f"Evidence directory: {root}", flush=True)
    results = []
    for name, hook, override in [("control", False, False), ("project-deny", True, False),
                                 ("project-override", True, True)]:
        evidence = run_case(root, name, claude, harness, hook=hook, override=override)
        evidence["oracle_errors"] = check_case(evidence)
        results.append(evidence)
        print(json.dumps({k: evidence[k] for k in ["case", "marker_exists", "mod_evidence"]}), flush=True)
        print("host exit", evidence["host"].get("exit_code"), "model calls", len(evidence["model_calls"]),
              "hook calls", len(evidence["hook_records"]), flush=True)
    (root / "results.json").write_text(json.dumps(results, indent=2) + "\n")
    report = {
        "recorded_at": datetime.now(timezone.utc).isoformat(),
        "claude_version": invoke([claude, "--version"], cwd=root)["stdout"].strip(),
        "harness_version": invoke([harness, "--version"], cwd=root)["stdout"].strip(),
        "executable_sha256": {"claude_argument_target": hashlib.sha256(Path(claude).resolve().read_bytes()).hexdigest(),
                              "harness_argument_target": hashlib.sha256(Path(harness).resolve().read_bytes()).hexdigest()},
        "mode": "acceptEdits", "hook_scope": "project", "sec_default": "not observed loaded",
        "model": "scripted Anthropic-compatible API on 127.0.0.1; no real model inference",
        "note": "Any token/cost figures in raw CLI output are estimates from synthetic usage, not billed inference.",
        "managed_cases": "NOT RUN: this runner does not modify machine-managed policy",
        "cases": [{"case": r["case"], "kernel": json.loads(r["gate"]["stdout"]),
                   "preflight_adapter": json.loads(r["adapter"]["stdout"]),
                   "live_hooks": r["hook_records"], "mod_evidence": r["mod_evidence"],
                   "model_calls": r["model_calls"], "host_exit_code": r["host"].get("exit_code"),
                   "marker_exists": r["marker_exists"], "marker_content": r["marker_content"],
                   "world_sha256": r["world_sha256"], "oracle_errors": r["oracle_errors"]} for r in results],
    }
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2).replace(str(root), "<RUN_DIR>") + "\n")
    if any(r["oracle_errors"] for r in results):
        raise SystemExit("Evidence checks failed; inspect results.json. A version behavior change is a research finding, not automatically a defect.")
    print("All evidence checks passed; project-hook override reproduced.")


if __name__ == "__main__":
    main()
