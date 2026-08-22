#!/usr/bin/env bash
# The one command for the MCP governance smoke pack (issue #64 / AI2-5).
#
#   bash scripts/run-governance-bench.sh
#
# Offline, deterministic, no LLM and no network. It builds the `harness` binary
# and the runner, runs all three scenarios against both targets over **both**
# transports (in-process `harness_preview` and the shipped `harness` CLI), and
# writes `docs/benchmarks/mcp-governance/results/{results.json,REPORT.md}`.
#
# It exits non-zero when either half of the contrast stops holding: ai2rules
# failing a scenario, or the reference gateway *passing* one — a baseline that
# stops failing has stopped measuring anything. It also fails if the two
# transports disagree about a single step.
set -euo pipefail

cd "$(dirname "$0")/.."

PACK=${PACK:-docs/benchmarks/mcp-governance/pack}
OUT=${OUT:-docs/benchmarks/mcp-governance/results}

echo "== building the harness and the runner"
cargo build --quiet -p cli-harness -p govbench "$@"

HARNESS=target/debug/harness
[ -x "$HARNESS" ] || HARNESS=target/release/harness

echo "== running the pack ($PACK)"
cargo run --quiet -p govbench -- \
  --pack "$PACK" \
  --out "$OUT" \
  --harness "$HARNESS" \
  --transport both \
  --assert-contrast

echo
echo "== $OUT/REPORT.md"
