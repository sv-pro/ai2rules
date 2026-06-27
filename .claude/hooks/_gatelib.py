"""Shared gate helpers for the Claude Code host hooks (DECISIONS D29).

`world-gate.py` (PreToolUse) and `taint-notify.py` (SubagentStop) both need the
*same* answer to "is this session tainted?", so the logic lives here once.

Taint is a LEDGER of causes (the sidecar under .claude/state/), recomputed on every
call:  tainted  ==  some recorded cause is NOT covered by a valid `trust_pins`
entry.  A trust_pin is an operator attestation that a specific source is trusted,
pinned to its content identity; a pinned, content-verified read was never an
untrusted-taint source, so it does not escalate.  Any drift (the bytes / commit no
longer match) revokes the pin and the cause taints again — the descriptor-drift
primitive (safe-mcp-proxy) applied to read sources.  Pins NEVER weaken the egress
floor (invariant 7); they only re-classify a vouched source upstream of taint.

Everything fails CLOSED for taint (can't verify a pin → not covered → still tainted)
and the callers fail OPEN on import error (a broken helper can't brick a session).
"""
import os
import re
import json
import hashlib
import fnmatch
import datetime
import subprocess

_URL_RE = re.compile(r"^[a-zA-Z][a-zA-Z0-9+.\-]*://")


def load_cfg(proj_dir):
    """Load the WorldManifest the live hook reads (cc-world.json)."""
    cfg_path = os.environ.get("CC_WORLD_CONFIG") or os.path.join(
        proj_dir, ".claude", "cc-world.json")
    try:
        with open(cfg_path) as f:
            return json.load(f)
    except Exception:
        return None


def load_pins(cfg):
    pins = (cfg or {}).get("trust_pins") or []
    return [p for p in pins if isinstance(p, dict)] if isinstance(pins, list) else []


def _cause_target(line):
    """Extract the source a ledger line refers to (a path or a URL), or None.

    Lines are 'tainted by <tool> (agent: <a>): <target>'.  Lines without a
    '<space>: <target>' tail (e.g. 'propagated from subagent session X') are
    opaque causes — no pin can cover them, so they keep the session tainted.
    """
    if ": " not in line:
        return None
    target = line.rsplit(": ", 1)[1].strip()
    return target or None


def ledger_causes(taint_file):
    try:
        with open(taint_file) as f:
            return [ln.rstrip("\n") for ln in f if ln.strip()]
    except Exception:
        return []


def _sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def _pin_matches_path(pin_path, target):
    if any(c in pin_path for c in "*?["):
        return fnmatch.fnmatch(target, pin_path) or fnmatch.fnmatch(target, "*/" + pin_path)
    return pin_path in target


def _git_clean_at(repo, commit):
    try:
        head = subprocess.run(["git", "-C", repo, "rev-parse", "HEAD"],
                              capture_output=True, text=True, timeout=5)
        if head.returncode != 0 or head.stdout.strip() != commit:
            return False
        st = subprocess.run(["git", "-C", repo, "status", "--porcelain"],
                            capture_output=True, text=True, timeout=5)
        return st.returncode == 0 and st.stdout.strip() == ""
    except Exception:
        return False


def _pin_valid(pin, target, proj_dir):
    exp = pin.get("expires")
    if exp:
        try:
            if datetime.date.today().isoformat() > str(exp):
                return False  # attestation timed out
        except Exception:
            pass
    ident = pin.get("identity") or {}
    kind = ident.get("kind")
    if kind == "sha256":
        try:
            return _sha256(target).lower() == str(ident.get("hash", "")).lower()
        except Exception:
            return False
    if kind == "git_commit":
        repo = ident.get("repo", "")
        repo_abs = repo if os.path.isabs(repo) else os.path.join(proj_dir, repo)
        return bool(ident.get("commit")) and _git_clean_at(repo_abs, str(ident["commit"]))
    return False


def cause_covered(target, pins, proj_dir):
    """True iff `target` is a local file matched by a still-valid trust_pin."""
    if not target or _URL_RE.match(target):
        return False  # URLs / opaque causes are never pinnable
    abs_target = target if os.path.isabs(target) else os.path.join(proj_dir, target)
    if not os.path.exists(abs_target):
        return False  # fail closed: can't verify identity → not covered
    for pin in pins:
        pp = pin.get("path", "")
        if pp and _pin_matches_path(pp, abs_target) and _pin_valid(pin, abs_target, proj_dir):
            return True
    return False


def is_tainted(taint_file, pins, proj_dir):
    """Recompute taint from the cause ledger minus valid trust_pins."""
    for line in ledger_causes(taint_file):
        if not cause_covered(_cause_target(line), pins, proj_dir):
            return True
    return False
