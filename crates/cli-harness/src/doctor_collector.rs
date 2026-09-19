//! Doctor collector — filesystem/environment inspection for Claude Code CLI hooks
//!
//! Read-only inspection of project hooks, binaries, manifests, and kill switches.
//! No execution, no installation, no repair.
//!
//! Implementation of #85 (AI2-25).

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// Maximum file size to read (settings, shim, manifest)
const MAX_FILE_SIZE: u64 = 1024 * 1024; // 1 MiB

/// Immutable inspection context captured at entry
#[derive(Debug, Clone)]
pub struct InspectionContext {
    pub cwd: PathBuf,
    pub current_exe: Option<PathBuf>,
    pub home: Option<String>,
    pub path: Option<String>,
    pub claude_project_dir: Option<String>,
    pub harness_bin: Option<String>,
}

impl InspectionContext {
    /// Capture the current environment context (allowlisted env vars only)
    pub fn capture() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            current_exe: std::env::current_exe().ok(),
            home: std::env::var("HOME").ok(),
            path: std::env::var("PATH").ok(),
            claude_project_dir: std::env::var("CLAUDE_PROJECT_DIR").ok(),
            harness_bin: std::env::var("HARNESS_BIN").ok(),
        }
    }
}

/// Settings.json parse result
#[derive(Debug, Clone)]
pub struct SettingsInfo {
    pub path: PathBuf,
    pub status: SettingsStatus,
    pub hooks: Vec<HookRegistration>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsStatus {
    Valid,
    Missing,
    Invalid,
    Unreadable,
}

/// One PreToolUse registration found in settings
#[derive(Debug, Clone)]
pub struct HookRegistration {
    pub json_pointer: String,
    pub matcher: Option<String>,
    pub command: String,
    pub kind: String,
    pub sync: Option<bool>,
    pub condition: Option<String>,
}

/// Shim recognition result
#[derive(Debug, Clone)]
pub struct ShimInfo {
    pub path: PathBuf,
    pub profile: Option<ShimProfile>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShimProfile {
    CurrentInitGenerated {
        harness_bin: String,
        off_file: String,
        grant: bool,
    },
    Unsupported,
}

/// Binary resolution result
#[derive(Debug, Clone)]
pub struct BinaryInfo {
    pub logical_path: Option<PathBuf>,
    pub resolved_path: Option<PathBuf>,
    pub executable: Option<bool>,
    pub symlink_chain: Vec<PathBuf>,
    pub error: Option<String>,
}

/// Kill switch observation
#[derive(Debug, Clone)]
pub struct KillSwitch {
    pub kind: KillSwitchKind,
    pub path: PathBuf,
    pub state: KillSwitchState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillSwitchKind {
    ProjectModern,
    UserGlobal,
    ProjectLegacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillSwitchState {
    On,
    Off,
    Unknown,
}

/// Read a file with size bounds and FIFO/device detection
pub fn read_bounded_file(path: &Path) -> Result<String, String> {
    let metadata = fs::metadata(path).map_err(|e| format!("cannot stat: {e}"))?;

    if !metadata.is_file() {
        return Err("not a regular file".into());
    }

    if metadata.len() > MAX_FILE_SIZE {
        return Err(format!(
            "file too large: {} bytes (max {})",
            metadata.len(),
            MAX_FILE_SIZE
        ));
    }

    fs::read_to_string(path).map_err(|e| format!("read error: {e}"))
}

/// Parse .claude/settings.json and extract PreToolUse registrations
pub fn inspect_settings(project_root: &Path) -> SettingsInfo {
    let path = project_root.join(".claude/settings.json");

    if !path.exists() {
        return SettingsInfo {
            path,
            status: SettingsStatus::Missing,
            hooks: vec![],
            error: None,
        };
    }

    let content = match read_bounded_file(&path) {
        Ok(c) => c,
        Err(e) => {
            return SettingsInfo {
                path,
                status: SettingsStatus::Unreadable,
                hooks: vec![],
                error: Some(e),
            }
        }
    };

    let value: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            return SettingsInfo {
                path,
                status: SettingsStatus::Invalid,
                hooks: vec![],
                error: Some(format!("JSON parse error: {e}")),
            }
        }
    };

    if !value.is_object() {
        return SettingsInfo {
            path,
            status: SettingsStatus::Invalid,
            hooks: vec![],
            error: Some("root is not a JSON object".into()),
        };
    }

    let mut hooks = vec![];
    let mut status = SettingsStatus::Valid;
    let mut error = None;

