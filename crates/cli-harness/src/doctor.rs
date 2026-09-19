//! harness doctor — effective coverage report (AI2-25 / #84)
//!
//! Static inspection of Claude Code CLI project profile; no execution, no writes.
//! Schema: ai2rules.dev/harness-doctor/v0alpha3
//! Profile: claude-code-cli/static-v1

use serde::{Deserialize, Serialize};
use std::path::Path;

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
    /// Create a skeleton report with unknown status for incomplete implementation
    pub fn skeleton(cwd: &Path) -> Self {
        let doctor_version = env!("CARGO_PKG_VERSION");

        Self {
            schema_version: "ai2rules.dev/harness-doctor/v0alpha3".into(),
            profile: "claude-code-cli/static-v1".into(),
            project: ProjectContext {
                path: cwd.to_str().map(String::from),
            },
            summary: Summary {
                installation: InstallationStatus::Unknown,
                runtime_enforcement: RuntimeEnforcement::Unknown,
                coverage: CoverageStatus::Unknown,
                exit_code: 1,
            },
            binaries: Binaries {
                doctor: Binary {
                    path: std::env::current_exe()
                        .ok()
                        .and_then(|p| p.to_str().map(String::from)),
                    resolved_path: std::env::current_exe()
                        .ok()
                        .and_then(|p| std::fs::canonicalize(p).ok())
                        .and_then(|p| p.to_str().map(String::from)),
                    executable: Some(true),
                    version: Some(doctor_version.into()),
                    version_source: VersionSource::BuildMetadata,
                    status: BinaryStatus::Found,
                },
                hook_harness: Binary {
                    path: None,
                    resolved_path: None,
                    executable: None,
                    version: None,
                    version_source: VersionSource::NotProbed,
                    status: BinaryStatus::Unknown,
                },
                claude_candidate: Binary {
                    path: None,
                    resolved_path: None,
                    executable: None,
                    version: None,
                    version_source: VersionSource::NotProbed,
                    status: BinaryStatus::Unknown,
                },
                interpreter: Binary {
                    path: None,
                    resolved_path: None,
                    executable: None,
                    version: None,
                    version_source: VersionSource::NotProbed,
                    status: BinaryStatus::Unknown,
                },
            },
            settings: Settings {
                project_path: None,
                status: SettingsStatus::Unknown,
                observed_sources: vec![],
                effective_configuration: EffectiveConfiguration::Unknown,
                uninspected_sources: vec![],
            },
            hook: Hook {
                status: HookStatus::Unknown,
                entries: vec![],
                shim_path: None,
                shim_resolved_path: None,
                shim_profile: None,
                grant: None,
                enforce_absent: None,
            },
            manifest: Manifest {
                path: None,
                compile_status: CompileStatus::Unknown,
                hash: None,
                hash_kind: HashKind::ResolvedWorldManifestSha256,
                resolution_context: ResolutionContext {
                    project_dir: None,
                    home: None,
                    base_source: None,
                },
            },
            switches: Switches {
                observed: vec![],
                effective_in_inspected_context: SwitchState::Unknown,
            },
            coverage: Coverage {
                configured_matchers: vec![],
                projected_actions: vec![],
                projected_actions_status: ProjectedActionsStatus::Unknown,
                live_native_inventory: LiveInventory::Unknown,
                live_mcp_inventory: LiveInventory::Unknown,
                absent_behavior: AbsentBehavior::Unknown,
                downstream_execution: DownstreamExecution::Unknown,
                limitations: vec![
                    "Incomplete implementation: skeleton only (#84)".into(),
                    "Static inspection not yet implemented (#85)".into(),
                    "Coverage analysis not yet implemented (#86)".into(),
                ],
            },
            failure_behavior: FailureBehavior {
                shim_missing_binary: FailureMode::Unknown,
                adapter_process_error: FailureMode::Unknown,
                state_persistence_error: FailureMode::Unknown,
                evidence: vec![],
            },
            findings: vec![Finding {
                code: "SKELETON_INCOMPLETE".into(),
                severity: Severity::Info,
                component: "doctor".into(),
                message: "This is a skeleton implementation; inspection not yet complete".into(),
                evidence_paths: vec![],
                affects_installation: false,
                next_action: Some(NextAction {
                    kind: "manual".into(),
                    argv: None,
                    message: "Refer to issues #85-#88 for full doctor implementation".into(),
                }),
            }],
        }
    }
}

