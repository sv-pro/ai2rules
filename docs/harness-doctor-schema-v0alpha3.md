# harness doctor report schema v0alpha3

**Identifier:** `ai2rules.dev/harness-doctor/v0alpha3`  
**Profile:** `claude-code-cli/static-v1`  
**Status:** Review specification (#84) — not shipped

This is a typed contract for the `harness doctor [--json]` command, which inspects
the static installation state of the ai2rules harness in a Claude Code CLI project.
The command performs read-only inspection; it never writes manifests, installs hooks,
or runs discovered binaries.

## Implementation slice

Issue #84 defines the report contract, reducer, and CLI skeleton. Full static
inspection (#85), coverage analysis (#86), fixture matrix (#87), and documentation
(#88) are separate dependent issues. This schema freezes the machine contract so
tests can be written against it.

## Dispatch timing

Doctor is dispatched before normal agent/session/trace initialization to avoid
polluting the inspection with runtime artifacts.

## Exit code reduction

Exit codes follow priority: **error > broken > disabled > partial > unknown > ok**

| Exit | Status | Meaning |
|------|--------|---------|
| 0 | ok | Mandatory installation checks in this bounded profile pass. Runtime unknown and documented coverage limits do NOT change exit 0. |
| 1 | partial | Missing registration/Claude candidate/interpreter, narrow matcher, recognizable legacy limitations. |
| 1 | disabled | Observed effective kill switch on. |
| 1 | unknown | Unsupported/ambiguous configuration or a required static observation unavailable. |
| 2 | broken | Malformed relevant config, missing referenced shim/binary/manifest, invalid explicit HARNESS_BIN, wrong active project path, compile failure. |
| 3 | error | Unexpected internal doctor failure. Expected read/access failures are explicit unknown findings, not "internal crash". |

## Output modes

- **Human-readable (default):** Heading, summary, limitations, findings. Heading format:
  `INSTALLATION <STATUS> — inspected Claude Code CLI project profile`. Never "protected".
- **JSON (`--json`):** One JSON object + newline, no ANSI. Empty stderr for normal reports
  including exits 1/2. CLI parse errors use Clap exit 2/usage; stdout failure/process
  termination cannot guarantee JSON.

## Contract guarantees

- All defined fields are present in the output.
- Unknown scalar/boolean fields are `null`, not `false`.
- State enums use an `"unknown"` variant rather than null.
- Collections of observations can be `[]` but MUST have a separate completeness/status
  field; `[]` never means "there are no live tools".
- Stable code/order/type assertions, not prose snapshots.
- Findings are sorted deterministically.

## Top-level schema

```json
{
  "schema_version": "ai2rules.dev/harness-doctor/v0alpha3",
  "profile": "claude-code-cli/static-v1",
  "project": {
    "path": "/absolute/path/to/project" | null
  },
  "summary": {
    "installation": "ok" | "partial" | "disabled" | "unknown" | "broken" | "error",
    "runtime_enforcement": "unknown",
    "coverage": "partial" | "none" | "unknown",
    "exit_code": 0 | 1 | 2 | 3
  },
  "binaries": {
    "doctor": { /* Binary */ },
    "hook_harness": { /* Binary */ },
    "claude_candidate": { /* Binary */ },
    "interpreter": { /* Binary */ }
  },
  "settings": {
    "project_path": "/path/to/settings.json" | null,
    "status": "valid" | "missing" | "invalid" | "unknown",
    "observed_sources": [
      {
        "path": "/absolute/path",
        "scope": "user" | "workspace" | "folder",
        "status": "valid" | "missing" | "invalid"
      }
    ],
    "effective_configuration": "unknown",
    "uninspected_sources": ["reason1", "reason2"]
  },
  "hook": {
    "status": "recognized" | "missing" | "ambiguous" | "unsupported" | "unknown",
    "entries": [
      {
        "source_path": "/absolute/path/to/settings.json",
        "json_pointer": "/tools/hooks/0",
        "matcher": "*" | null,
        "kind": "PreToolUse",
        "invocation": "normalized invocation or 'unsupported'"
      }
    ],
    "shim_path": "/path/to/shim.sh" | null,
    "shim_resolved_path": "/canonical/path" | null,
    "shim_profile": "recognized_profile" | null,
    "grant": true | false | null,
    "enforce_absent": true | false | null
  },
  "manifest": {
    "path": "/path/to/cc-world.yaml" | null,
    "compile_status": "ok" | "missing" | "invalid" | "unknown",
    "hash": "sha256-hex" | null,
    "hash_kind": "resolved_world_manifest_sha256",
    "resolution_context": {
      "project_dir": "/absolute/path" | null,
      "home": "/home/user" | null,
      "base_source": "default" | "explicit" | null
    }
  },
  "switches": {
    "observed": [
      {
        "kind": "project" | "user",
        "path": "/path/to/switch",
        "state": "on" | "off" | "unknown"
      }
    ],
    "effective_in_inspected_context": "on" | "off" | "unknown"
  },
  "coverage": {
    "configured_matchers": ["*", "shell_*"],
    "projected_actions": ["Read", "Write", "Shell"],
    "projected_actions_status": "known" | "unknown",
    "live_native_inventory": "unknown",
    "live_mcp_inventory": "unknown",
    "absent_behavior": "pass_through" | "deny" | "unknown",
    "downstream_execution": "not_individually_mediated" | "unknown",
    "limitations": [
      "Static inspection only; no runtime session",
      "Claude candidate version unknown without invocation"
    ]
  },
  "failure_behavior": {
    "shim_missing_binary": "open" | "unknown",
    "adapter_process_error": "open" | "unknown",
    "state_persistence_error": "conditional_deny" | "unknown",
    "evidence": [
      "Recognized shim pattern: fail-open on missing binary",
      "Selected binary is not this executable; adapter semantics unknown"
    ]
  },
  "findings": [
    {
      "code": "MANIFEST_MISSING",
      "severity": "info" | "warning" | "error",
      "component": "manifest" | "hook" | "settings" | "binary" | "doctor",
      "message": "Human-readable finding description",
      "evidence_paths": ["/path/to/evidence"],
      "affects_installation": true | false,
      "next_action": {
        "kind": "command" | "manual",
        "argv": ["harness", "init"] | null,
        "message": "Human-readable next step"
      } | null
    }
  ]
}
```

## Binary schema

```json
{
  "path": "/usr/local/bin/harness" | null,
  "resolved_path": "/canonical/path/harness" | null,
  "executable": true | false | null,
  "version": "0.6.0" | null,
  "version_source": "build_metadata" | "same_executable" | "not_probed",
  "status": "found" | "missing" | "unknown"
}
```

**version_source semantics:**
- `build_metadata`: Doctor's own version from `CARGO_PKG_VERSION`
- `same_executable`: Selected harness binary is the current executable
- `not_probed`: Do NOT invoke discovered binaries for `--version`

Finding explains unknown versions; existence is not authenticity.

## Distinctions

The report distinguishes:
- **Executing doctor binary** (always found, version from build metadata)
- **Shim-selected harness binary** (identity known only when it is the current executable)
- **Claude CLI candidate** (discovered on PATH; version unknown without invocation)

## Privacy

Do not echo:
- Raw arbitrary hook commands (normalize recognized invocations, mark others unsupported)
- All settings content (point to source JSON location)
- Full environment variables (redact diagnostic excerpts)

## Future evolution

This schema is frozen for the #84 skeleton. Coverage/failure behavior/findings will
gain detail in #85-#87, but the top-level shape and required fields are stable.

## References

- Parent: [Linear AI2-25](https://linear.app/ai2rules/issue/AI2-25/)
- Spec: [GitHub #84](https://github.com/sv-pro/ai2rules/issues/84)
- Implementation: `crates/cli-harness/src/doctor.rs`

---

**Revision history:**
- v0alpha3 (this): Corrections from #84 review (2026-09)
- v0alpha2: Unshipped chat proposal (superseded)
- v0alpha1: Internal draft (superseded)