    if let Some(hooks_obj) = value.get("hooks").and_then(|h| h.as_object()) {
        if let Some(pre_tool_use) = hooks_obj.get("PreToolUse") {
            match pre_tool_use {
                Value::Array(arr) => {
                    for (i, entry) in arr.iter().enumerate() {
                        if let Some(obj) = entry.as_object() {
                            let matcher = obj
                                .get("matcher")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            
                            // Extract hooks array within each entry
                            if let Some(hooks_arr) = obj.get("hooks").and_then(|h| h.as_array()) {
                                for (j, hook) in hooks_arr.iter().enumerate() {
                                    let command = hook
                                        .get("command")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let hook_type = hook
                                        .get("type")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown")
                                        .to_string();
                                    let sync = hook.get("sync").and_then(|v| v.as_bool());
                                    let condition = hook
                                        .get("condition")
                                        .and_then(|v| v.as_str())
                                        .map(String::from);

                                    hooks.push(HookRegistration {
                                        json_pointer: format!("/hooks/PreToolUse/{}/hooks/{}", i, j),
                                        matcher: matcher.clone(),
                                        command,
                                        kind: format!("PreToolUse/{}", hook_type),
                                        sync,
                                        condition,
                                    });
                                }
                            }
                        } else {
                            status = SettingsStatus::Invalid;
                            error = Some(format!("PreToolUse[{}] is not an object", i));
                            break;
                        }
                    }
                }
                _ => {
                    status = SettingsStatus::Invalid;
                    error = Some("hooks.PreToolUse is not an array".into());
                }
            }
        }
    }

    SettingsInfo {
        path,
        status,
        hooks,
        error,
    }
}

/// Recognize the current `harness init` generated bash shim exactly
pub fn recognize_shim(shim_path: &Path) -> ShimInfo {
    let content = match read_bounded_file(shim_path) {
        Ok(c) => c,
        Err(e) => {
            return ShimInfo {
                path: shim_path.to_path_buf(),
                profile: None,
                error: Some(e),
            }
        }
    };

    // Parse current init-generated shim
    if let Some(profile) = parse_current_init_shim(&content) {
        return ShimInfo {
            path: shim_path.to_path_buf(),
            profile: Some(profile),
            error: None,
        };
    }

    ShimInfo {
        path: shim_path.to_path_buf(),
        profile: Some(ShimProfile::Unsupported),
        error: None,
    }
}

/// Parse the current `harness init` generated shim format
fn parse_current_init_shim(content: &str) -> Option<ShimProfile> {
    // Look for the characteristic patterns
    let has_ai2rules = content.contains("ai2rules governance shim");
    let has_cc_hook = content.contains("cc-hook");
    let has_trusted_bin = content.contains("TRUSTED_BIN=");

    if !has_ai2rules || !has_cc_hook || !has_trusted_bin {
        return None;
    }

    // Extract TRUSTED_BIN (single-quoted, handles escaped quotes)
    let harness_bin = extract_sh_quoted_var(content, "TRUSTED_BIN=")?;

    // Extract off_file from the if condition (handles both quoted and unquoted)
    let off_file = extract_off_file(content)?;

    // Check for --grant flag
    let grant = content.contains("cc-hook --grant");

    Some(ShimProfile::CurrentInitGenerated {
        harness_bin,
        off_file,
        grant,
    })
}

/// Extract a single-quoted shell variable value
fn extract_sh_quoted_var(content: &str, prefix: &str) -> Option<String> {
    let start = content.find(prefix)? + prefix.len();
    let rest = &content[start..];

    if !rest.starts_with('\'') {
        return None;
    }

    let mut chars = rest[1..].chars();
    let mut result = String::new();
    let mut escaped = false;

    loop {
        match chars.next()? {
            '\'' if !escaped => break,
            '\\' if !escaped => {
                escaped = true;
                result.push('\\');
            }
            c => {
                result.push(c);
                escaped = false;
            }
        }
    }

    Some(result)
}

/// Extract the off_file path from the if condition
fn extract_off_file(content: &str) -> Option<String> {
    // Look for: if [ -f {path} ] || [ -f "$HOME/.claude/gate-off" ]
    let if_line = content.lines().find(|l| l.contains("[ -f") && l.contains(".claude/gate-off"))?;

    // Extract first -f test path (may be quoted or unquoted)
    let after_f = if_line.split("[ -f ").nth(1)?;
    let path_part = after_f.split(" ]").next()?.trim();

    // Handle both '$VAR' and simple paths
    if path_part.starts_with('$') {
        // Variable reference like $HOME/.claude/ai2rules/off/...
        // For our purposes, we just record it as-is
        Some(path_part.to_string())
    } else {
        // Direct path, possibly quoted
        Some(path_part.trim_matches('\'').trim_matches('"').to_string())
    }
}

/// Resolve a path following symlinks, detecting cycles and broken targets
pub fn resolve_path(path: &Path) -> BinaryInfo {
    let mut chain = vec![path.to_path_buf()];
    let mut seen = std::collections::HashSet::new();
    let mut current = path.to_path_buf();

    loop {
        if seen.contains(&current) {
            return BinaryInfo {
                logical_path: Some(path.to_path_buf()),
                resolved_path: None,
                executable: None,
                symlink_chain: chain,
                error: Some("symlink cycle detected".into()),
            };
        }
        seen.insert(current.clone());

        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.is_symlink() {
                    match fs::read_link(&current) {
                        Ok(target) => {
                            let next = if target.is_absolute() {
                                target
                            } else {
                                current.parent().unwrap_or(Path::new(".")).join(target)
                            };
                            chain.push(next.clone());
                            current = next;
                        }
                        Err(e) => {
                            return BinaryInfo {
                                logical_path: Some(path.to_path_buf()),
                                resolved_path: None,
                                executable: None,
                                symlink_chain: chain,
                                error: Some(format!("broken symlink: {e}")),
                            }
                        }
                    }
                } else {
                    // Found a regular file/directory
                    let executable = if metadata.is_file() {
                        Some(is_executable(&metadata))
                    } else {
                        None
                    };

                    return BinaryInfo {
                        logical_path: Some(path.to_path_buf()),
                        resolved_path: Some(current.clone()),
                        executable,
                        symlink_chain: chain,
                        error: None,
                    };
                }
            }
            Err(e) => {
                return BinaryInfo {
                    logical_path: Some(path.to_path_buf()),
                    resolved_path: None,
                    executable: None,
                    symlink_chain: chain,
                    error: Some(format!("cannot stat: {e}")),
                }
            }
        }
    }
}

