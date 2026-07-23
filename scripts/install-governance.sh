#!/usr/bin/env bash
# install-governance.sh (v0) — make any project on any machine governed by the
# ai2rules harness (a WorldManifest enforced via a Claude Code PreToolUse hook).
#
# One command does both halves:
#   * per machine: ensure the `harness` binary is available on PATH
#   * per project: drop the .claude/ shim + starter manifest, merge settings.json
#
# Usage:
#   install-governance.sh [--grant] [--source DIR] [--bin-dir DIR] [--force] [TARGET]
#     TARGET         project dir to govern (default: current dir)
#     --grant        Tier-1 grant mode (manifest grants -> fewer prompts).
#                    Default is additive (deny/ask overlay only; safest, no lockout).
#     --source DIR   an ai2rules checkout (templates + binary/build). Auto-detected
#                    when this script lives in <checkout>/scripts/.
#     --bin-dir DIR  where to install harness (default: ~/.local/bin)
#     --force        reinstall the binary even if one is already on PATH
#     -h, --help     this help
#
# Rollback in the governed project: `touch .claude/gate-off` (off, next call, no
# restart) or `~/.claude/gate-off` (panic, everywhere); `rm` to re-enable.
set -euo pipefail

GRANT=0; FORCE=0; TARGET=""; BIN_DIR="${HOME}/.local/bin"; SOURCE=""
SELF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

while [ $# -gt 0 ]; do
  case "$1" in
    --grant)   GRANT=1 ;;
    --force)   FORCE=1 ;;
    --source)  SOURCE="${2:?--source needs a dir}"; shift ;;
    --bin-dir) BIN_DIR="${2:?--bin-dir needs a dir}"; shift ;;
    -h|--help) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    -*)        echo "unknown option: $1" >&2; exit 2 ;;
    *)         TARGET="$1" ;;
  esac
  shift
done
TARGET="${TARGET:-$PWD}"
say(){ printf '\033[36m[install]\033[0m %s\n' "$*"; }

# --- locate an ai2rules checkout (templates + build fallback) --------------------
if [ -z "$SOURCE" ] && [ -f "$SELF/../Cargo.toml" ] && [ -f "$SELF/../.claude/cc-world.yaml" ]; then
  SOURCE="$(cd "$SELF/.." && pwd)"
fi
if [ -z "$SOURCE" ] || [ ! -f "$SOURCE/.claude/cc-world.yaml" ]; then
  echo "error: no ai2rules checkout found for templates. Pass --source DIR." >&2
  exit 1
fi

# --- per machine: ensure `harness` on PATH ---------------------------------------
ensure_harness(){
  if [ "$FORCE" -eq 0 ] && command -v harness >/dev/null 2>&1; then
    say "harness already on PATH: $(command -v harness)"; return
  fi
  mkdir -p "$BIN_DIR"
  if [ -x "$SOURCE/target/release/harness" ]; then
    say "installing harness from the checkout's release build -> $BIN_DIR"
    install -m 0755 "$SOURCE/target/release/harness" "$BIN_DIR/harness"
  elif command -v cargo >/dev/null 2>&1; then
    say "building harness (cargo release) — one-time ..."
    cargo build --release --manifest-path "$SOURCE/Cargo.toml" -p cli-harness
    install -m 0755 "$SOURCE/target/release/harness" "$BIN_DIR/harness"
  else
    echo "error: no prebuilt harness and no cargo to build one." >&2
    echo "  On a Rust machine: cargo install --path <checkout>/crates/cli-harness" >&2
    echo "  Or download a prebuilt release binary and put it on PATH." >&2
    exit 1
  fi
  say "installed: $BIN_DIR/harness"
  case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) say "NOTE: $BIN_DIR is not on PATH — add it, or rely on the shim's own lookup." ;;
  esac
}

