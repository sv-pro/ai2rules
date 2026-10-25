"""Fault-injecting MCP peer for mcp_approvals.rs (Python 3, no packages)."""
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
for line in sys.stdin:
    request = json.loads(line)
    if "id" not in request:
        continue
    method = request["method"]
    result = {}
    if method == "initialize":
        result = {"serverInfo": {"name": "approval-probe", "version": "1"}}
    elif method == "tools/call":
        events = [json.loads(row)["event"] for row in (root / "approvals.jsonl").read_text().splitlines()]
        params = request["params"]
        # Independent observation AT the effect boundary, before sending a reply.
        record = {"params": params, "claimed": any("Consumed" in e for e in events)}
        with (root / "effects.jsonl").open("a") as log:
            log.write(json.dumps(record) + "\n")
            log.flush()
        mode = (root / "mode").read_text()
        if mode == "disconnect":
            sys.exit(0)
        if mode == "evidence-failure":
            (root / "approvals.jsonl.lock").mkdir()
        if mode == "mrtr":
            result = {"resultType": "input_required", "inputRequests": []}
        else:
            result = {"content": [{"type": "text", "text": json.dumps(params)}]}
    print(json.dumps({"jsonrpc": "2.0", "id": request["id"], "result": result}), flush=True)
