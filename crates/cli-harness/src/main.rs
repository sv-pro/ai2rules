use agent_core::{
    default_executor, run, ApprovalPolicy, ModelClient, ModelTurn, SessionConfig, TranscriptEntry,
    TurnContext,
};
use clap::Parser;
use compiler::{compile, compile_default, loader::load_yaml};
use harness_preview::{gate, GateRequest};
use harness_types::{
    ApprovalTokenId, CompiledWorld, Decision, EffectMode, ExecutionMode, Provenance, ToolCall,
};
use provider_adapters::anthropic::tool_use_block;
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};
use trace_store::{ApprovalStore, TraceStore};
use world_kernel::ExecEnv;

mod agy_hook;
mod broken_world;
mod cc_hook;
mod console;
mod demo;
mod doctor;
mod doctor_collector;
mod hostkit;
mod init;
mod init_machine;
mod local_page;
mod mcp_approval;
mod mcp_author;
mod mcp_gateway;
mod mock_jira;
mod otlp;
mod outcome;
mod project;
mod replay_transcripts;
mod serve;
mod story;

/// ai2rules
#[derive(Parser, Debug)]
#[command(author, version, about = "ai2rules", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Run in simulate mode (no side effects)
    #[arg(long)]
    simulate: bool,

    /// Run in background mode (collapses ASK to DENY)
    #[arg(long)]
    background: bool,

    /// Path to world manifest (YAML)
    #[arg(long)]
    world: Option<PathBuf>,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Govern the project in this directory: write a starter manifest, the
    /// PreToolUse shim, and the host settings entry — with nothing but this
    /// binary. No ai2rules checkout, no `cargo`, no `jq`.
    ///
    /// The starter manifest is compiled into this executable and is compiled
    /// (by the real compiler) before anything is written. The shim bakes the
    /// absolute path of *this* binary, so there is no separate install step.
    /// Safe to re-run: a tuned manifest is kept unless `--force`, and the
    /// settings hook is merged idempotently.
    Init {
        /// Project directory to govern.
        #[arg(default_value = ".")]
        target: PathBuf,
        /// Replace mode: the manifest becomes the authoritative allowlist and
        /// ALLOW verdicts *grant* (the host skips its own prompt). Default is
        /// additive — deny/ask overlay only, which can never lock you out.
        #[arg(long)]
        grant: bool,
        /// Overwrite an existing `.claude/cc-world.yaml`. Off by default: a
        /// tuned manifest is the valuable artifact here, not the template.
        #[arg(long)]
        force: bool,
        /// Print the plan and write nothing.
        #[arg(long)]
        dry_run: bool,
        /// Govern every Claude Code session this account opens, in any folder:
        /// the hook goes into `~/.claude/settings.json` (or `$CLAUDE_CONFIG_DIR`),
        /// the world and an absolute state dir into `~/.claude/ai2rules/user/`.
        /// A project can still switch user hooks off (`disableAllHooks`); for a
        /// hook only an administrator can remove, use `--managed`.
        #[arg(long, conflicts_with = "managed")]
        user: bool,
        /// Govern every session on this machine, for every account, through
        /// Claude Code's managed settings (the system policy directory). Needs an
        /// elevated shell. The shim has no user-side kill-switch, and the kernel
        /// is copied into the admin-owned directory.
        #[arg(long)]
        managed: bool,
        /// With `--managed`: write into this directory instead of the OS policy
        /// directory (for staging a bundle, or testing).
        #[arg(long, value_name = "DIR", requires = "managed")]
        policy_dir: Option<PathBuf>,
        /// With `--user` or `--managed`: unregister the hook and delete the shim,
        /// keeping every other setting. The manifest and state are kept.
        #[arg(long)]
        remove: bool,
    },
    /// Run the safe first-proof scenarios over the real compiled project world.
    Demo {
        /// Tell a story instead, over its own embedded world: no `harness init`
        /// needed. `law-firm`: an AI legal assistant reads a poisoned client email.
        #[arg(long, value_enum)]
        story: Option<story::Story>,
        /// With `--story`: also write the run as a self-contained page to step
        /// through in a browser, for presenting. The page shows this run's verdicts.
        #[arg(long, value_name = "PATH", requires = "story")]
        html: Option<PathBuf>,
    },
    /// Inspect effective coverage: report on installation, hook, manifest, and
    /// projected actions in the current project (AI2-25 / #84 skeleton).
    Doctor {
        /// Emit JSON report on stdout instead of human-readable text.
        #[arg(long)]
        json: bool,
    },
    /// Replay the tool calls in past Claude Code transcripts through the kernel,
    /// exactly as `harness cc-hook` would have decided them, and report how often
    /// it would have asked or denied, and why. Nothing is executed. Uses the
    /// starter manifest unless `--world` is given.
    ReplayTranscripts {
        /// Transcript files or folders (searched for `.jsonl`). Default:
        /// `~/.claude/projects`, every session on this machine.
        paths: Vec<PathBuf>,
        /// Execution mode, as for `cc-hook`.
        #[arg(long, default_value = "interactive", value_parser = ["interactive", "background"])]
        mode: String,
        /// The manifest channel calls are attributed to, as for `cc-hook`.
        #[arg(long, default_value = "user_prompt")]
        source_channel: String,
        /// Count ABSENT as a denial, as `cc-hook --enforce-absent` would.
        #[arg(long)]
        enforce_absent: bool,
        /// Replace mode, as `cc-hook --grant`: ALLOW becomes an explicit grant.
        #[arg(long)]
        grant: bool,
        /// Examples shown per rule in the report.
        #[arg(long, default_value_t = 5)]
        examples: usize,
        /// Every call with its verdict and reason, as JSON.
        #[arg(long)]
        json: bool,
        /// The hook's decision log (`cc-hook --log`): join each replayed call to
        /// the live verdict and its outcome by `tool_use_id` (D84).
        #[arg(long)]
        decisions: Option<PathBuf>,
    },
    /// Convert a finished host decision log (`cc-hook --log`) to OpenTelemetry
    /// traces as OTLP/JSON, for a collector's `otlpjsonfile` receiver (AI2-66).
    /// One trace per session, one GenAI `execute_tool` span per decision, from
    /// the verdict to the call's outcome. Offline: nothing is sent anywhere.
    ExportOtlp {
        /// The decision log to convert.
        #[arg(long)]
        log: PathBuf,
        /// Write here instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Also export the call and result summaries (already redacted, D66).
        /// Off by default: the GenAI conventions make tool arguments and
        /// results opt-in, as they may be sensitive.
        #[arg(long)]
        with_summaries: bool,
    },
    /// Launch the World Authoring Tool: a local browser editor for world
    /// manifests, backed by the real compiler + kernel (E11).
    Serve {
        /// Port to bind on 127.0.0.1
        #[arg(long, default_value_t = 8787)]
        port: u16,
    },
    /// Govern one proposed tool call: read a `GateRequest` JSON on stdin and
    /// write the `GateResponse` verdict on stdout — the host-neutral gate ABI
    /// (D24). A host adapter wraps this. See `docs/harness-gate-abi.md`.
    Gate {
        /// Path to the world manifest (YAML/JSON) to govern against.
        #[arg(long)]
        world: PathBuf,
    },
    /// Shape a host's offered tool schemas through a compiled world: read a
    /// projection request on stdin and write the visible schemas plus manifest
    /// and schema identities on stdout (D72).
    Project {
        /// Path to the world manifest (YAML/JSON) whose projected actions govern discovery.
        #[arg(long)]
        world: PathBuf,
    },
    /// Run a self-contained mock JIRA MCP server on stdio — the demo upstream the
    /// gateway governs (DECISIONS D33 / E16.A).
    MockJira {
        /// Advertise the REAL Atlassian Rovo tool names (`getJiraIssue`, …) plus a
        /// Confluence tool, instead of the invented `jira_*` mock names — so the
        /// real-Atlassian manifest (`jira-atlassian.world.yaml`) can be exercised
        /// offline (E16.E). See `docs/demos/jira-copilot/REAL-ATLASSIAN.md`.
        #[arg(long)]
        rovo: bool,
        /// Behave like a **malicious or drifted** upstream (finding #14): advertise
        /// an allowed tool name with an extra, undeclared `deleteAll` argument in
        /// its `inputSchema`, and echo back whatever arguments actually arrive.
        /// The governed surface must never republish that schema, and must never
        /// forward that argument. Used by `tests/mcp_gateway_poisoned.rs`.
        #[arg(long)]
        poisoned: bool,
        /// Answer every `tools/call` with an MRTR `InputRequiredResult` (MCP
        /// `2026-07-28`) demanding a credential, instead of a result. The governed
        /// gateway must refuse to relay that demand — see D49 and issue #40.
        #[arg(long)]
        input_required: bool,
        /// Answer every `tools/call` with the tool name and arguments that actually
        /// arrived, so a test can assert what the gateway forwarded — a scoped
        /// tool's upstream name, renamed arguments and injected literals (D79).
        /// Used by `tests/mcp_gateway_scoped.rs`.
        #[arg(long)]
        echo: bool,
    },
    /// Claude Code PreToolUse adapter, in Rust (D33 / E16.C): read a PreToolUse
    /// event on stdin, govern it with the kernel in-process, and emit a deny/ask
    /// decision — or, in `--grant`/replace mode, an explicit `allow` that grants
    /// (skips the host's Allow/Deny prompt). Additive by default (never
    /// auto-allows); fail-open on an unreadable event, and a world that does not
    /// compile follows `--on-broken-world` (D87). Replaces the Python `world-gate-adapter.py` —
    /// this is the "deep" half governing native tools.
    CcHook {
        /// Path to the world manifest (YAML/JSON) that governs this session.
        #[arg(long)]
        world: PathBuf,
        /// Directory for the per-session taint sidecar.
        #[arg(long, default_value = ".claude/state")]
        state: PathBuf,
        /// Execution mode threaded into every gate call. The kernel itself
        /// collapses ASK→DENY in background (invariant 10).
        #[arg(long, default_value = "interactive", value_parser = ["interactive", "background"])]
        mode: String,
        /// Enforce ABSENT: deny tools the world does not declare (reason
        /// prefixed "ABSENT: "). Default is passthrough — additive dogfooding,
        /// since a PreToolUse hook cannot remove native tools and denying
        /// everything outside the manifest would brick the host.
        #[arg(long)]
        enforce_absent: bool,
        /// The manifest channel this session's tool calls are attributed to —
        /// the proposing actor's trust, resolved against the world's `channels:`
        /// table.
        ///
        /// **Declared, not derived (finding #22).** A PreToolUse event says what
        /// tool is about to run, never who asked for it: Claude Code sends
        /// `tool_name`/`tool_input`/`permission_mode`, Antigravity sends
        /// `toolCall`/`modelName`/`stepIdx`. Neither identifies the proposer, so an
        /// adapter that claimed to know would be guessing. What it can do is let
        /// you *state* the posture: run a supervised session as `user_prompt`, and
        /// an unattended one as a lower-trust channel so the capability matrix
        /// shrinks accordingly. Data-flow taint is enforced either way and does not
        /// depend on this.
        #[arg(long, default_value = "user_prompt")]
        source_channel: String,
        /// Replace mode: emit an explicit `allow` on ALLOW verdicts, which
        /// *grants* — Claude Code skips its Allow/Deny prompt — so the manifest
        /// is the authoritative allowlist, not an additive overlay. Pair with an
        /// emptied `settings.json` deny/ask baseline (native deny/ask still fire
        /// even on a hook `allow`). Default off: ALLOW stays a silent passthrough.
        #[arg(long)]
        grant: bool,
        /// Append one JSON line per decision to this file: what the agent tried,
        /// the verdict, the rule, the session's taint after the call, and the
        /// host's `tool_use_id` / `agent_id` / `agent_type` / `cwd`. It is what
        /// `harness console` shows as its live feed. Wired to `PostToolUse` and
        /// `PostToolUseFailure` as well, the same command appends an outcome line
        /// per executed call, keyed by `tool_use_id` (D84). Observability only: a
        /// log that cannot be written never changes the verdict. Off by default.
        #[arg(long)]
        log: Option<PathBuf>,
        /// What to do when the world cannot be read or compiled (D87): `closed`
        /// refuses every call, `open` lets it through ungoverned, `last-good`
        /// decides with the last world that compiled here (cached in `--state`)
        /// and falls back to `closed` when none ever did. Every mode warns the
        /// person. Lives outside the world because a broken world cannot say how
        /// it wants to be handled.
        #[arg(long, value_enum, default_value_t = broken_world::BrokenWorldMode::LastGood)]
        on_broken_world: broken_world::BrokenWorldMode,
    },
    /// Antigravity CLI (`agy`) PreToolUse adapter, in Rust (D48): read an
    /// Antigravity `PreToolUse` payload on stdin, govern it with the kernel
    /// in-process, and emit a deny/force_ask decision — or, in `--grant`/replace
    /// mode, an explicit `allow` that grants (skips the host's prompt). Additive
    /// by default (never auto-allows); fail-open on an unreadable event, and a
    /// world that does not compile follows `--on-broken-world` (D87). The agy
    /// sibling of `cc-hook`.
    AgyHook {
        /// Path to the world manifest (YAML/JSON) that governs this session.
        #[arg(long)]
        world: PathBuf,
        /// Directory for the per-conversation taint sidecar.
        #[arg(long, default_value = ".agents/state")]
        state: PathBuf,
        /// Execution mode threaded into every gate call. The kernel itself
        /// collapses ASK->DENY in background (invariant 10).
        #[arg(long, default_value = "interactive", value_parser = ["interactive", "background"])]
        mode: String,
        /// Enforce ABSENT: deny tools the world does not declare (reason
        /// prefixed "ABSENT: "). Default is passthrough — additive dogfooding,
        /// since a PreToolUse hook cannot remove native tools and denying
        /// everything outside the manifest would brick the host.
        #[arg(long)]
        enforce_absent: bool,
        /// The manifest channel this session's tool calls are attributed to —
        /// the proposing actor's trust, resolved against the world's `channels:`
        /// table.
        ///
        /// **Declared, not derived (finding #22).** An Antigravity PreToolUse
        /// payload carries `toolCall`, `modelName` and `stepIdx` — what is about to
        /// run, never who asked for it — so an adapter claiming to know the
        /// proposer would be guessing. What it can do is let you *state* the
        /// posture: a supervised session as `user_prompt`, an unattended one as a
        /// lower-trust channel so the capability matrix shrinks accordingly.
        /// Data-flow taint is enforced either way and does not depend on this.
        #[arg(long, default_value = "user_prompt")]
        source_channel: String,
        /// Replace mode: emit an explicit `allow` on ALLOW verdicts, which
        /// *grants* — Antigravity skips its permission prompt — so the manifest
        /// is the authoritative allowlist, not an additive overlay. Default off:
        /// ALLOW stays a silent passthrough.
        #[arg(long)]
        grant: bool,
        /// Map kernel ASK verdicts onto Antigravity's cache-respecting `ask`
        /// instead of `force_ask`. Default is `force_ask`, so a cached
        /// "Always Allow" grant can never silently satisfy an approval the
        /// kernel required.
        #[arg(long)]
        soft_ask: bool,
        /// Append one JSON line per decision to this file, in `cc-hook --log`'s
        /// format (D84). Antigravity sends no tool-call id, so its lines carry
        /// `step_idx` instead. Observability only: a log that cannot be written
        /// never changes the verdict. Off by default.
        #[arg(long)]
        log: Option<PathBuf>,
        /// What to do when the world cannot be read or compiled (D87): `closed`
        /// refuses every call, `open` lets it through ungoverned, `last-good`
        /// decides with the last world that compiled here (cached in `--state`)
        /// and falls back to `closed` when none ever did. Every mode warns the
        /// person. Lives outside the world because a broken world cannot say how
        /// it wants to be handled.
        #[arg(long, value_enum, default_value_t = broken_world::BrokenWorldMode::LastGood)]
        on_broken_world: broken_world::BrokenWorldMode,
    },
    /// Front a real upstream MCP server with the kernel: shape its `tools/list`
    /// (ABSENT) and gate every `tools/call`, forwarding only ALLOW (D33 / E16.B).
    McpGateway {
        /// Path to the world manifest (YAML/JSON) that shapes the surface.
        #[arg(long)]
        world: PathBuf,
        /// Provenance source channel = the proposer's trust (cli|workspace_file|web|…).
        #[arg(long, default_value = "cli")]
        source: String,
        /// Initial carried session taint floor: `clean` (default) | `tainted`.
        #[arg(long, default_value = "clean", value_parser = ["clean", "tainted"])]
        taint: String,
        /// Execution mode threaded into every gate call. The kernel itself
        /// collapses ASK→DENY in background (invariant 10).
        #[arg(long, default_value = "interactive", value_parser = ["interactive", "background"])]
        mode: String,
        /// Optional append-only JSONL audit log path.
        #[arg(long)]
        audit: Option<PathBuf>,
        /// Optional `ApprovalStore` (JSONL) enabling out-of-band approval of `ASK`
        /// actions (D89): the gateway mints a pending token, a human runs
        /// `harness approvals approve <id>`, and a matching retry may execute once after kernel reevaluation.
        /// Without it, `ASK` stays a non-forwarded block. Point `harness approvals`
        /// at the same path.
        #[arg(long, requires = "principal")]
        approvals: Option<PathBuf>,
        /// Operator-configured identity for this gateway process (never request metadata).
        #[arg(long, requires = "approvals")]
        principal: Option<String>,
        /// Upstream MCP server command (pass after `--`), e.g. `-- harness mock-jira`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        upstream: Vec<String>,
    },
    /// Review and act on out-of-band approvals recorded by `mcp-gateway
    /// --approvals` (D89). Operates on the same JSONL `ApprovalStore`.
    Approvals {
        #[command(subcommand)]
        action: ApprovalsAction,
    },
    /// The policy console (D81 / E15.7): a local page whose profiles and
    /// Allow / Ask / Off switches rewrite a project's world file, which
    /// `harness cc-hook` reads on every call, so a live Claude Code session
    /// changes behaviour on its next tool call. The hook's `--log` is the page's
    /// live feed. The kernel still decides every call.
    Console {
        /// The project governed by `harness cc-hook` (it has `.claude/cc-world.yaml`).
        #[arg(long, default_value = ".")]
        project: PathBuf,
        /// The world file to edit. Default: `<project>/.claude/cc-world.yaml`.
        #[arg(long)]
        world: Option<PathBuf>,
        /// The hook's decision log to show. Default:
        /// `<project>/.claude/state/decisions.jsonl`.
        #[arg(long)]
        log: Option<PathBuf>,
        /// Local port for the page (127.0.0.1 only). 0 picks a free one.
        #[arg(long, default_value_t = 8790)]
        port: u16,
    },
    /// Author a scoped MCP surface in the browser (D79 / E13.4c): connect to an
    /// upstream MCP server, pick and rename its tools, fix or limit their
    /// arguments, try calls through the real kernel, and save the world manifest
    /// that `harness mcp-gateway --world` serves.
    McpAuthor {
        /// The world manifest to edit and save. Loaded if it exists.
        #[arg(long)]
        world: PathBuf,
        /// Local port for the page (127.0.0.1 only). 0 picks a free one.
        #[arg(long, default_value_t = 8788)]
        port: u16,
        /// Upstream MCP server command (pass after `--`), e.g. `-- harness mock-jira --rovo`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
        upstream: Vec<String>,
    },
}

