//! `govbench` — run the MCP governance smoke pack.
//!
//! ```text
//! govbench [--pack DIR] [--out DIR] [--target all|weak|ai2rules]
//!          [--transport both|linked|wire] [--harness PATH] [--assert-contrast]
//! ```
//!
//! One command, offline, no LLM. Writes `results.json` and `REPORT.md` to the
//! output directory and prints the per-scenario matrix.

use std::cell::RefCell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;

use govbench::report;
use govbench::result::{
    BenchResult, PackIdentity, RunResult, Status, TargetIdentity, TransportParity, RESULT_VERSION,
};
use govbench::run::run_scenario;
use govbench::target::Target;
use govbench::targets::{Ai2rules, Transport, WeakGateway};
use govbench::upstream::Upstream;
use govbench::Pack;

const USAGE: &str = "\
govbench — the Public MCP Governance Benchmark smoke pack

  --pack DIR          scenario pack (default docs/benchmarks/mcp-governance/pack)
  --out DIR           where to write results.json and REPORT.md
                      (default docs/benchmarks/mcp-governance/results)
  --target WHICH      all | weak | ai2rules            (default all)
  --transport WHICH   both | linked | wire             (default both)
  --harness PATH      the `harness` binary for the wire transport
                      (default $AI2RULES_HARNESS, then target/{debug,release}/harness)
  --assert-contrast   exit non-zero unless ai2rules passes every scenario and the
                      weak reference gateway fails every scenario
  --quiet             do not print the matrix
";

fn main() -> ExitCode {
    match real_main() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("govbench: {error}");
            ExitCode::from(1)
        }
    }
}

struct Args {
    pack: PathBuf,
    out: PathBuf,
    target: String,
    transport: String,
    harness: Option<PathBuf>,
    assert_contrast: bool,
    quiet: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        pack: PathBuf::from("docs/benchmarks/mcp-governance/pack"),
        out: PathBuf::from("docs/benchmarks/mcp-governance/results"),
        target: "all".to_string(),
        transport: "both".to_string(),
        harness: std::env::var_os("AI2RULES_HARNESS").map(PathBuf::from),
        assert_contrast: false,
        quiet: false,
    };
    let mut argv = std::env::args().skip(1);
    while let Some(arg) = argv.next() {
        let mut value = || {
            argv.next()
                .ok_or_else(|| format!("{arg} needs a value\n\n{USAGE}"))
        };
        match arg.as_str() {
            "--pack" => args.pack = PathBuf::from(value()?),
            "--out" => args.out = PathBuf::from(value()?),
            "--target" => args.target = value()?,
            "--transport" => args.transport = value()?,
            "--harness" => args.harness = Some(PathBuf::from(value()?)),
            "--assert-contrast" => args.assert_contrast = true,
            "--quiet" => args.quiet = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other}\n\n{USAGE}")),
        }
    }
    if !matches!(args.target.as_str(), "all" | "weak" | "ai2rules") {
        return Err(format!("--target must be all|weak|ai2rules\n\n{USAGE}"));
    }
    if !matches!(args.transport.as_str(), "both" | "linked" | "wire") {
        return Err(format!("--transport must be both|linked|wire\n\n{USAGE}"));
    }
    Ok(args)
}

fn real_main() -> Result<ExitCode, String> {
    let args = parse_args()?;
    let pack = Pack::load(&args.pack)?;

    let wire_binary = if args.transport == "linked" {
        None
    } else {
        Some(locate_harness(args.harness.clone())?)
    };

    let mut runs: Vec<RunResult> = Vec::new();
    let mut targets: Vec<TargetIdentity> = Vec::new();
    let mut parity_detail: Vec<String> = Vec::new();
    let mut parity_ok = true;

    for scenario in &pack.scenarios {
        if args.target != "ai2rules" {
            let upstream = Rc::new(RefCell::new(Upstream::new(pack.tools.clone())));
            let mut target = WeakGateway::new(upstream.clone(), pack.weak_policy.clone());
            record(&mut targets, &target);
            runs.push(run_scenario(scenario, &mut target, &upstream));
        }
        if args.target != "weak" {
            // The recorded ai2rules run uses the wire transport whenever it is
            // available: the shipped `harness` binary is the product, and the
            // linked run exists to prove the library agrees with it.
            let mut linked = None;
            if args.transport != "wire" {
                let upstream = Rc::new(RefCell::new(Upstream::new(pack.tools.clone())));
                let mut target = ai2rules(&pack, &upstream, Transport::Linked)?;
                let run = run_scenario(scenario, &mut *target, &upstream);
                if args.transport == "linked" {
                    record(&mut targets, &*target);
                }
                linked = Some(run);
            }
            match &wire_binary {
                None => runs.push(linked.take().expect("linked run when transport=linked")),
                Some(binary) => {
                    let upstream = Rc::new(RefCell::new(Upstream::new(pack.tools.clone())));
                    let mut target = ai2rules(&pack, &upstream, Transport::Wire(binary.clone()))?;
                    record(&mut targets, &*target);
                    let wire = run_scenario(scenario, &mut *target, &upstream);
                    if let Some(linked) = linked {
                        match compare(&linked, &wire) {
                            Ok(()) => parity_detail
                                .push(format!("`{}` agreed on every step", scenario.id)),
                            Err(detail) => {
                                parity_ok = false;
                                parity_detail.push(format!("`{}`: {detail}", scenario.id));
                            }
                        }
                    }
                    runs.push(wire);
                }
            }
        }
    }

    let transport_parity =
        (args.transport == "both" && args.target != "weak").then(|| TransportParity {
            status: if parity_ok {
                Status::Pass
            } else {
                Status::Fail
            },
            compared: vec!["linked".to_string(), "wire".to_string()],
            detail: parity_detail.join("; "),
        });

    let result = BenchResult {
        v: RESULT_VERSION,
        pack: PackIdentity {
            path: args.pack.display().to_string(),
            world_id: pack.world.world_id().as_str().to_string(),
            manifest_hash: pack.world.manifest_hash().as_str().to_string(),
            scenarios: pack.scenarios.iter().map(|s| s.id.clone()).collect(),
        },
        targets,
        runs,
        transport_parity,
    };

    std::fs::create_dir_all(&args.out).map_err(|e| format!("{}: {e}", args.out.display()))?;
    let json = serde_json::to_string_pretty(&result).map_err(|e| e.to_string())?;
    write(&args.out.join("results.json"), &format!("{json}\n"))?;
    write(&args.out.join("REPORT.md"), &report::render(&result))?;

    if !args.quiet {
        print_matrix(&result);
        println!(
            "\nwrote {}/results.json and {}/REPORT.md",
            args.out.display(),
            args.out.display()
        );
    }

    if args.assert_contrast {
        if let Err(problem) = contrast(&result) {
            eprintln!("govbench: {problem}");
            return Ok(ExitCode::from(2));
        }
    }
    if result
        .transport_parity
        .as_ref()
        .is_some_and(|parity| parity.status == Status::Fail)
    {
        eprintln!("govbench: linked and wire transports disagreed");
        return Ok(ExitCode::from(2));
    }
    Ok(ExitCode::SUCCESS)
}