#[cfg(unix)]
fn is_executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let mode = metadata.permissions().mode();
    (mode & 0o111) != 0
}

#[cfg(not(unix))]
fn is_executable(_metadata: &fs::Metadata) -> bool {
    // On non-Unix, we can't easily check execute permission
    // Conservatively return false
    false
}

/// Check if bash interpreter is available
pub fn check_bash_available(path_env: Option<&str>) -> bool {
    if let Some(path) = path_env {
        for dir in path.split(':') {
            let bash_path = Path::new(dir).join("bash");
            if bash_path.exists() {
                return true;
            }
        }
    }
    false
}

/// Check if a script is readable (don't require +x for readable 0644 script)
pub fn check_script_readable(path: &Path) -> Result<(), String> {
    match fs::metadata(path) {
        Ok(_) => {
            // Try to read it
            match fs::read(path) {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("not readable: {e}")),
            }
        }
        Err(e) => Err(format!("cannot stat: {e}")),
    }
}

/// Observe kill switches per shim semantics: regular file test (-f)
pub fn observe_kill_switches(
    project_root: &Path,
    home: Option<&str>,
    modern_off_file: Option<&str>,
) -> Vec<KillSwitch> {
    let mut switches = vec![];

    // Modern project-specific switch (baked in shim)
    if let Some(off_path) = modern_off_file {
        let path = PathBuf::from(off_path);
        let state = match fs::metadata(&path) {
            Ok(md) if md.is_file() => KillSwitchState::On,
            Ok(_) => KillSwitchState::Off, // exists but not a regular file
            Err(_) => KillSwitchState::Off,
        };
        switches.push(KillSwitch {
            kind: KillSwitchKind::ProjectModern,
            path,
            state,
        });
    }

    // Global user switch
    if let Some(home_dir) = home {
        let path = PathBuf::from(home_dir).join(".claude/gate-off");
        let state = match fs::metadata(&path) {
            Ok(md) if md.is_file() => KillSwitchState::On,
            Ok(_) => KillSwitchState::Off,
            Err(_) => KillSwitchState::Off,
        };
        switches.push(KillSwitch {
            kind: KillSwitchKind::UserGlobal,
            path,
            state,
        });
    }

    // Legacy project switch (project/.claude/gate-off)
    // Modern init does NOT use this, but legacy installer did
    let legacy_path = project_root.join(".claude/gate-off");
    if legacy_path.exists() {
        let state = match fs::metadata(&legacy_path) {
            Ok(md) if md.is_file() => KillSwitchState::On,
            Ok(_) => KillSwitchState::Off,
            Err(_) => KillSwitchState::Unknown,
        };
        switches.push(KillSwitch {
            kind: KillSwitchKind::ProjectLegacy,
            path: legacy_path,
            state,
        });
    }

    switches
}