#[derive(clap::Subcommand, Debug)]
enum ApprovalsAction {
    /// List approvals, including consumed grants with no confirmed outcome.
    List {
        /// Path to the `ApprovalStore` JSONL (same as the gateway's `--approvals`).
        #[arg(long, default_value = ".ai2rules/approvals.jsonl")]
        store: PathBuf,
    },
    /// Approve a pending request by id; a retry must still pass the kernel.
    Approve {
        /// The approval token id (see `harness approvals list`).
        id: String,
        #[arg(long, default_value = ".ai2rules/approvals.jsonl")]
        store: PathBuf,
    },
    /// Reject a pending request by id.
    Reject {
        /// The approval token id (see `harness approvals list`).
        id: String,
        #[arg(long, default_value = ".ai2rules/approvals.jsonl")]
        store: PathBuf,
    },
}

struct InteractiveModel;

impl ModelClient for InteractiveModel {
    fn next(&mut self, ctx: &TurnContext) -> ModelTurn {
        let mut available_actions: Vec<String> = vec![];
        if let Some(tools_arr) = ctx.tools.as_array() {
            for tool in tools_arr {
                if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
                    available_actions.push(name.to_string());
                }
            }
        }
        available_actions.sort();
        available_actions.insert(0, "quit".to_string());

