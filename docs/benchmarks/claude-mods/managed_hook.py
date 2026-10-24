#!/usr/bin/env python3
"""Recording shim installed ONLY by the disposable image's managed policy."""
import json
from pathlib import Path
import subprocess
import sys

HARNESS = "/opt/probe-tools/node_modules/ai2rules-harness-linux-x64/harness"


def main():
    event = sys.stdin.read()
    project = Path.cwd()
    case = project.parent
    result = subprocess.run(
        [HARNESS, "cc-hook", "--world", str(project / ".claude/cc-world.yaml"),
         "--state", str(case / "live-state")],
        input=event, text=True, capture_output=True, timeout=8)
    with (case / "hook.jsonl").open("a") as output:
        output.write(json.dumps({"event": json.loads(event),
            "policy_source": "/etc/claude-code/managed-settings.json",
            "wrapper": str(Path(__file__).resolve()),
            "exit_code": result.returncode, "stdout": result.stdout,
            "stderr": result.stderr}) + "\n")
    sys.stdout.write(result.stdout)
    sys.stderr.write(result.stderr)
    sys.exit(result.returncode)


if __name__ == "__main__":
    main()