# --- per project: shim (self-contained, kill-switch baked in) ---------------------
write_shim(){
  mkdir -p "$TARGET/.claude/hooks"
  local dst="$TARGET/.claude/hooks/world-gate.sh" grant_flag=""
  [ "$GRANT" -eq 1 ] && grant_flag=" --grant"
  cat > "$dst" <<SHIM
#!/usr/bin/env bash
# ai2rules governance shim (installed by install-governance.sh). Execs the Rust
# kernel's PreToolUse adapter; no governance logic lives here. Fail-open: no binary
# -> exit 0 (the tool call falls through to Claude Code's normal flow).
set -u
PD="\${CLAUDE_PROJECT_DIR:-\$(pwd)}"
# Instant kill-switch, no restart: touch .claude/gate-off (this project) or
# ~/.claude/gate-off (panic, everywhere) to disable governance on the NEXT call; rm
# to re-enable. The shim runs per call, so the toggle is immediate.
if [ -f "\$PD/.claude/gate-off" ] || [ -f "\$HOME/.claude/gate-off" ]; then exit 0; fi
BIN="\${HARNESS_BIN:-}"
if [ -z "\$BIN" ] || [ ! -x "\$BIN" ]; then
  BIN=""
  for c in "\$PD/target/release/harness" "\$PD/target/debug/harness"; do
    [ -x "\$c" ] && { BIN="\$c"; break; }
  done
fi
[ -z "\$BIN" ] && BIN="\$(command -v harness 2>/dev/null || true)"
[ -z "\$BIN" ] && exit 0  # fail-open: no kernel, normal permissions
exec "\$BIN" cc-hook${grant_flag} --world "\$PD/.claude/cc-world.yaml" --state "\$PD/.claude/state"
SHIM
  chmod +x "$dst"
  say "shim -> .claude/hooks/world-gate.sh ($([ "$GRANT" -eq 1 ] && echo 'grant / Tier-1' || echo 'additive'))"
}

# --- per project: starter manifest (never clobbers a tuned one) -------------------
write_manifest(){
  local dst="$TARGET/.claude/cc-world.yaml"
  if [ -f "$dst" ]; then say "manifest exists — keeping it: .claude/cc-world.yaml"; return; fi
  # Prefer the dedicated, portable starter (governs native tools + confines file
  # actions to declared roots). Fall back to the dogfood manifest if it's absent.
  local src="$SOURCE/scripts/starter-world.yaml"
  [ -f "$src" ] || src="$SOURCE/.claude/cc-world.yaml"
  cp "$src" "$dst"
  say "starter manifest -> .claude/cc-world.yaml (roots-confined; tune it for this project)"
}

# --- per project: merge the PreToolUse hook (idempotent) --------------------------
merge_settings(){
  local s="$TARGET/.claude/settings.json"
  local command='bash "$CLAUDE_PROJECT_DIR/.claude/hooks/world-gate.sh"'
  local entry='{"matcher":"*","hooks":[{"type":"command","command":"bash \"$CLAUDE_PROJECT_DIR/.claude/hooks/world-gate.sh\"","timeout":10}]}'
  if ! command -v jq >/dev/null 2>&1; then
    say "jq not found — add this PreToolUse hook to $s by hand:"; echo "  $entry"; return
  fi
  local cur='{}'; [ -f "$s" ] && cur="$(cat "$s")"
  echo "$cur" | jq --arg command "$command" --argjson e "$entry" '
    .hooks.PreToolUse = (
      (.hooks.PreToolUse // []) as $p
      | if ([
          $p[]?
          | select(.matcher == "*")
          | .hooks[]?
          | select(.type == "command" and .command == $command and .timeout == 10)
        ] | length > 0)
        then $p else $p + [$e] end
    )
  ' > "$s.tmp" && mv "$s.tmp" "$s"
  say "PreToolUse hook merged -> .claude/settings.json"
}

# --- per project: gitignore runtime state -----------------------------------------
add_gitignore(){
  local gi="$TARGET/.gitignore"
  for line in ".claude/state/" ".claude/gate-off"; do
    { [ ! -f "$gi" ] || ! grep -qxF "$line" "$gi"; } && echo "$line" >> "$gi"
  done
  say "gitignore -> .claude/state/, .claude/gate-off"
}

say "target project : $TARGET"
say "source checkout: $SOURCE"
ensure_harness
write_shim
write_manifest
merge_settings
add_gitignore
cat <<DONE

[install] Done. Verify it's actually governing (in a Claude Code session in the target):
  1) WebFetch any page             -> taints the session
  2) then curl / WebFetch again    -> should be DENIED (no_tainted_external)
     A plain, ungoverned Claude would just prompt. That deny is the proof.
Kill-switch: touch "$TARGET/.claude/gate-off"   (off, next call; rm to re-enable)
DONE