        println!("\n[Model] Select an action to propose:");
        let action = match inquire::Select::new("Action:", available_actions).prompt() {
            Ok(s) if s == "quit" => return ModelTurn::Final("User quit".into()),
            Ok(s) => s,
            Err(_) => return ModelTurn::Final("Interrupted".into()),
        };

        // Extract and print the schema for the chosen action to guide the user
        if let Some(tools_arr) = ctx.tools.as_array() {
            if let Some(tool) = tools_arr
                .iter()
                .find(|t| t.get("name").and_then(|n| n.as_str()) == Some(&action))
            {
                if let Some(schema) = tool.get("input_schema") {
                    println!(
                        "  Expected schema: {}",
                        serde_json::to_string(schema).unwrap_or_default()
                    );
                }
            }
        }

        let args_str = match inquire::Text::new("Arguments (JSON):")
            .with_default("{}")
            .prompt()
        {
            Ok(s) => s,
            Err(_) => return ModelTurn::Final("Interrupted".into()),
        };

        let args: Value = serde_json::from_str(&args_str).unwrap_or_else(|_| {
            println!("Invalid JSON. Defaulting to empty object {{}}.");
            serde_json::json!({})
        });
        ModelTurn::ToolUse(tool_use_block("req-1", &action, args))
    }
}

