#!/usr/bin/env bash
# check-demos.sh — run every demo and assert it still says what it claims.
#
# Why this exists: twice, a hardening change landed and a demo degraded in
# *silence*, exit code 0 throughout.
#
#   * `demo-one-kernel-many-hosts.sh` stopped sending `source_channel` when the
#     gate began requiring it, so three teaching beats printed
#     `DENY missing_source_channel` instead of ASK / background_denies_ask /
#     taint_invariant — and the script's own parity check still passed, because
#     both shapes failed closed identically.
#   * `execution_demo`'s third refusal became unrepresentable when arguments were
#     sealed (D45). `decide()` returned NotRepresentable, the `if let` matched
#     nothing, and a third of the demo vanished with no output at all.
#
# The unit tests were green through both. They cover the kernel; nothing covered
# what the demos *print*, which is what a reader actually experiences. This does.
#
# Usage: bash scripts/check-demos.sh    (offline, no creds, ~1 min)
set -uo pipefail
cd "$(dirname "$0")/.."

# Output is compared after stripping ANSI colour and squeezing runs of spaces, so
# markers below are written with single spaces and survive column realignment.
normalize() { sed 's/\x1b\[[0-9;]*m//g' | tr -s ' '; }

# Patterns that mean "a demo fell through a branch and said so", or that a
# regression already caught once has come back. Never allowed in any demo output.
FORBIDDEN=('unexpected:' 'missing_source_channel' 'DEBUG-')

fails=0

# check <label> <command> <required marker>...
check() {
  local label="$1" cmd="$2"
  shift 2
  local out bad=0 m
  if ! out="$(eval "$cmd" 2>&1)"; then
    printf 'FAIL  %s — exited nonzero\n' "$label"
    printf '%s\n' "$out" | tail -15 | sed 's/^/        /'
    fails=$((fails + 1))
    return
  fi
  out="$(printf '%s\n' "$out" | normalize)"
  for m in "$@"; do
    grep -qF -- "$m" <<<"$out" || { printf 'FAIL  %s — missing: %s\n' "$label" "$m"; bad=1; }
  done
  for m in "${FORBIDDEN[@]}"; do
    grep -qF -- "$m" <<<"$out" && { printf 'FAIL  %s — forbidden: %s\n' "$label" "$m"; bad=1; }
  done
  if [ "$bad" -eq 0 ]; then
    printf 'ok    %s\n' "$label"
  else
    fails=$((fails + 1))
  fi
}

echo "== building (once) =="
cargo build --workspace --examples --quiet || { echo "build failed"; exit 1; }

echo
echo "== crate examples =="

check "kernel_demo — five verdicts" \
  "cargo run -q -p world-kernel --example kernel_demo" \
  '(rule: default_allow)' 'UNKNOWN_TO_ONTOLOGY' '(rule: capability)' \
  '(rule: taint_invariant)' '(rule: approval_required)' '(rule: max_commands_per_task)'

check "execution_demo — three refusals" \
  "cargo run -q -p world-kernel --example execution_demo" \
  'REFUSED : write outside writable roots' \
  'REFUSED : descriptor drift' \
  'REFUSED : killed after 200ms'

check "trace_demo — redaction, replay, drift" \
  "cargo run -q -p trace-store --example trace_demo" \
  'contains the secret token? false' \
  'reproduced 5/5 decisions' \
  'Deny -> Absent'

check "agent_loop — propose/decide/perceive" \
  "cargo run -q -p agent-core --example agent_loop" \
  'Deny (taint_invariant)' 'UNKNOWN_TO_ONTOLOGY' 'ASK (pending approval)'

check "tools_demo — scoped caps, MCP, web" \
  "cargo run -q -p agent-core --example tools_demo" \
  'lowered argv: ["pytest"]' 'Deny (taint_invariant)'

check "approvals_demo — ASK resumes, background fails closed" \
  "cargo run -q -p agent-core --example approvals_demo" \
  'ASK → APPROVED → ALLOW' 'Deny (background_denies_ask)'

check "poisoned_knowledge_demo — cross-layer" \
  "cargo run -q -p agent-core --example poisoned_knowledge_demo" \
  'call_known_mcp_tool ALLOW' 'Deny (taint_invariant)'

echo
echo "== shell demos =="

check "demo-one-kernel-many-hosts.sh" \
  "bash scripts/demo-one-kernel-many-hosts.sh" \
  "ABSENT: action is not in this world's ontology" \
  'ASK rule=approval_required' \
  'DENY rule=background_denies_ask' \
  'DENY rule=taint_invariant' \
  'one kernel, many hosts'

check "replace-permissions/demo.sh" \
  "bash docs/demos/replace-permissions/demo.sh" \
  '(manifest ALLOW -> grant) -> allow' \
  '(taint floor) -> deny' \
  '(destructive -> ask) -> ask' \
  'ABSENT) -> deny'

echo
if [ "$fails" -gt 0 ]; then
  echo "$fails demo(s) no longer say what they claim."
  exit 1
fi
echo "All demos still say what they claim."