/// Reduce kill switches to effective state
pub fn reduce_kill_switches(switches: &[KillSwitch]) -> KillSwitchState {
    let mut has_on = false;
    let mut has_unknown = false;

    for switch in switches {
        match switch.state {
            KillSwitchState::On => has_on = true,
            KillSwitchState::Unknown => has_unknown = true,
            KillSwitchState::Off => {}
        }
    }

    if has_on {
        KillSwitchState::On
    } else if has_unknown {
        KillSwitchState::Unknown
    } else {
        KillSwitchState::Off
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inspection_context_capture() {
        let ctx = InspectionContext::capture();
        assert!(ctx.cwd.exists());
    }

    #[test]
    fn test_read_bounded_file_missing() {
        let result = read_bounded_file(Path::new("/nonexistent/file"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_current_init_shim() {
        let shim = r#"#!/usr/bin/env bash
# ai2rules governance shim — written by `harness init`. Execs the Rust kernel's
# PreToolUse adapter; no governance logic lives here.
TRUSTED_BIN='/usr/local/bin/harness'
if [ -f /home/user/.claude/ai2rules/off/project-slug ] || [ -f "$HOME/.claude/gate-off" ]; then exit 0; fi
exec "$BIN" cc-hook --world "$PD/.claude/cc-world.yaml" --state "$PD/.claude/state"
"#;

        let profile = parse_current_init_shim(shim);
        assert!(profile.is_some());
        if let Some(ShimProfile::CurrentInitGenerated {
            harness_bin,
            off_file,
            grant,
        }) = profile
        {
            assert_eq!(harness_bin, "/usr/local/bin/harness");
            assert!(off_file.contains("ai2rules/off"));
            assert!(!grant);
        }
    }

    #[test]
    fn test_parse_current_init_shim_with_grant() {
        let shim = r#"#!/usr/bin/env bash
# ai2rules governance shim — written by `harness init`. Execs the Rust kernel's
TRUSTED_BIN='/opt/harness'
if [ -f /home/user/.claude/ai2rules/off/test ] || [ -f "$HOME/.claude/gate-off" ]; then exit 0; fi
exec "$BIN" cc-hook --grant --world "$PD/.claude/cc-world.yaml" --state "$PD/.claude/state"
"#;

        let profile = parse_current_init_shim(shim);
        assert!(profile.is_some());
        if let Some(ShimProfile::CurrentInitGenerated { grant, .. }) = profile {
            assert!(grant);
        }
    }

    #[test]
    fn test_extract_sh_quoted_var() {
        assert_eq!(
            extract_sh_quoted_var("TRUSTED_BIN='/usr/bin/harness'", "TRUSTED_BIN="),
            Some("/usr/bin/harness".into())
        );

        assert_eq!(
            extract_sh_quoted_var("VAR='path/with/\\'quote'", "VAR="),
            Some("path/with/\\'quote".into())
        );
    }

    #[test]
    fn test_reduce_kill_switches_any_on() {
        let switches = vec![
            KillSwitch {
                kind: KillSwitchKind::ProjectModern,
                path: PathBuf::from("/a"),
                state: KillSwitchState::Off,
            },
            KillSwitch {
                kind: KillSwitchKind::UserGlobal,
                path: PathBuf::from("/b"),
                state: KillSwitchState::On,
            },
        ];

        assert_eq!(reduce_kill_switches(&switches), KillSwitchState::On);
    }

    #[test]
    fn test_reduce_kill_switches_all_off() {
        let switches = vec![
            KillSwitch {
                kind: KillSwitchKind::ProjectModern,
                path: PathBuf::from("/a"),
                state: KillSwitchState::Off,
            },
            KillSwitch {
                kind: KillSwitchKind::UserGlobal,
                path: PathBuf::from("/b"),
                state: KillSwitchState::Off,
            },
        ];

        assert_eq!(reduce_kill_switches(&switches), KillSwitchState::Off);
    }

    #[test]
    fn test_reduce_kill_switches_unknown() {
        let switches = vec![
            KillSwitch {
                kind: KillSwitchKind::ProjectModern,
                path: PathBuf::from("/a"),
                state: KillSwitchState::Off,
            },
            KillSwitch {
                kind: KillSwitchKind::UserGlobal,
                path: PathBuf::from("/b"),
                state: KillSwitchState::Unknown,
            },
        ];

        assert_eq!(reduce_kill_switches(&switches), KillSwitchState::Unknown);
    }
}