fn ask_approval(call: &ToolCall, _world: &CompiledWorld, _provenance: &Provenance) -> bool {
    println!("\n[APPROVAL REQUIRED]");
    println!("Action: {}", call.action_name);
    println!(
        "Arguments: {}",
        serde_json::to_string_pretty(&call.arguments).unwrap_or_default()
    );

    inquire::Confirm::new("Approve this action?")
        .with_default(false)
        .prompt()
        .unwrap_or(false)
}

fn main() {
    let cli = Cli::parse();

    if let Some(Command::Init {
        target,
        grant,
        force,
        dry_run,
        user,
        managed,
        policy_dir,
        remove,
    }) = &cli.command
    {
        let scope = match (*user, *managed) {
            (true, _) => Some(init_machine::Scope::User),
            (_, true) => Some(init_machine::Scope::Managed),
            _ => None,
        };
        std::process::exit(match scope {
            Some(scope) => init_machine::run(&init_machine::Options {
                scope,
                policy_dir: policy_dir.as_deref(),
                grant: *grant,
                force: *force,
                dry_run: *dry_run,
                remove: *remove,
            }),
            None if *remove => {
                eprintln!("harness init: --remove needs --user or --managed");
                2
            }
            None => init::run(target, *grant, *force, *dry_run),
        });
    }

    if let Some(Command::Demo { story, html }) = &cli.command {
        std::process::exit(match story {
            Some(story) => story::run(*story, html.as_deref()),
            None => demo::run(cli.world.as_deref()),
        });
    }

    if let Some(Command::ReplayTranscripts {
        paths,
        mode,
        source_channel,
        enforce_absent,
        grant,
        examples,
        json,
        decisions,
    }) = &cli.command
    {
        std::process::exit(replay_transcripts::run(&replay_transcripts::Options {
            paths,
            world: cli.world.as_deref(),
            mode,
            source_channel,
            enforce_absent: *enforce_absent,
            grant: *grant,
            examples: *examples,
            json: *json,
            decisions: decisions.as_deref(),
        }));
    }

    if let Some(Command::ExportOtlp {
        log,
        out,
        with_summaries,
    }) = &cli.command
    {
        std::process::exit(otlp::run(&otlp::Options {
            log,
            out: out.as_deref(),
            with_summaries: *with_summaries,
        }));
    }

    if let Some(Command::Doctor { json }) = &cli.command {
        std::process::exit(doctor::run(*json));
    }

    if let Some(Command::Serve { port }) = cli.command {
        if let Err(e) = serve::run(port) {
            eprintln!("authoring server error: {e}");
            std::process::exit(1);
        }
        return;
    }

    if let Some(Command::Gate { world }) = &cli.command {
        std::process::exit(run_gate(world));
    }

    if let Some(Command::Project { world }) = &cli.command {
        std::process::exit(project::run(world));
    }

    if let Some(Command::MockJira {
        rovo,
        poisoned,
        input_required,
        echo,
    }) = &cli.command
    {
        std::process::exit(mock_jira::run(*rovo, *poisoned, *input_required, *echo));
    }

    if let Some(Command::CcHook {
        world,
        state,
        mode,
        source_channel,
        enforce_absent,
        grant,
        log,
        on_broken_world,
    }) = &cli.command
    {
        std::process::exit(cc_hook::run(
            world,
            state,
            mode,
            source_channel,
            *enforce_absent,
            *grant,
            log.as_deref(),
            *on_broken_world,
        ));
    }

    if let Some(Command::AgyHook {
        world,
        state,
        mode,
        source_channel,
        enforce_absent,
        grant,
        soft_ask,
        log,
        on_broken_world,
    }) = &cli.command
    {
        std::process::exit(agy_hook::run(
            world,
            state,
            mode,
            source_channel,
            *enforce_absent,
            *grant,
            *soft_ask,
            log.as_deref(),
            *on_broken_world,
        ));
    }

    if let Some(Command::Console {
        project,
        world,
        log,
        port,
    }) = &cli.command
    {
        std::process::exit(console::run(
            project,
            world.as_deref(),
            log.as_deref(),
            *port,
        ));
    }

    if let Some(Command::McpAuthor {
        world,
        port,
        upstream,
    }) = &cli.command
    {
        std::process::exit(mcp_author::run(world, *port, upstream));
    }

    if let Some(Command::McpGateway {
        world,
        source,
        taint,
        mode,
        audit,
        approvals,
        principal,
        upstream,
    }) = &cli.command
    {
        let tainted = taint == "tainted";
        std::process::exit(mcp_gateway::run(
            world,
            upstream,
            source,
            tainted,
            mode,
            audit.as_deref(),
            approvals.as_deref().zip(principal.as_deref()),
        ));
    }

    if let Some(Command::Approvals { action }) = &cli.command {
        std::process::exit(run_approvals(action));
    }

    let world = if let Some(path) = cli.world {
        let content = std::fs::read_to_string(path).expect("failed to read world file");
        let manifest = load_yaml(&content).expect("failed to parse manifest YAML");
        compile(&manifest).expect("failed to compile world")
    } else {
        compile_default()
    };

    let executor = default_executor(&world);
    // Session storage for this CLI runner. A tempdir, which also satisfies the
    // constraint that matters: the approval store must NOT live inside the project
    // being governed (finding #15). An earlier note here proposed `.agents/` for a
    // deployed tool — that is precisely wrong, and it is the same mistake D58 found
    // in the npm layout and D57 in the control plane: anything the enforcement
    // depends on must live outside what it enforces upon. A grant record the agent
    // can write is not a grant record. The log is MAC'd so tampering is detectable
    // wherever it sits, but detection is the backstop, not the plan.
    let sandbox = tempfile::tempdir().expect("sandbox");
    let trace = TraceStore::open(sandbox.path().join("trace.jsonl"));
    let mut store = ApprovalStore::open(sandbox.path().join("approvals.jsonl")).expect("store");

    let mut model = InteractiveModel;

    let config = SessionConfig {
        effect_mode: if cli.simulate {
            EffectMode::Simulate
        } else {
            EffectMode::Execute
        },
        mode: if cli.background {
            ExecutionMode::Background
        } else {
            ExecutionMode::Interactive
        },
        approval: ApprovalPolicy::Interactive(ask_approval),
        max_steps: 100, // Let the interactive model handle stopping
        ..SessionConfig::default()
    };

    println!("ai2rules initialized.");
    println!("World: {}", world.world_id().as_str());
    println!("Mode: {:?} | Effect: {:?}", config.mode, config.effect_mode);

    let outcome = run(
        &world,
        &ExecEnv::default(),
        &executor,
        &trace,
        &mut store,
        &mut model,
        &config,
        Some(&mut |entry| {
            render_entry(entry);
        }),
    );

    println!("\nSession ended. Final text: {:?}", outcome.final_text);
    println!("{} records appended to trace.", outcome.records);
}

