//! harness doctor — effective coverage report (AI2-25 / #84, #85)
//!
//! Static inspection of Claude Code CLI project profile; no execution, no writes.
//! Schema: ai2rules.dev/harness-doctor/v0alpha3
//! Profile: claude-code-cli/static-v1

use crate::doctor_collector::{
    check_bash_available, check_script_readable, inspect_settings, observe_kill_switches,
    recognize_shim, reduce_kill_switches, resolve_path, InspectionContext, KillSwitchKind,
    KillSwitchState, ShimProfile,
};
use compiler::loader::load_yaml;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Top-level doctor report matching v0alpha3 contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub schema_version: String,
    pub profile: String,
    pub project: ProjectContext,
    pub summary: Summary,
    pub binaries: Binaries,
    pub settings: Settings,
    pub hook: Hook,
    pub manifest: Manifest,
    pub switches: Switches,
    pub coverage: Coverage,
    pub failure_behavior: FailureBehavior,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub installation: InstallationStatus,
    pub runtime_enforcement: RuntimeEnforcement,
    pub coverage: CoverageStatus,
    pub exit_code: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallationStatus {
    Ok,
    Partial,
    Disabled,
    Unknown,
    Broken,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeEnforcement {
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    Partial,
    None,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binaries {
    pub doctor: Binary,
    pub hook_harness: Binary,
    pub claude_candidate: Binary,
    pub interpreter: Binary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Binary {
    pub path: Option<String>,
    pub resolved_path: Option<String>,
    pub executable: Option<bool>,
    pub version: Option<String>,
    pub version_source: VersionSource,
    pub status: BinaryStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionSource {
    BuildMetadata,
    SameExecutable,
    NotProbed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BinaryStatus {
    Found,
    Missing,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub project_path: Option<String>,
    pub status: SettingsStatus,
    pub observed_sources: Vec<SettingsSource>,
    pub effective_configuration: EffectiveConfiguration,
    pub uninspected_sources: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsStatus {
    Valid,
    Missing,
    Invalid,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsSource {
    pub path: String,
    pub scope: String,
    pub status: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveConfiguration {
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    pub status: HookStatus,
    pub entries: Vec<HookEntry>,
    pub shim_path: Option<String>,
    pub shim_resolved_path: Option<String>,
    pub shim_profile: Option<String>,
    pub grant: Option<bool>,
    pub enforce_absent: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookStatus {
    Recognized,
    Missing,
    Ambiguous,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookEntry {
    pub source_path: String,
    pub json_pointer: String,
    pub matcher: Option<String>,
    pub kind: String,
    pub invocation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub path: Option<String>,
    pub compile_status: CompileStatus,
    pub hash: Option<String>,
    pub hash_kind: HashKind,
    pub resolution_context: ResolutionContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompileStatus {
    Ok,
    Missing,
    Invalid,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HashKind {
    ResolvedWorldManifestSha256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionContext {
    pub project_dir: Option<String>,
    pub home: Option<String>,
    pub base_source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Switches {
    pub observed: Vec<ObservedSwitch>,
    pub effective_in_inspected_context: SwitchState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedSwitch {
    pub kind: String,
    pub path: String,
    pub state: SwitchState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwitchState {
    On,
    Off,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coverage {
    pub configured_matchers: Vec<String>,
    pub projected_actions: Vec<String>,
    pub projected_actions_status: ProjectedActionsStatus,
    pub live_native_inventory: LiveInventory,
    pub live_mcp_inventory: LiveInventory,
    pub absent_behavior: AbsentBehavior,
    pub downstream_execution: DownstreamExecution,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectedActionsStatus {
    Known,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiveInventory {
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbsentBehavior {
    PassThrough,
    Deny,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownstreamExecution {
    NotIndividuallyMediated,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureBehavior {
    pub shim_missing_binary: FailureMode,
    pub adapter_process_error: FailureMode,
    pub state_persistence_error: FailureMode,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureMode {
    Open,
    ConditionalDeny,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub code: String,
    pub severity: Severity,
    pub component: String,
    pub message: String,
    pub evidence_paths: Vec<String>,
    pub affects_installation: bool,
    pub next_action: Option<NextAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextAction {
    pub kind: String,
    pub argv: Option<Vec<String>>,
    pub message: String,
}

impl DoctorReport {
    /// Create a full inspection report
    pub fn inspect(cwd: &Path) -> Self {
        let ctx = InspectionContext::capture();
        let doctor_version = env!("CARGO_PKG_VERSION");

        let mut findings = vec![];
        let project_root = cwd.to_path_buf();

        // Inspect settings
        let settings_info = inspect_settings(&project_root);
        let settings_status = match settings_info.status {
            crate::doctor_collector::SettingsStatus::Valid => SettingsStatus::Valid,
            crate::doctor_collector::SettingsStatus::Missing => {
                findings.push(Finding {
                    code: "CC_SETTINGS_MISSING".into(),
                    severity: Severity::Error,
                    component: "settings".into(),
                    message: "Project .claude/settings.json not found".into(),
                    evidence_paths: vec![settings_info.path.to_string_lossy().into()],
                    affects_installation: true,
                    next_action: Some(NextAction {
                        kind: "command".into(),
                        argv: Some(vec!["harness".into(), "init".into()]),
                        message: "Run `harness init` to create settings and hook".into(),
                    }),
                });
                SettingsStatus::Missing
            }
            crate::doctor_collector::SettingsStatus::Invalid => {
                findings.push(Finding {
                    code: "CC_SETTINGS_INVALID".into(),
                    severity: Severity::Error,
                    component: "settings".into(),
                    message: format!(
                        "Settings file invalid: {}",
                        settings_info.error.as_deref().unwrap_or("unknown error")
                    ),
                    evidence_paths: vec![settings_info.path.to_string_lossy().into()],
                    affects_installation: true,
                    next_action: None,
                });
                SettingsStatus::Invalid
            }
            crate::doctor_collector::SettingsStatus::Unreadable => {
                findings.push(Finding {
                    code: "CC_SETTINGS_UNREADABLE".into(),
                    severity: Severity::Error,
                    component: "settings".into(),
                    message: format!(
                        "Settings file unreadable: {}",
                        settings_info.error.as_deref().unwrap_or("unknown error")
                    ),
                    evidence_paths: vec![settings_info.path.to_string_lossy().into()],
                    affects_installation: true,
                    next_action: None,
                });
                SettingsStatus::Unknown
            }
        };

        // Check for PreToolUse registrations
        let pretooluse_hooks: Vec<_> = settings_info
            .hooks
            .iter()
            .filter(|h| h.kind.contains("PreToolUse") || h.command.contains("world-gate.sh"))
            .collect();

        let mut hook_entries = vec![];
        let mut hook_status = HookStatus::Unknown;
        let mut shim_path = None;
        let mut shim_profile_name = None;
        let mut grant_flag = None;
        let mut harness_bin_from_shim = None;
        let mut off_file_from_shim = None;

        if pretooluse_hooks.is_empty() {
            findings.push(Finding {
                code: "CC_PRETOOLUSE_MISSING".into(),
                severity: Severity::Error,
                component: "hook".into(),
                message: "No PreToolUse hook registration found in settings".into(),
                evidence_paths: vec![settings_info.path.to_string_lossy().into()],
                affects_installation: true,
                next_action: Some(NextAction {
                    kind: "command".into(),
                    argv: Some(vec!["harness".into(), "init".into()]),
                    message: "Run `harness init` to register the hook".into(),
                }),
            });
            hook_status = HookStatus::Missing;
        } else {
            // Check for narrow matchers
            let has_match_all = pretooluse_hooks
                .iter()
                .any(|h| h.matcher.as_deref() == Some("*"));
            let narrow_matchers: Vec<_> = pretooluse_hooks
                .iter()
                .filter_map(|h| {
                    h.matcher
                        .as_ref()
                        .filter(|m| m.as_str() != "*")
                        .map(|m| m.as_str())
                })
                .collect();

            if !has_match_all && !narrow_matchers.is_empty() {
                findings.push(Finding {
                    code: "CC_PRETOOLUSE_NARROW".into(),
                    severity: Severity::Warning,
                    component: "hook".into(),
                    message: format!(
                        "PreToolUse hook uses narrow matchers only: {}",
                        narrow_matchers.join(", ")
                    ),
                    evidence_paths: vec![settings_info.path.to_string_lossy().into()],
                    affects_installation: true,
                    next_action: Some(NextAction {
                        kind: "manual".into(),
                        argv: None,
                        message: "Consider using matcher '*' for full coverage".into(),
                    }),
                });
                hook_status = HookStatus::Unsupported;
            }

            if pretooluse_hooks.len() > 1 && has_match_all {
                findings.push(Finding {
                    code: "CC_PRETOOLUSE_AMBIGUOUS".into(),
                    severity: Severity::Warning,
                    component: "hook".into(),
                    message: format!(
                        "Multiple PreToolUse registrations found ({} entries)",
                        pretooluse_hooks.len()
                    ),
                    evidence_paths: vec![settings_info.path.to_string_lossy().into()],
                    affects_installation: false,
                    next_action: None,
                });
                hook_status = HookStatus::Ambiguous;
            }

            // Extract shim path from command (recognize: bash '<path>')
            for hook in &pretooluse_hooks {
                let invocation = if hook.command.contains("world-gate.sh") {
                    if let Some(path) = extract_shim_path(&hook.command) {
                        shim_path = Some(PathBuf::from(path.clone()));
                        format!("bash '{}'", path)
                    } else {
                        "unsupported".into()
                    }
                } else {
                    "unsupported".into()
                };

                hook_entries.push(HookEntry {
                    source_path: settings_info.path.to_string_lossy().into(),
                    json_pointer: hook.json_pointer.clone(),
                    matcher: hook.matcher.clone(),
                    kind: hook.kind.clone(),
                    invocation,
                });
            }

            if shim_path.is_none() {
                findings.push(Finding {
                    code: "CC_SHIM_UNSUPPORTED".into(),
                    severity: Severity::Error,
                    component: "hook".into(),
                    message: "Hook command does not match recognized shim pattern".into(),
                    evidence_paths: vec![settings_info.path.to_string_lossy().into()],
                    affects_installation: true,
                    next_action: None,
                });
                hook_status = HookStatus::Unsupported;
            }
        }

        // Recognize and parse shim
        let mut shim_resolved_path = None;
        if let Some(ref spath) = shim_path {
            if !spath.exists() {
                findings.push(Finding {
                    code: "CC_SHIM_MISSING".into(),
                    severity: Severity::Error,
                    component: "hook".into(),
                    message: "Shim script not found".into(),
                    evidence_paths: vec![spath.to_string_lossy().into()],
                    affects_installation: true,
                    next_action: Some(NextAction {
                        kind: "command".into(),
                        argv: Some(vec!["harness".into(), "init".into()]),
                        message: "Run `harness init` to create the shim".into(),
                    }),
                });
            } else {
                // Check if script is readable
                if let Err(e) = check_script_readable(spath) {
                    findings.push(Finding {
                        code: "CC_SHIM_UNREADABLE".into(),
                        severity: Severity::Error,
                        component: "hook".into(),
                        message: format!("Shim script not readable: {}", e),
                        evidence_paths: vec![spath.to_string_lossy().into()],
                        affects_installation: true,
                        next_action: None,
                    });
                } else {
                    // Recognize shim
                    let shim_info = recognize_shim(spath);
                    shim_resolved_path = resolve_path(spath)
                        .resolved_path
                        .map(|p| p.to_string_lossy().into());

                    match shim_info.profile {
                        Some(ShimProfile::CurrentInitGenerated {
                            ref harness_bin,
                            ref off_file,
                            grant,
                        }) => {
                            shim_profile_name = Some("current_init_generated".into());
                            grant_flag = Some(grant);
                            harness_bin_from_shim = Some(harness_bin.clone());
                            off_file_from_shim = Some(off_file.clone());
                            hook_status = if hook_status == HookStatus::Unknown {
                                HookStatus::Recognized
                            } else {
                                hook_status
                            };
                        }
                        Some(ShimProfile::Unsupported) => {
                            findings.push(Finding {
                                code: "CC_SHIM_UNSUPPORTED".into(),
                                severity: Severity::Warning,
                                component: "hook".into(),
                                message: "Shim script does not match recognized patterns".into(),
                                evidence_paths: vec![spath.to_string_lossy().into()],
                                affects_installation: true,
                                next_action: None,
                            });
                            hook_status = HookStatus::Unsupported;
                        }
                        None => {
                            if let Some(err) = shim_info.error {
                                findings.push(Finding {
                                    code: "CC_SHIM_UNREADABLE".into(),
                                    severity: Severity::Error,
                                    component: "hook".into(),
                                    message: format!("Cannot read shim: {}", err),
                                    evidence_paths: vec![spath.to_string_lossy().into()],
                                    affects_installation: true,
                                    next_action: None,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Check CLAUDE_PROJECT_DIR mismatch
        if let Some(ref cpd) = ctx.claude_project_dir {
            let expected_root = Path::new(cpd);
            if expected_root != project_root {
                findings.push(Finding {
                    code: "CC_SHIM_PROJECT_MISMATCH".into(),
                    severity: Severity::Error,
                    component: "hook".into(),
                    message: format!(
                        "CLAUDE_PROJECT_DIR ({}) does not match current directory ({})",
                        cpd,
                        project_root.display()
                    ),
                    evidence_paths: vec![],
                    affects_installation: true,
                    next_action: None,
                });
            }
        }

        // Check bash interpreter
        let bash_available = check_bash_available(ctx.path.as_deref());
        if !bash_available {
            findings.push(Finding {
                code: "CC_INTERPRETER_MISSING".into(),
                severity: Severity::Error,
                component: "binary".into(),
                message: "bash interpreter not found in PATH".into(),
                evidence_paths: vec![],
                affects_installation: true,
                next_action: Some(NextAction {
                    kind: "manual".into(),
                    argv: None,
                    message: "Install bash".into(),
                }),
            });
        }

        // Resolve harness binary per HARNESS_BIN precedence
        let (harness_binary_info, harness_version, harness_version_source) =
            if let Some(ref hb) = ctx.harness_bin {
                if !hb.is_empty() {
                    let hb_path = PathBuf::from(hb);
                    if !hb_path.is_absolute() {
                        findings.push(Finding {
                            code: "CC_HARNESS_OVERRIDE_INVALID".into(),
                            severity: Severity::Error,
                            component: "binary".into(),
                            message: format!("HARNESS_BIN is not absolute: {}", hb),
                            evidence_paths: vec![],
                            affects_installation: true,
                            next_action: None,
                        });
                        (resolve_path(&hb_path), None, VersionSource::NotProbed)
                    } else {
                        let resolved = resolve_path(&hb_path);
                        if resolved.executable != Some(true) {
                            findings.push(Finding {
                                code: "CC_HARNESS_OVERRIDE_INVALID".into(),
                                severity: Severity::Error,
                                component: "binary".into(),
                                message: format!("HARNESS_BIN is not executable: {}", hb),
                                evidence_paths: vec![hb.clone()],
                                affects_installation: true,
                                next_action: None,
                            });
                        }

                        // Check if it's the same executable as doctor
                        let (version, version_source) = if let Some(ref current) = ctx.current_exe {
                            if resolved.resolved_path.as_ref() == Some(current) {
                                (Some(doctor_version.into()), VersionSource::SameExecutable)
                            } else {
                                (None, VersionSource::NotProbed)
                            }
                        } else {
                            (None, VersionSource::NotProbed)
                        };

                        (resolved, version, version_source)
                    }
                } else if let Some(ref bin) = harness_bin_from_shim {
                    let bin_path = PathBuf::from(bin);
                    let resolved = resolve_path(&bin_path);

                    let (version, version_source) = if let Some(ref current) = ctx.current_exe {
                        if resolved.resolved_path.as_ref() == Some(current) {
                            (Some(doctor_version.into()), VersionSource::SameExecutable)
                        } else {
                            (None, VersionSource::NotProbed)
                        }
                    } else {
                        (None, VersionSource::NotProbed)
                    };

                    (resolved, version, version_source)
                } else {
                    (
                        resolve_path(&PathBuf::from("/nonexistent")),
                        None,
                        VersionSource::NotProbed,
                    )
                }
            } else if let Some(ref bin) = harness_bin_from_shim {
                let bin_path = PathBuf::from(bin);
                let resolved = resolve_path(&bin_path);

                let (version, version_source) = if let Some(ref current) = ctx.current_exe {
                    if resolved.resolved_path.as_ref() == Some(current) {
                        (Some(doctor_version.into()), VersionSource::SameExecutable)
                    } else {
                        (None, VersionSource::NotProbed)
                    }
                } else {
                    (None, VersionSource::NotProbed)
                };

                (resolved, version, version_source)
            } else {
                findings.push(Finding {
                    code: "CC_EXECUTABLE_MISSING".into(),
                    severity: Severity::Error,
                    component: "binary".into(),
                    message: "Cannot determine harness binary path".into(),
                    evidence_paths: vec![],
                    affects_installation: true,
                    next_action: None,
                });
                (
                    resolve_path(&PathBuf::from("/nonexistent")),
                    None,
                    VersionSource::NotProbed,
                )
            };

        // Check if binary is inside governed project
        if let Some(ref resolved) = harness_binary_info.resolved_path {
            if resolved.starts_with(&project_root) {
                findings.push(Finding {
                    code: "CC_BINARY_IN_PROJECT".into(),
                    severity: Severity::Warning,
                    component: "binary".into(),
                    message: format!(
                        "Harness binary is inside governed project: {}",
                        resolved.display()
                    ),
                    evidence_paths: vec![resolved.to_string_lossy().into()],
                    affects_installation: false,
                    next_action: Some(NextAction {
                        kind: "manual".into(),
                        argv: None,
                        message: "Consider moving binary outside governed project".into(),
                    }),
                });
            }
        }

        let hook_harness_status = if harness_binary_info.resolved_path.is_some()
            && harness_binary_info.executable == Some(true)
        {
            BinaryStatus::Found
        } else if harness_binary_info.error.is_some() {
            findings.push(Finding {
                code: "CC_EXECUTABLE_MISSING".into(),
                severity: Severity::Error,
                component: "binary".into(),
                message: format!(
                    "Harness binary not found or not accessible: {}",
                    harness_binary_info
                        .error
                        .as_deref()
                        .unwrap_or("unknown error")
                ),
                evidence_paths: harness_binary_info
                    .logical_path
                    .as_ref()
                    .map(|p| vec![p.to_string_lossy().into()])
                    .unwrap_or_default(),
                affects_installation: true,
                next_action: None,
            });
            BinaryStatus::Missing
        } else {
            BinaryStatus::Unknown
        };

        // Look for Claude CLI on PATH
        let claude_path = ctx
            .path
            .as_ref()
            .and_then(|path| find_executable_on_path("claude", path));
        let claude_status = if claude_path.is_some() {
            BinaryStatus::Found
        } else {
            findings.push(Finding {
                code: "CC_CLI_NOT_FOUND".into(),
                severity: Severity::Info,
                component: "binary".into(),
                message: "Claude CLI not found on PATH".into(),
                evidence_paths: vec![],
                affects_installation: false,
                next_action: None,
            });
            BinaryStatus::Missing
        };

        // Observe kill switches
        let kill_switches = observe_kill_switches(
            &project_root,
            ctx.home.as_deref(),
            off_file_from_shim.as_deref(),
        );
        let effective_switch_state = reduce_kill_switches(&kill_switches);

        let observed_switches: Vec<_> = kill_switches
            .iter()
            .map(|sw| ObservedSwitch {
                kind: match sw.kind {
                    KillSwitchKind::ProjectModern => "project_modern".into(),
                    KillSwitchKind::UserGlobal => "user_global".into(),
                    KillSwitchKind::ProjectLegacy => "project_legacy".into(),
                },
                path: sw.path.to_string_lossy().into(),
                state: match sw.state {
                    KillSwitchState::On => SwitchState::On,
                    KillSwitchState::Off => SwitchState::Off,
                    KillSwitchState::Unknown => SwitchState::Unknown,
                },
            })
            .collect();

        // Check for legacy project switch
        if kill_switches
            .iter()
            .any(|sw| sw.kind == KillSwitchKind::ProjectLegacy)
        {
            findings.push(Finding {
                code: "CC_KILL_SWITCH_LEGACY".into(),
                severity: Severity::Warning,
                component: "switches".into(),
                message: "Legacy project kill switch detected (project/.claude/gate-off)".into(),
                evidence_paths: kill_switches
                    .iter()
                    .filter(|sw| sw.kind == KillSwitchKind::ProjectLegacy)
                    .map(|sw| sw.path.to_string_lossy().into())
                    .collect(),
                affects_installation: false,
                next_action: Some(NextAction {
                    kind: "manual".into(),
                    argv: None,
                    message: "Modern init uses HOME/.claude/ai2rules/off/<slug>".into(),
                }),
            });
        }

        if effective_switch_state == KillSwitchState::On {
            findings.push(Finding {
                code: "CC_KILL_SWITCH_ENABLED".into(),
                severity: Severity::Warning,
                component: "switches".into(),
                message: "Kill switch is enabled; governance is disabled".into(),
                evidence_paths: kill_switches
                    .iter()
                    .filter(|sw| sw.state == KillSwitchState::On)
                    .map(|sw| sw.path.to_string_lossy().into())
                    .collect(),
                affects_installation: true,
                next_action: Some(NextAction {
                    kind: "manual".into(),
                    argv: None,
                    message: "Remove kill switch file to re-enable governance".into(),
                }),
            });
        } else if effective_switch_state == KillSwitchState::Unknown {
            findings.push(Finding {
                code: "CC_KILL_SWITCH_UNKNOWN".into(),
                severity: Severity::Info,
                component: "switches".into(),
                message: "Kill switch state could not be determined".into(),
                evidence_paths: vec![],
                affects_installation: false,
                next_action: None,
            });
        }

        // Check manifest
        let manifest_path = project_root.join(".claude/cc-world.yaml");
        let (manifest_status, manifest_hash) = if !manifest_path.exists() {
            findings.push(Finding {
                code: "CC_MANIFEST_MISSING".into(),
                severity: Severity::Error,
                component: "manifest".into(),
                message: "World manifest not found".into(),
                evidence_paths: vec![manifest_path.to_string_lossy().into()],
                affects_installation: true,
                next_action: Some(NextAction {
                    kind: "command".into(),
                    argv: Some(vec!["harness".into(), "init".into()]),
                    message: "Run `harness init` to create manifest".into(),
                }),
            });
            (CompileStatus::Missing, None)
        } else {
            match std::fs::read_to_string(&manifest_path) {
                Ok(content) => match load_yaml(&content) {
                    Ok(_manifest) => {
                        // Successfully compiled
                        let hash = format!("{:x}", md5::compute(&content));
                        (CompileStatus::Ok, Some(hash))
                    }
                    Err(e) => {
                        findings.push(Finding {
                            code: "CC_MANIFEST_INVALID".into(),
                            severity: Severity::Error,
                            component: "manifest".into(),
                            message: format!("Manifest compilation failed: {}", e),
                            evidence_paths: vec![manifest_path.to_string_lossy().into()],
                            affects_installation: true,
                            next_action: None,
                        });
                        (CompileStatus::Invalid, None)
                    }
                },
                Err(e) => {
                    findings.push(Finding {
                        code: "CC_MANIFEST_UNREADABLE".into(),
                        severity: Severity::Error,
                        component: "manifest".into(),
                        message: format!("Cannot read manifest: {}", e),
                        evidence_paths: vec![manifest_path.to_string_lossy().into()],
                        affects_installation: true,
                        next_action: None,
                    });
                    (CompileStatus::Unknown, None)
                }
            }
        };

        // Boundary honesty: list uninspected sources
        let uninspected = vec![
            "Local/user settings".into(),
            "CLI overrides".into(),
            "Session configuration".into(),
            "Managed/remote policy".into(),
        ];

        findings.push(Finding {
            code: "CC_SETTINGS_SCOPE_UNVERIFIED".into(),
            severity: Severity::Info,
            component: "settings".into(),
            message: "Some settings sources not inspected; effective configuration unknown".into(),
            evidence_paths: vec![],
            affects_installation: false,
            next_action: None,
        });

        // Sort findings by severity then code
        findings.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then_with(|| a.code.cmp(&b.code))
        });

        // Reduce installation status
        let installation_status = if effective_switch_state == KillSwitchState::On {
            InstallationStatus::Disabled
        } else if findings
            .iter()
            .any(|f| f.severity == Severity::Error && f.affects_installation)
        {
            InstallationStatus::Broken
        } else if findings
            .iter()
            .any(|f| f.severity == Severity::Warning && f.affects_installation)
        {
            InstallationStatus::Partial
        } else if hook_status == HookStatus::Recognized
            && settings_status == SettingsStatus::Valid
            && manifest_status == CompileStatus::Ok
        {
            InstallationStatus::Ok
        } else {
            InstallationStatus::Unknown
        };

        let coverage_status = if installation_status == InstallationStatus::Ok {
            CoverageStatus::Partial
        } else if installation_status == InstallationStatus::Broken
            || installation_status == InstallationStatus::Disabled
        {
            CoverageStatus::None
        } else {
            CoverageStatus::Unknown
        };

        let exit_code = reduce_exit_code_from_status(installation_status);

        DoctorReport {
            schema_version: "ai2rules.dev/harness-doctor/v0alpha3".into(),
            profile: "claude-code-cli/static-v1".into(),
            project: ProjectContext {
                path: cwd.to_str().map(String::from),
            },
            summary: Summary {
                installation: installation_status,
                runtime_enforcement: RuntimeEnforcement::Unknown,
                coverage: coverage_status,
                exit_code,
            },
            binaries: Binaries {
                doctor: Binary {
                    path: ctx.current_exe.as_ref().map(|p| p.to_string_lossy().into()),
                    resolved_path: ctx
                        .current_exe
                        .as_ref()
                        .and_then(|p| std::fs::canonicalize(p).ok())
                        .map(|p| p.to_string_lossy().into()),
                    executable: Some(true),
                    version: Some(doctor_version.into()),
                    version_source: VersionSource::BuildMetadata,
                    status: BinaryStatus::Found,
                },
                hook_harness: Binary {
                    path: harness_binary_info
                        .logical_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into()),
                    resolved_path: harness_binary_info
                        .resolved_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into()),
                    executable: harness_binary_info.executable,
                    version: harness_version,
                    version_source: harness_version_source,
                    status: hook_harness_status,
                },
                claude_candidate: Binary {
                    path: claude_path.as_ref().map(|p| p.to_string_lossy().into()),
                    resolved_path: claude_path.map(|p| p.to_string_lossy().into()),
                    executable: Some(true),
                    version: None,
                    version_source: VersionSource::NotProbed,
                    status: claude_status,
                },
                interpreter: Binary {
                    path: if bash_available {
                        Some("bash".into())
                    } else {
                        None
                    },
                    resolved_path: None,
                    executable: Some(bash_available),
                    version: None,
                    version_source: VersionSource::NotProbed,
                    status: if bash_available {
                        BinaryStatus::Found
                    } else {
                        BinaryStatus::Missing
                    },
                },
            },
            settings: Settings {
                project_path: Some(settings_info.path.to_string_lossy().into()),
                status: settings_status,
                observed_sources: vec![SettingsSource {
                    path: settings_info.path.to_string_lossy().into(),
                    scope: "folder".into(),
                    status: match settings_status {
                        SettingsStatus::Valid => "valid".into(),
                        SettingsStatus::Missing => "missing".into(),
                        SettingsStatus::Invalid => "invalid".into(),
                        SettingsStatus::Unknown => "unknown".into(),
                    },
                }],
                effective_configuration: EffectiveConfiguration::Unknown,
                uninspected_sources: uninspected,
            },
            hook: Hook {
                status: hook_status,
                entries: hook_entries,
                shim_path: shim_path.map(|p| p.to_string_lossy().into()),
                shim_resolved_path,
                shim_profile: shim_profile_name.clone(),
                grant: grant_flag,
                enforce_absent: None,
            },
            manifest: Manifest {
                path: Some(manifest_path.to_string_lossy().into()),
                compile_status: manifest_status,
                hash: manifest_hash,
                hash_kind: HashKind::ResolvedWorldManifestSha256,
                resolution_context: ResolutionContext {
                    project_dir: Some(project_root.to_string_lossy().into()),
                    home: ctx.home.clone(),
                    base_source: None,
                },
            },
            switches: Switches {
                observed: observed_switches,
                effective_in_inspected_context: match effective_switch_state {
                    KillSwitchState::On => SwitchState::On,
                    KillSwitchState::Off => SwitchState::Off,
                    KillSwitchState::Unknown => SwitchState::Unknown,
                },
            },
            coverage: Coverage {
                configured_matchers: pretooluse_hooks
                    .iter()
                    .filter_map(|h| h.matcher.clone())
                    .collect(),
                projected_actions: vec![],
                projected_actions_status: ProjectedActionsStatus::Unknown,
                live_native_inventory: LiveInventory::Unknown,
                live_mcp_inventory: LiveInventory::Unknown,
                absent_behavior: AbsentBehavior::Unknown,
                downstream_execution: DownstreamExecution::Unknown,
                limitations: vec![
                    "Static inspection only; no runtime session".into(),
                    "Claude candidate version unknown without invocation".into(),
                    "Coverage analysis not yet implemented (#86)".into(),
                ],
            },
            failure_behavior: FailureBehavior {
                shim_missing_binary: if shim_profile_name.is_some() {
                    FailureMode::Open
                } else {
                    FailureMode::Unknown
                },
                adapter_process_error: FailureMode::Open,
                state_persistence_error: FailureMode::ConditionalDeny,
                evidence: if shim_profile_name.is_some() {
                    vec![
                        "Recognized shim pattern: fail-open on missing binary".into(),
                        "Adapter errors fail open (exit 0)".into(),
                    ]
                } else {
                    vec![]
                },
            },
            findings,
        }
    }
}

/// Extract shim path from command (recognize: bash '<path>')
fn extract_shim_path(command: &str) -> Option<String> {
    let trimmed = command.trim();
    if !trimmed.starts_with("bash") {
        return None;
    }

    let after_bash = trimmed.strip_prefix("bash")?.trim();
    if !after_bash.starts_with('\'') {
        return None;
    }

    let path_part = after_bash.strip_prefix('\'')?.split('\'').next()?;
    Some(path_part.to_string())
}

/// Find an executable on PATH
fn find_executable_on_path(name: &str, path_env: &str) -> Option<PathBuf> {
    for dir in path_env.split(':') {
        let exe_path = Path::new(dir).join(name);
        if exe_path.exists() && exe_path.is_file() {
            return Some(exe_path);
        }
    }
    None
}

/// Exit code reduction: priority error > broken > disabled > partial > unknown > ok
fn reduce_exit_code_from_status(status: InstallationStatus) -> i32 {
    use InstallationStatus::*;

    match status {
        Error => 3,
        Broken => 2,
        Disabled => 1,
        Partial => 1,
        Unknown => 1,
        Ok => 0,
    }
}

/// Exit code reduction from report (backward compat with skeleton)
#[allow(dead_code)]
pub fn reduce_exit_code(report: &DoctorReport) -> i32 {
    report.summary.exit_code
}

/// Render the human-readable report heading
pub fn format_heading(status: InstallationStatus) -> String {
    match status {
        InstallationStatus::Ok => {
            "INSTALLATION OK — inspected Claude Code CLI project profile".into()
        }
        InstallationStatus::Partial => {
            "INSTALLATION PARTIAL — inspected Claude Code CLI project profile".into()
        }
        InstallationStatus::Disabled => {
            "INSTALLATION DISABLED — inspected Claude Code CLI project profile".into()
        }
        InstallationStatus::Unknown => {
            "INSTALLATION UNKNOWN — inspected Claude Code CLI project profile".into()
        }
        InstallationStatus::Broken => {
            "INSTALLATION BROKEN — inspected Claude Code CLI project profile".into()
        }
        InstallationStatus::Error => {
            "INSTALLATION ERROR — inspected Claude Code CLI project profile".into()
        }
    }
}

/// Run the doctor command: inspect installation and report status
pub fn run(json_output: bool) -> i32 {
    let cwd = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("doctor: cannot determine current directory: {e}");
            return 3;
        }
    };

    let report = DoctorReport::inspect(&cwd);
    let exit_code = report.summary.exit_code;

    if json_output {
        match serde_json::to_string(&report) {
            Ok(json) => {
                println!("{json}");
            }
            Err(e) => {
                eprintln!("doctor: cannot serialize report: {e}");
                return 3;
            }
        }
    } else {
        println!("{}", format_heading(report.summary.installation));
        println!();
        println!("Schema: {}", report.schema_version);
        println!("Profile: {}", report.profile);
        if let Some(path) = &report.project.path {
            println!("Project: {path}");
        }
        println!();
        println!("Installation: {:?}", report.summary.installation);
        println!(
            "Runtime enforcement: {:?}",
            report.summary.runtime_enforcement
        );
        println!("Coverage: {:?}", report.summary.coverage);
        println!();

        if !report.coverage.limitations.is_empty() {
            println!("Limitations:");
            for limitation in &report.coverage.limitations {
                println!("  - {limitation}");
            }
            println!();
        }

        if !report.findings.is_empty() {
            println!("Findings:");
            for finding in &report.findings {
                println!(
                    "  [{:?}] {}: {}",
                    finding.severity, finding.code, finding.message
                );
                if let Some(action) = &finding.next_action {
                    println!("    Next: {}", action.message);
                }
            }
            println!();
        }
    }

    exit_code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_code_reduction_priority() {
        assert_eq!(reduce_exit_code_from_status(InstallationStatus::Ok), 0);
        assert_eq!(reduce_exit_code_from_status(InstallationStatus::Unknown), 1);
        assert_eq!(reduce_exit_code_from_status(InstallationStatus::Partial), 1);
        assert_eq!(
            reduce_exit_code_from_status(InstallationStatus::Disabled),
            1
        );
        assert_eq!(reduce_exit_code_from_status(InstallationStatus::Broken), 2);
        assert_eq!(reduce_exit_code_from_status(InstallationStatus::Error), 3);
    }

    #[test]
    fn test_format_heading_never_says_protected() {
        for status in [
            InstallationStatus::Ok,
            InstallationStatus::Partial,
            InstallationStatus::Disabled,
            InstallationStatus::Unknown,
            InstallationStatus::Broken,
            InstallationStatus::Error,
        ] {
            let heading = format_heading(status);
            assert!(
                !heading.contains("protected"),
                "heading must not say 'protected' (got: {heading})"
            );
            assert!(
                heading.contains("inspected Claude Code CLI project profile"),
                "heading must mention profile (got: {heading})"
            );
        }
    }

    #[test]
    fn test_extract_shim_path() {
        assert_eq!(
            extract_shim_path("bash '/path/to/world-gate.sh'"),
            Some("/path/to/world-gate.sh".into())
        );

        assert_eq!(
            extract_shim_path("  bash  '/some/path.sh'  "),
            Some("/some/path.sh".into())
        );

        assert_eq!(extract_shim_path("bash /path/no/quotes"), None);
        assert_eq!(extract_shim_path("sh '/path'"), None);
    }
}
