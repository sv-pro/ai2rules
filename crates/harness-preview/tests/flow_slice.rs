//! The AI2-70 vertical slice, end to end (D95): a scripted, flow-capable host
//! drives `read_document → summarize_fixture → publish_report` through the real
//! gate ABI against `docs/demos/one-kernel/flow-world.yaml`, binding labels to
//! the values the kernel returns and counting the effects its one sink runs.
//!
//! What the proof is: every DENY vector asserts `effects() == 0` and the ALLOW
//! control asserts `effects() == 1` — observed on the host's own ledger, never
//! inferred from a verdict. The session is Trusted and clean throughout; the
//! verdicts come from the labels alone.
//!
//! What the host is: the minimum a real adapter has to do to carry labels.
//! Labels bind to values by digest, so a value the model copies verbatim keeps
//! its label and a value the host has never seen falls back to the ambient.
//! That is also the honest limit of v0 (D94's partial-quote caveat): a value
//! the model transforms is a new value, and re-binding it is the host's
//! problem, not something the kernel infers.

use std::collections::BTreeMap;

use compiler::{compile, load_yaml, sha256_hex};
use harness_preview::{
    gate_and_lower, gate_and_lower_authorized, GateBacking, GateContext, GateEffective,
    GateFlowContext, GateLabel, GateRequest, GateResponse, GateUsage, ABI_VERSION,
};
use harness_types::{CompiledWorld, Confidentiality, FlowLabel, Integrity};
use serde_json::{json, Value};

const TP: FlowLabel = FlowLabel::TRUSTED_PUBLIC;
const TPRIV: FlowLabel = FlowLabel {
    integrity: Integrity::Trusted,
    confidentiality: Confidentiality::Private,
};
const UP: FlowLabel = FlowLabel {
    integrity: Integrity::Untrusted,
    confidentiality: Confidentiality::Public,
};

const DESTINATION: &str = "https://reports.example/weekly";

fn world() -> CompiledWorld {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/demos/one-kernel/flow-world.yaml");
    let text = std::fs::read_to_string(&path).expect("read flow-world.yaml");
    compile(&load_yaml(&text).expect("parses")).expect("compiles")
}

fn from_wire(label: &GateLabel) -> FlowLabel {
    FlowLabel {
        integrity: match label.integrity.as_str() {
            "trusted" => Integrity::Trusted,
            "untrusted" => Integrity::Untrusted,
            other => panic!("integrity {other}"),
        },
        confidentiality: match label.confidentiality.as_str() {
            "public" => Confidentiality::Public,
            "private" => Confidentiality::Private,
            other => panic!("confidentiality {other}"),
        },
    }
}

/// One governed call as the host saw it.
#[derive(Debug, Clone, PartialEq)]
struct Step {
    tool: String,
    decision: String,
    rule: Option<String>,
    /// The label the host bound to the returned value (ALLOW only).
    result_label: Option<FlowLabel>,
    /// The value the handler returned (ALLOW only).
    output: Option<String>,
    /// Size of the effect ledger after this call.
    effects_after: usize,
}

/// A scripted host that carries flow labels: the ambient session label, a
/// digest-keyed table of value labels, three local handlers, and the counted
/// sink. Everything the kernel needs comes from here; nothing comes from the
/// model's arguments or a tool's metadata.
struct FlowHost {
    world: CompiledWorld,
    /// Persisted, monotonic: only ever joined with what the gate returns.
    ambient: FlowLabel,
    taint: String,
    usage: GateUsage,
    /// sha256(value) → label. Host-owned; joined on every bind, never lowered.
    labels: BTreeMap<String, FlowLabel>,
    /// The effect ledger of the one sink: `(to, body)` per publish that ran.
    published: Vec<(String, String)>,
    fixtures: BTreeMap<&'static str, &'static str>,
    /// What the publish handler's `_meta` claims about its result, if anything.
    /// A remote tool's opinion of its own output; the host must ignore it.
    handler_claim: Option<FlowLabel>,
}

impl FlowHost {
    fn new() -> Self {
        Self {
            world: world(),
            ambient: TP,
            taint: "clean".to_string(),
            usage: GateUsage::default(),
            labels: BTreeMap::new(),
            published: Vec::new(),
            fixtures: [("q3-forecast", "Q3 forecast: revenue 4.2M, churn 3%.")]
                .into_iter()
                .collect(),
            handler_claim: None,
        }
    }