/// The `harness gate` subcommand: compile `world_path`, read one `GateRequest`
/// JSON on stdin, run the pure kernel gate, and write the `GateResponse` on
/// stdout. Returns the process exit code (`docs/harness-gate-abi.md` §5):
/// `0` = evaluated — the verdict (incl. `DENY`/`ASK`) is on stdout; `2` = a
/// malformed request or an unreadable/uncompilable manifest; `1` = internal
/// error. The verdict is never encoded in the exit code (D24): mapping it to a
/// host's decision shape is the adapter's job.
fn run_approvals(action: &ApprovalsAction) -> i32 {
    let open = |store: &PathBuf| ApprovalStore::open(store.clone());
    match action {
        ApprovalsAction::List { store } => {
            let s = match open(store) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("approvals: cannot open store {}: {e}", store.display());
                    return 2;
                }
            };
            let mut any = false;
            for t in s.all() {
                any = true;
                let ph = t.params_hash.as_str();
                println!(
                    "{:<18}  {:<9}  {:<26}  params={} principal={} resource={} expires_ms={}",
                    t.id.as_str(),
                    format!("{:?}", t.state),
                    t.action.as_str(),
                    ph,
                    t.principal.as_str(),
                    t.resource,
                    t.valid_until_unix_ms,
                );
            }
            if !any {
                println!("(no approvals recorded in {})", store.display());
            }
            0
        }
        ApprovalsAction::Approve { id, store } => {
            let mut s = match open(store) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("approvals: cannot open store {}: {e}", store.display());
                    return 2;
                }
            };
            match s.approve(&ApprovalTokenId::new(id.clone())) {
                Ok(()) => {
                    println!(
                        "approved {id} — one exact-call claim; expiry and kernel policy still apply"
                    );
                    0
                }
                Err(e) => {
                    eprintln!("approvals: cannot approve {id}: {e}");
                    1
                }
            }
        }
        ApprovalsAction::Reject { id, store } => {
            let mut s = match open(store) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("approvals: cannot open store {}: {e}", store.display());
                    return 2;
                }
            };
            match s.reject(&ApprovalTokenId::new(id.clone())) {
                Ok(()) => {
                    println!("rejected {id}");
                    0
                }
                Err(e) => {
                    eprintln!("approvals: cannot reject {id}: {e}");
                    1
                }
            }
        }
    }
}

