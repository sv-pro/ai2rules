//! AI2-111 spike: the business layer of the WebMCP stand, checked three ways.
//!
//! 1. From `profile.yaml` (fixed classes, no engine).
//! 2. From the Cedar policies, by evaluating every request of a finite domain
//!    (2 systems x 7 tools x every context) with the concrete authorizer.
//! 3. From the same policies, symbolically, with SymCC and cvc5: the checks hold
//!    for every request and every entity store, not just the enumerated ones.
//!
//! Each way yields the same three facts the D3 world is compiled from: which
//! systems hold private data, which tools need approval, which tools must not
//! exist. `compile-world.mjs` turns them into worlds and compares them with the
//! lab's own. Then: property checks, and regression checks on loosened edits.
//!
//! Usage: `cedar-check <spike dir>`, with CVC5=<path to cvc5 1.3.1> for part 3.
//! Prints one JSON report on stdout.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Instant;

use cedar_policy::{
    Authorizer, Context, Decision, Effect, Entities, EntityUid, EvalResult, PolicySet, Request,
    RequestEnv, Schema,
};
use cedar_policy_symcc::{solver::LocalSolver, CedarSymCompiler, CompiledPolicySet};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// The facts a D3 world is compiled from, whatever they were derived from.
/// Tool names are `system/tool`, as in the profile.
#[derive(Serialize, Default, PartialEq, Debug)]
struct Derived {
    sensitive_systems: BTreeSet<String>,
    approvals: BTreeSet<String>,
    forbidden: BTreeSet<String>,
    /// Per tool: may data from a confidential system reach it from elsewhere?
    /// `true` = never (the tool's arguments are Public sinks).
    private_stays_home: BTreeMap<String, bool>,
}

// ---------------------------------------------------------------- profile ---

#[derive(Deserialize)]
struct Profile {
    systems: BTreeMap<String, ProfileSystem>,
    entities: BTreeMap<String, ProfileEntity>,
    actions: BTreeMap<String, ProfileAction>,
}

#[derive(Deserialize)]
struct ProfileSystem {
    #[serde(default)]
    holds: Vec<String>,
}

#[derive(Deserialize)]
struct ProfileEntity {
    confidentiality: String,
}

#[derive(Deserialize)]
struct ProfileAction {
    #[serde(default)]
    approval: Option<String>,
    #[serde(default = "yes")]
    allowed: bool,
}

fn yes() -> bool {
    true
}

fn derive_from_profile(dir: &Path) -> Res<Derived> {
    let p: Profile = serde_yaml::from_str(&std::fs::read_to_string(dir.join("profile.yaml"))?)?;
    let private = |e: &String| {
        p.entities
            .get(e)
            .is_some_and(|e| matches!(e.confidentiality.as_str(), "private" | "regulated"))
    };
    let mut d = Derived::default();
    for (name, sys) in &p.systems {
        if sys.holds.iter().any(private) {
            d.sensitive_systems.insert(name.clone());
        }
    }
    for (tool, a) in &p.actions {
        if !a.allowed {
            d.forbidden.insert(tool.clone());
            continue;
        }
        if a.approval.is_some() {
            d.approvals.insert(tool.clone());
        }
        // The profile states the flow rule once, by class: private entities
        // never leave their system. So it holds for every tool.
        d.private_stays_home.insert(tool.clone(), true);
    }
    Ok(d)
}

// ------------------------------------------------------------------ cedar ---

struct Domain {
    schema: Schema,
    policies: PolicySet,
    entities: Entities,
}

fn load(dir: &Path, schema: &str, policies: &str, entities: Option<&str>) -> Res<Domain> {
    let cedar = dir.join("cedar");
    let (schema, _warnings) =
        Schema::from_cedarschema_str(&std::fs::read_to_string(cedar.join(schema))?)?;
    let policies = PolicySet::from_str(&std::fs::read_to_string(cedar.join(policies))?)?;
    let entities = match entities {
        Some(f) => {
            Entities::from_json_str(&std::fs::read_to_string(cedar.join(f))?, Some(&schema))?
        }
        None => Entities::empty(),
    };
    Ok(Domain {
        schema,
        policies,
        entities,
    })
}

fn uid(s: &str) -> EntityUid {
    EntityUid::from_str(s).expect("entity uid literal")
}

fn permits_only(p: &PolicySet) -> Res<PolicySet> {
    Ok(PolicySet::from_policies(
        p.policies()
            .filter(|p| p.effect() == Effect::Permit)
            .cloned(),
    )?)
}

/// Every subset of `items`.
fn subsets(items: &[String]) -> Vec<Vec<String>> {
    (0..1usize << items.len())
        .map(|m| {
            (0..items.len())
                .filter(|i| m & (1 << i) != 0)
                .map(|i| items[i].clone())
                .collect()
        })
        .collect()
}