/// Exit code reduction: priority error > broken > disabled > partial > unknown > ok
pub fn reduce_exit_code(report: &DoctorReport) -> i32 {
    use InstallationStatus::*;

    match report.summary.installation {
        Error => 3,
        Broken => 2,
        Disabled => 1,
        Partial => 1,
        Unknown => 1,
        Ok => 0,
    }
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

    let report = DoctorReport::skeleton(&cwd);
    let exit_code = reduce_exit_code(&report);

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
        // Priority: error > broken > disabled > partial > unknown > ok
        let mut report = DoctorReport::skeleton(Path::new("."));

        report.summary.installation = InstallationStatus::Ok;
        assert_eq!(reduce_exit_code(&report), 0);

        report.summary.installation = InstallationStatus::Unknown;
        assert_eq!(reduce_exit_code(&report), 1);

        report.summary.installation = InstallationStatus::Partial;
        assert_eq!(reduce_exit_code(&report), 1);

        report.summary.installation = InstallationStatus::Disabled;
        assert_eq!(reduce_exit_code(&report), 1);

        report.summary.installation = InstallationStatus::Broken;
        assert_eq!(reduce_exit_code(&report), 2);

        report.summary.installation = InstallationStatus::Error;
        assert_eq!(reduce_exit_code(&report), 3);
    }

    #[test]
    fn test_skeleton_has_required_fields() {
        let report = DoctorReport::skeleton(Path::new("/test"));

        assert_eq!(
            report.schema_version,
            "ai2rules.dev/harness-doctor/v0alpha3"
        );
        assert_eq!(report.profile, "claude-code-cli/static-v1");
        assert_eq!(report.summary.installation, InstallationStatus::Unknown);
        assert_eq!(report.summary.exit_code, 1);
    }

    #[test]
    fn test_skeleton_uses_nulls_not_false() {
        let report = DoctorReport::skeleton(Path::new("."));

        // Check that unknown scalars are None, not Some(false)
        assert!(report.hook.grant.is_none());
        assert!(report.hook.enforce_absent.is_none());
        assert!(report.binaries.hook_harness.executable.is_none());
        assert!(report.binaries.claude_candidate.version.is_none());
    }

    #[test]
    fn test_skeleton_never_fakes_success() {
        let report = DoctorReport::skeleton(Path::new("."));

        assert_ne!(report.summary.installation, InstallationStatus::Ok);
        assert_eq!(report.summary.installation, InstallationStatus::Unknown);
        assert_ne!(reduce_exit_code(&report), 0);
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
    fn test_json_serialization_roundtrip() {
        let report = DoctorReport::skeleton(Path::new("."));
        let json = serde_json::to_string(&report).expect("serialize");
        let parsed: DoctorReport = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(parsed.schema_version, report.schema_version);
        assert_eq!(parsed.profile, report.profile);
        assert_eq!(parsed.summary.installation, report.summary.installation);
    }

    #[test]
    fn test_reduction_combinations_with_broken_and_disabled() {
        let mut report = DoctorReport::skeleton(Path::new("."));

        // Broken takes priority over disabled
        report.summary.installation = InstallationStatus::Broken;
        assert_eq!(reduce_exit_code(&report), 2);

        // Disabled is exit 1
        report.summary.installation = InstallationStatus::Disabled;
        assert_eq!(reduce_exit_code(&report), 1);
    }

    #[test]
    fn test_findings_sorted_deterministically() {
        let report = DoctorReport::skeleton(Path::new("."));

        // Skeleton has at least one finding with stable code
        assert!(!report.findings.is_empty());
        let first = &report.findings[0];
        assert_eq!(first.code, "SKELETON_INCOMPLETE");
    }
}