fn ai2rules(
    pack: &Pack,
    upstream: &Rc<RefCell<Upstream>>,
    transport: Transport,
) -> Result<Box<Ai2rules>, String> {
    Ai2rules::new(
        upstream.clone(),
        pack.world.clone(),
        &pack.world_path,
        transport,
    )
    .map(Box::new)
    .map_err(|e| format!("cannot start the ai2rules target: {e}"))
}

fn record(targets: &mut Vec<TargetIdentity>, target: &dyn Target) {
    if targets.iter().any(|known| known.id == target.id()) {
        return;
    }
    targets.push(TargetIdentity {
        id: target.id().to_string(),
        description: target.description().to_string(),
        metadata: target.metadata(),
    });
}

/// The `harness` binary for the wire transport.
fn locate_harness(explicit: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return if path.exists() {
            Ok(path)
        } else {
            Err(format!("{}: no such harness binary", path.display()))
        };
    }
    for candidate in ["target/debug/harness", "target/release/harness"] {
        let path = PathBuf::from(candidate);
        if path.exists() {
            return Ok(path);
        }
    }
    Err(
        "no `harness` binary found — build it (`cargo build -p cli-harness`), pass \
         --harness PATH, or run with --transport linked"
            .to_string(),
    )
}

/// Do the two transports agree about what happened? Evidence is excluded: it
/// names the transport by design.
fn compare(linked: &RunResult, wire: &RunResult) -> Result<(), String> {
    if linked.effect_count != wire.effect_count {
        return Err(format!(
            "effect count {} (linked) vs {} (wire)",
            linked.effect_count, wire.effect_count
        ));
    }
    if linked.steps.len() != wire.steps.len() {
        return Err("different step counts".to_string());
    }
    for (a, b) in linked.steps.iter().zip(&wire.steps) {
        if a.verdict != b.verdict || a.rule != b.rule {
            return Err(format!(
                "step `{}`: {} {:?} (linked) vs {} {:?} (wire)",
                a.id,
                a.verdict.as_str(),
                a.rule,
                b.verdict.as_str(),
                b.rule
            ));
        }
        if a.visible != b.visible || a.surface_id != b.surface_id {
            return Err(format!("step `{}`: different surface", a.id));
        }
        if a.effect_applied != b.effect_applied {
            return Err(format!("step `{}`: different downstream effect", a.id));
        }
    }
    Ok(())
}

/// The acceptance check, stated at the call site rather than in the oracle: the
/// weak baseline's intentional failures are all detected, and ai2rules produces
/// the expected outcome everywhere.
fn contrast(result: &BenchResult) -> Result<(), String> {
    let mut problems = Vec::new();
    for run in &result.runs {
        match (run.target.as_str(), run.outcome) {
            (WeakGateway::ID, Status::Pass) => problems.push(format!(
                "`{}` passed `{}`; the reference gateway's defect there is no longer detected",
                run.target, run.scenario
            )),
            (Ai2rules::ID, Status::Fail) => {
                problems.push(format!("`{}` failed `{}`", run.target, run.scenario))
            }
            _ => {}
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

fn print_matrix(result: &BenchResult) {
    let targets: Vec<&str> = result.targets.iter().map(|t| t.id.as_str()).collect();
    let width = result
        .pack
        .scenarios
        .iter()
        .map(String::len)
        .max()
        .unwrap_or(8);
    print!("{:width$}", "scenario", width = width);
    for target in &targets {
        print!("  {target:>24}");
    }
    println!();
    for scenario in &result.pack.scenarios {
        print!("{scenario:width$}", width = width);
        for target in &targets {
            let cell = result
                .runs
                .iter()
                .find(|run| &run.scenario == scenario && &run.target == target)
                .map(|run| format!("{} ({} effects)", run.outcome.as_str(), run.effect_count))
                .unwrap_or_else(|| "—".to_string());
            print!("  {cell:>24}");
        }
        println!();
    }
}

fn write(path: &std::path::Path, contents: &str) -> Result<(), String> {
    std::fs::write(path, contents).map_err(|e| format!("{}: {e}", path.display()))
}