fn run_gate(world_path: &Path) -> i32 {
    let world = match hostkit::load_compiled_world(world_path) {
        Ok(world) => world,
        Err(e) => {
            eprintln!("gate: {e}");
            return 2;
        }
    };

    let mut buf = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut buf) {
        eprintln!("gate: cannot read stdin: {e}");
        return 1;
    }
    let req: GateRequest = match serde_json::from_str(&buf) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("gate: malformed GateRequest: {e}");
            return 2;
        }
    };

    let response = gate(&world, &req);
    match serde_json::to_string(&response) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(e) => {
            eprintln!("gate: cannot serialize response: {e}");
            1
        }
    }
}

fn render_entry(entry: &TranscriptEntry) {
    println!("\n▶ Action: {}", entry.action);
    if let Some(decision) = entry.decision {
        println!("  Decision: {}", decision_label(decision));
        if let Some(rule) = entry.rule.as_deref() {
            println!("  Rule:     {rule}");
        }
        if let Some(effect) = entry.effect_mode {
            println!("  Effect:   {effect:?}");
        }
        if entry.verdict != decision_name(decision) {
            println!("  Flow:     {}", entry.verdict);
        }
        println!("  Feedback: {}", feedback(decision, entry.rule.as_deref()));
    } else {
        println!("  Verdict:  {}", entry.verdict);
    }
    println!("  Result:   {} [taint: {:?}]", entry.result, entry.taint);
}