fn northwind_allowed(
    d: &Domain,
    ps: &PolicySet,
    tool: &str,
    sys: &str,
    pf: &[String],
    approved: bool,
) -> Res<bool> {
    let action = uid(&format!(r#"Northwind::Action::"{tool}""#));
    let pf: Vec<Value> = pf
        .iter()
        .map(|s| json!({"__entity": {"type": "Northwind::System", "id": s}}))
        .collect();
    let ctx = Context::from_json_value(
        json!({"private_from": pf, "approved": approved}),
        Some((&d.schema, &action)),
    )?;
    let req = Request::new(
        uid(r#"Northwind::Agent::"assistant""#),
        action,
        uid(&format!(r#"Northwind::System::"{sys}""#)),
        ctx,
        Some(&d.schema),
    )?;
    Ok(Authorizer::new()
        .is_authorized(&req, ps, &d.entities)
        .decision()
        == Decision::Allow)
}

fn northwind_tools(d: &Domain) -> Vec<String> {
    d.schema
        .request_envs()
        .map(|e| e.action().id().unescaped().to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn northwind_systems(d: &Domain) -> Vec<(String, bool)> {
    d.entities
        .iter()
        .filter(|e| e.uid().type_name().to_string() == "Northwind::System")
        .map(|e| {
            let confidential = matches!(e.attr("confidential"), Some(Ok(EvalResult::Bool(true))));
            (e.uid().id().unescaped().to_string(), confidential)
        })
        .collect()
}

/// The tool -> system binding, read off the permit policies alone (the forbids
/// may remove a tool entirely; it still lives on its system).
fn bindings(d: &Domain) -> Res<Vec<(String, String)>> {
    let permits = permits_only(&d.policies)?;
    let mut out = vec![];
    for tool in northwind_tools(d) {
        for (sys, _) in northwind_systems(d) {
            if northwind_allowed(d, &permits, &tool, &sys, &[], true)? {
                out.push((tool.clone(), sys));
            }
        }
    }
    Ok(out)
}

fn derive_by_enumeration(d: &Domain) -> Res<(Derived, usize)> {
    let systems = northwind_systems(d);
    let names: Vec<String> = systems.iter().map(|(s, _)| s.clone()).collect();
    let mut out = Derived {
        sensitive_systems: systems
            .iter()
            .filter(|(_, c)| *c)
            .map(|(s, _)| s.clone())
            .collect(),
        ..Default::default()
    };
    let mut requests = 0;
    for (tool, sys) in bindings(d)? {
        let name = format!("{sys}/{tool}");
        let (mut any, mut unapproved, mut leaks) = (false, false, false);
        for pf in subsets(&names) {
            for approved in [false, true] {
                requests += 1;
                if northwind_allowed(d, &d.policies, &tool, &sys, &pf, approved)? {
                    any = true;
                    unapproved |= !approved;
                    leaks |= pf.iter().any(|s| s != &sys);
                }
            }
        }
        if !any {
            out.forbidden.insert(name);
            continue;
        }
        if !unapproved {
            out.approvals.insert(name.clone());
        }
        out.private_stays_home.insert(name, !leaks);
    }
    Ok((out, requests))
}

fn env_for(d: &Domain, action: &str) -> Res<RequestEnv> {
    d.schema
        .request_envs()
        .find(|e| e.action().id().unescaped() == action)
        .ok_or_else(|| format!("no request environment for action {action}").into())
}

async fn derive_symbolically(d: &Domain, sc: &mut CedarSymCompiler<LocalSolver>) -> Res<Derived> {
    let q_approved =
        PolicySet::from_str("permit (principal, action, resource) when { context.approved };")?;
    let q_home = PolicySet::from_str("permit (principal, action, resource) when { [resource].containsAll(context.private_from) };")?;
    let mut out = Derived {
        sensitive_systems: northwind_systems(d)
            .into_iter()
            .filter(|(_, c)| *c)
            .map(|(s, _)| s)
            .collect(),
        ..Default::default()
    };
    for (tool, sys) in bindings(d)? {
        let name = format!("{sys}/{tool}");
        let env = env_for(d, &tool)?;
        let p = CompiledPolicySet::compile(&d.policies, &env, &d.schema)?;
        if sc.check_always_denies_opt(&p).await? {
            out.forbidden.insert(name);
            continue;
        }
        let qa = CompiledPolicySet::compile(&q_approved, &env, &d.schema)?;
        if sc.check_implies_opt(&p, &qa).await? {
            out.approvals.insert(name.clone());
        }
        let qh = CompiledPolicySet::compile(&q_home, &env, &d.schema)?;
        out.private_stays_home
            .insert(name, sc.check_implies_opt(&p, &qh).await?);
    }
    Ok(out)
}

/// Does `candidate` allow only what `baseline` allows, for every request of
/// every action? Returns the counterexamples where it does not.
async fn regressions(
    baseline: &Domain,
    candidate: &PolicySet,
    sc: &mut CedarSymCompiler<LocalSolver>,
) -> Res<Vec<String>> {
    let mut found = vec![];
    for env in baseline.schema.request_envs() {
        let cand = CompiledPolicySet::compile(candidate, &env, &baseline.schema)?;
        let base = CompiledPolicySet::compile(&baseline.policies, &env, &baseline.schema)?;
        if let Some(cex) = sc
            .check_implies_with_counterexample_opt(&cand, &base)
            .await?
        {
            found.push(describe(&cex.request, &cex.entities));
        }
    }
    Ok(found)
}

fn describe(req: &Request, entities: &Entities) -> String {
    let ctx = req
        .context()
        .and_then(|c| c.to_json_value().ok())
        .unwrap_or(Value::Null);
    let ents = entities.to_json_value().unwrap_or(Value::Null);
    format!("{req} | context {ctx} | entities {ents}")
}

/// One property: `policies` imply `property` for `action`, and the action is
/// not vacuously denied (an always-deny would imply anything).
async fn property(
    d: &Domain,
    policies: &PolicySet,
    action: &str,
    property: &str,
    sc: &mut CedarSymCompiler<LocalSolver>,
) -> Res<Value> {
    let env = env_for(d, action)?;
    let p = CompiledPolicySet::compile(policies, &env, &d.schema)?;
    let q = CompiledPolicySet::compile(&PolicySet::from_str(property)?, &env, &d.schema)?;
    let started = Instant::now();
    let vacuous = sc.check_always_denies_opt(&p).await?;
    let cex = sc.check_implies_with_counterexample_opt(&p, &q).await?;
    Ok(json!({
        "action": action,
        "property": property,
        "holds": cex.is_none() && !vacuous,
        "vacuous": vacuous,
        "counterexample": cex.map(|c| describe(&c.request, &c.entities)),
        "ms": started.elapsed().as_millis(),
    }))
}

async fn symbolic_part(dir: &Path, nw: &Domain) -> Res<Value> {
    let mut sc = CedarSymCompiler::new(LocalSolver::cvc5()?)?;
    let started = Instant::now();
    let derived = derive_symbolically(nw, &mut sc).await?;
    let derive_ms = started.elapsed().as_millis();

    let loosened = load(
        dir,
        "northwind.cedarschema",
        "northwind-loosened.cedar",
        Some("northwind.entities.json"),
    )?
    .policies;
    let books = load(dir, "books.cedarschema", "books.cedar", None)?;
    let books_loose = load(dir, "books.cedarschema", "books-loosened.cedar", None)?.policies;

    let pay_rule = "permit (principal, action, resource) when { resource.amount <= 10000 || context.cfo_approved };";
    let vendor_rule = "permit (principal, action, resource) when { [resource].containsAll(context.carries_invoice_of) };";
    let home_rule = "permit (principal, action, resource) when { [resource].containsAll(context.private_from) };";

    Ok(json!({
        "derived": derived,
        "derive_ms": derive_ms,
        "properties": [
            property(nw, &nw.policies, "search_products", home_rule, &mut sc).await?,
            property(nw, &loosened, "search_products", home_rule, &mut sc).await?,
            property(&books, &books.policies, "pay_invoice", pay_rule, &mut sc).await?,
            property(&books, &books.policies, "email_vendor", vendor_rule, &mut sc).await?,
            property(&books, &books_loose, "pay_invoice", pay_rule, &mut sc).await?,
        ],
        "regressions": {
            "northwind-loosened.cedar vs northwind.cedar": regressions(nw, &loosened, &mut sc).await?,
            "northwind.cedar vs itself": regressions(nw, &nw.policies, &mut sc).await?,
            "books-loosened.cedar vs books.cedar": regressions(&books, &books_loose, &mut sc).await?,
        },
    }))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Res<()> {
    let dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "..".into()));
    let profile = derive_from_profile(&dir)?;
    let nw = load(
        &dir,
        "northwind.cedarschema",
        "northwind.cedar",
        Some("northwind.entities.json"),
    )?;
    let (enumerated, requests) = derive_by_enumeration(&nw)?;
    let symbolic = match std::env::var_os("CVC5") {
        Some(_) => symbolic_part(&dir, &nw)
            .await
            .unwrap_or_else(|e| json!({ "error": e.to_string() })),
        None => json!({ "skipped": "CVC5 is not set" }),
    };
    let agree = symbolic
        .get("derived")
        .map(|s| s == &serde_json::to_value(&enumerated).unwrap());
    let report = json!({
        "profile": profile,
        "cedar_enumerated": { "derived": enumerated, "requests": requests },
        "cedar_symbolic": symbolic,
        "profile_equals_cedar": profile == enumerated,
        "enumeration_equals_symbolic": agree,
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