    fn effects(&self) -> usize {
        self.published.len()
    }

    /// Bind a label to a value. Joins with any label already bound, so a later
    /// claim can only raise a label, never lower it.
    fn bind(&mut self, value: &str, label: FlowLabel) {
        self.labels
            .entry(sha256_hex(value.as_bytes()))
            .and_modify(|l| *l = l.join(label))
            .or_insert(label);
    }

    fn label_of(&self, value: &str) -> Option<FlowLabel> {
        self.labels.get(&sha256_hex(value.as_bytes())).copied()
    }

    fn context(&self) -> GateContext {
        GateContext {
            session_id: "slice".to_string(),
            mode: Some("interactive".to_string()),
            taint: Some(self.taint.clone()),
            source_channel: Some("user_prompt".to_string()),
            approval_token: None,
            usage: Some(self.usage),
            flow: None,
        }
    }

    /// The request for a call, with every string argument the host has a label
    /// for labelled by digest. Unknown values are omitted — the kernel falls
    /// back to the ambient for them.
    fn request(&self, tool: &str, arguments: Value) -> GateRequest {
        let args = arguments
            .as_object()
            .into_iter()
            .flat_map(|o| o.iter())
            .filter_map(|(k, v)| {
                let label = self.label_of(v.as_str()?)?;
                Some((k.clone(), GateLabel::from(label)))
            })
            .collect();
        GateRequest {
            v: ABI_VERSION,
            tool: tool.to_string(),
            arguments,
            path: None,
            context: GateContext {
                flow: Some(GateFlowContext {
                    ambient: Some(GateLabel::from(self.ambient)),
                    args,
                }),
                ..self.context()
            },
        }
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Step {
        let req = self.request(tool, arguments);
        self.govern(&req, false)
    }

    /// Resume after the human said yes: the native authorized entry point.
    fn call_authorized(&mut self, tool: &str, arguments: Value) -> Step {
        let req = self.request(tool, arguments);
        self.govern(&req, true)
    }

    fn govern(&mut self, req: &GateRequest, authorized: bool) -> Step {
        let (res, _spec) = if authorized {
            gate_and_lower_authorized(&self.world, req)
        } else {
            gate_and_lower(&self.world, req)
        };
        self.persist(&res);
        let (output, result_label) = if res.decision == "ALLOW" {
            let effective = res
                .effective
                .as_ref()
                .expect("ALLOW carries the effective call");
            let (output, claim) = self.execute(effective);
            // The kernel's floor, raised by nothing less: a handler's claim may
            // only ever make the label more conservative.
            let floor = res
                .context
                .flow
                .as_ref()
                .and_then(|f| f.result.as_ref())
                .map(from_wire);
            let label = match (floor, claim) {
                (Some(floor), Some(claim)) => Some(floor.join(claim)),
                (Some(floor), None) => Some(floor),
                // No floor: the result is unlabelled; the host binds nothing
                // and the value falls back to the ambient when it comes back.
                (None, _) => None,
            };
            if let Some(label) = label {
                self.bind(&output, label);
            }
            (Some(output), label)
        } else {
            (None, None)
        };
        Step {
            tool: req.tool.clone(),
            decision: res.decision.clone(),
            rule: res.rule.clone(),
            result_label,
            output,
            effects_after: self.effects(),
        }
    }

    fn persist(&mut self, res: &GateResponse) {
        self.taint = res.context.taint.clone();
        self.usage = res.context.usage;
        if let Some(flow) = &res.context.flow {
            self.ambient = self.ambient.join(from_wire(&flow.ambient));
        }
    }

    /// Dispatch the lowered call to its handler. Returns the output and the
    /// label the handler *claims* for it (only the remote sink claims one).
    fn execute(&mut self, effective: &GateEffective) -> (String, Option<FlowLabel>) {
        let arg = |k: &str| {
            effective.arguments[k]
                .as_str()
                .unwrap_or_default()
                .to_string()
        };
        match &effective.backing {
            GateBacking::Local { handler } if handler == "read_document" => {
                let name = arg("name");
                let body = self
                    .fixtures
                    .get(name.as_str())
                    .unwrap_or_else(|| panic!("no fixture {name}"));
                ((*body).to_string(), None)
            }
            GateBacking::Local { handler } if handler == "summarize_fixture" => {
                let body = arg("body");
                let prefix: String = body.chars().take(11).collect();
                (format!("summary: {prefix}"), None)
            }
            GateBacking::Mcp { server, tool } if server == "reports" && tool == "publish" => {
                self.published.push((arg("to"), arg("body")));
                ("published".to_string(), self.handler_claim)
            }
            other => panic!("unexpected backing {other:?}"),
        }
    }
}

fn read(host: &mut FlowHost) -> Step {
    host.call("read_document", json!({ "name": "q3-forecast" }))
}

fn summarize(host: &mut FlowHost, body: &str) -> Step {
    host.call("summarize_fixture", json!({ "body": body }))
}

fn publish_args(body: &str) -> Value {
    json!({ "to": DESTINATION, "body": body })
}

#[test]
fn private_body_cannot_be_published() {
    let mut host = FlowHost::new();

    let read = read(&mut host);
    assert_eq!(read.decision, "ALLOW");
    assert_eq!(read.result_label, Some(TPRIV));
    let document = read.output.clone().unwrap();
    assert_eq!(
        host.ambient, TPRIV,
        "the session now holds something private"
    );

    // The transform: a Private parent stays Private, with no classifier and no
    // declassification anywhere.
    let summary = summarize(&mut host, &document);
    assert_eq!(summary.decision, "ALLOW");
    assert_eq!(summary.result_label, Some(TPRIV));
    let text = summary.output.clone().unwrap();
    assert_ne!(text, document, "the summary is a new value");

    // The sink: a trusted caller, a clean session, and still a DENY — with
    // nothing run and nothing published.
    let publish = host.call("publish_report", publish_args(&text));
    assert_eq!(publish.decision, "DENY");
    assert_eq!(publish.rule.as_deref(), Some("confidentiality_violation"));
    assert_eq!(publish.output, None);
    assert_eq!(host.effects(), 0);
    assert_eq!(host.taint, "clean", "the taint bit never moved");
}

#[test]
fn public_body_is_published_exactly_once() {
    let mut host = FlowHost::new();
    // A body the user typed: unlabelled, so it falls back to the ambient, which
    // is Trusted/Public in a fresh session.
    let body = "Weekly digest: all green.";

    let ask = host.call("publish_report", publish_args(body));
    assert_eq!(ask.decision, "ASK");
    assert_eq!(ask.rule.as_deref(), Some("approval_required"));
    assert_eq!(host.effects(), 0, "nothing runs before the human answers");

    let allow = host.call_authorized("publish_report", publish_args(body));
    assert_eq!(allow.decision, "ALLOW");
    assert_eq!(host.effects(), 1, "the control publishes exactly once");
    assert_eq!(
        host.published[0],
        (DESTINATION.to_string(), body.to_string())
    );
    // A sink-only contract labels no result; the remote publish taints the
    // session as any MCP-backed call does (finding #13).
    assert_eq!(allow.result_label, None);
    assert_eq!(host.taint, "tainted");
    assert_eq!(host.ambient, UP);
}

#[test]
fn untrusted_destination_is_denied_by_the_existing_floor() {
    let mut host = FlowHost::new();
    // A destination the host saw arrive from somewhere untrusted.
    host.bind(DESTINATION, UP);
    let publish = host.call("publish_report", publish_args("Weekly digest: all green."));
    assert_eq!(publish.decision, "DENY");
    assert_eq!(publish.rule.as_deref(), Some("taint_invariant"));
    assert_eq!(host.effects(), 0);
}

#[test]
fn model_and_remote_claims_cannot_elevate() {
    let mut host = FlowHost::new();
    let document = read(&mut host).output.unwrap();

    // A host-side attempt to relabel the document as public: the bind joins,
    // so the Private floor stays.
    host.bind(&document, TP);
    assert_eq!(host.label_of(&document), Some(TPRIV));

    // The model passes the private document verbatim: the digest matches and
    // the label travels with it.
    let publish = host.call("publish_report", publish_args(&document));
    assert_eq!(publish.rule.as_deref(), Some("confidentiality_violation"));
    assert_eq!(host.effects(), 0);

    // A remote tool's `_meta` claiming its result is trusted/public cannot
    // lower the floor the kernel returned for it either. (The publish of a
    // public body is the only way to reach the handler; its claim is joined
    // onto a `None` floor and binds nothing, and onto a Private floor it would
    // be ignored by the join.)
    host.handler_claim = Some(TP);
    assert_eq!(TPRIV.join(TP), TPRIV, "a claim can only raise a label");
}

#[test]
fn missing_or_unknown_context_fails_closed() {
    let mut host = FlowHost::new();
    for (tool, args) in [
        ("publish_report", publish_args("Weekly digest: all green.")),
        ("read_document", json!({ "name": "q3-forecast" })),
    ] {
        // No sidecar at all.
        let mut req = host.request(tool, args.clone());
        req.context.flow = None;
        let step = host.govern(&req, false);
        assert_eq!(step.decision, "DENY", "{tool}");
        assert_eq!(step.rule.as_deref(), Some("missing_flow_context"), "{tool}");

        // A sidecar with no ambient label.
        let mut req = host.request(tool, args.clone());
        req.context.flow = Some(GateFlowContext::default());
        let step = host.govern(&req, false);
        assert_eq!(step.rule.as_deref(), Some("invalid_flow_context"), "{tool}");

        // A label outside the vocabulary.
        let mut req = host.request(tool, args);
        req.context.flow = Some(GateFlowContext {
            ambient: Some(GateLabel {
                integrity: "trusted".to_string(),
                confidentiality: "secret".to_string(),
            }),
            args: BTreeMap::new(),
        });
        let step = host.govern(&req, false);
        assert_eq!(step.rule.as_deref(), Some("invalid_flow_context"), "{tool}");
    }
    assert_eq!(host.effects(), 0);
}

#[test]
fn ask_does_not_erase_labels() {
    let mut host = FlowHost::new();
    let document = read(&mut host).output.unwrap();
    let text = summarize(&mut host, &document).output.unwrap();
    // Even the authorized resume — the human said yes — meets the same DENY:
    // a flow refusal is decided before disposition, where approval lives.
    let publish = host.call_authorized("publish_report", publish_args(&text));
    assert_eq!(publish.decision, "DENY");
    assert_eq!(publish.rule.as_deref(), Some("confidentiality_violation"));
    assert_eq!(host.effects(), 0);
}

#[test]
fn slice_is_deterministic() {
    let run = || -> Vec<Step> {
        let mut host = FlowHost::new();
        let mut steps = vec![];
        let document = {
            let s = read(&mut host);
            steps.push(s.clone());
            s.output.unwrap()
        };
        let text = {
            let s = summarize(&mut host, &document);
            steps.push(s.clone());
            s.output.unwrap()
        };
        steps.push(host.call("publish_report", publish_args(&text)));
        // The session now holds private material, so its ambient is Private and
        // an unlabelled value would be refused. The host knows this body came
        // from the user's own prompt and says so — a label is the host's word,
        // and that is what lets a public value out of a private session.
        let body = "Weekly digest: all green.";
        host.bind(body, TP);
        steps.push(host.call("publish_report", publish_args(body)));
        steps.push(host.call_authorized("publish_report", publish_args(body)));
        steps
    };
    let a = run();
    let b = run();
    assert_eq!(a, b);
    let decisions: Vec<&str> = a.iter().map(|s| s.decision.as_str()).collect();
    assert_eq!(decisions, ["ALLOW", "ALLOW", "DENY", "ASK", "ALLOW"]);
    assert_eq!(a.last().unwrap().effects_after, 1);
}

#[test]
fn an_unlabelled_value_in_a_private_session_is_private() {
    // The flip side of the host's word: once the session has read something
    // private, a value the host cannot vouch for is treated as private too.
    let mut host = FlowHost::new();
    read(&mut host);
    let publish = host.call("publish_report", publish_args("Weekly digest: all green."));
    assert_eq!(publish.rule.as_deref(), Some("confidentiality_violation"));
    assert_eq!(host.effects(), 0);
}