fn decision_name(decision: Decision) -> &'static str {
    match decision {
        Decision::Absent => "ABSENT",
        Decision::Allow => "ALLOW",
        Decision::Deny => "DENY",
        Decision::Ask => "ASK",
        Decision::Replan => "REPLAN",
    }
}

fn decision_label(decision: Decision) -> String {
    let code = match decision {
        Decision::Allow => "32;1",
        Decision::Deny => "31;1",
        Decision::Ask => "33;1",
        Decision::Absent => "35;1",
        Decision::Replan => "36;1",
    };
    format!("\x1b[{code}m{}\x1b[0m", decision_name(decision))
}

fn feedback(decision: Decision, rule: Option<&str>) -> &'static str {
    match (decision, rule.unwrap_or("")) {
        (Decision::Absent, "unknown_to_ontology") => {
            "action is not in the ontology; no intent or executor path exists"
        }
        (Decision::Absent, "absent") => "action exists but is not projected into this world",
        (Decision::Absent, "capability") => {
            "current trust/capability context cannot see this action"
        }
        (Decision::Absent, _) => "action is unavailable in this world/context",
        (Decision::Deny, "background_denies_ask") => {
            "approval-required action failed closed in background mode"
        }
        (Decision::Deny, "taint_invariant") => "tainted input cannot cross this effect boundary",
        (Decision::Deny, _) => "policy blocked a visible action",
        (Decision::Ask, "approval_required") => "human approval is required before execution",
        (Decision::Ask, "approval_rejected") => "approval was rejected; action did not execute",
        (Decision::Ask, _) => "approval flow paused execution",
        (Decision::Replan, _) => "budget or scope exceeded; propose a smaller step",
        (Decision::Allow, _) => "execution spec crossed the boundary",
    }
}
