# Prior art and scenario naming — AI2-88 / #165

Checked online on 2026-10-26. Primary sources were opened, including full HTML
papers, not inferred from titles/search snippets. Paper claims below are reported
claims; their experiments were not independently rerun. Mappings are our
interpretation, not assertions that the lab reproduces published experiments.
The lab's S1–S5 identifiers are local names, not paper condition identifiers.

## Naming map

| Lab scenario | Tool Surface Poisoning [TSP] | WebMCP-Phalanx [P] | WASP [W] | Chrome guidance [C] |
|---|---|---|---|---|
| S1 page body | General content injection; not a named MSTI condition | Semantic/content poisoning, approximate | Plain-text injection analogue | Non-WebMCP untrusted page content |
| S2 description | Tool Framing, closest to C2 | Semantic poisoning at description checkpoint | No WebMCP-description taxonomy | Malicious manifests |
| S3 substitution | Tool Hijacking; replacement/timing variant | Protocol/state manipulation | No registration-lifecycle counterpart | Tool identity/lifecycle not a named attack class |
| S4 failure/retry | Tool Hijacking; only approximate C1/C3 relation | Protocol/state manipulation | No registration-lifecycle counterpart | No named failure/re-registration class |
| S5 cross-tab leak | Leakage objective; not a named cross-tab condition | Related to cross-page residual; different mechanism | Exfiltration goal analogue | Cross-origin data leakage |

## [TSP] Tool Surface Poisoning

[arXiv 2606.06387v1](https://arxiv.org/html/2606.06387v1), sections 2–4,
Tables 2–3, and section 6.

MSTI distinguishes Tool Hijacking (availability/identity manipulation) from Tool
Framing (metadata semantics). C1 aborts/replaces a tool; C3 preempts registration
under first-register-wins semantics. Neither is simply global latest-name
resolution across tabs. The timing experiments include a post-first-use point.
ASR requires malicious invocation with task data reaching the attacker endpoint;
task completion is measured independently. Proposed mitigations include origin
binding, lifecycle consistency, data boundaries and audit logs; these are design
recommendations, not a measured D1/D2 comparison. Our arbitrary marker follower
cannot reproduce the paper's model-selection measurements. S4 fallback between
tabs must not be advertised as a literal C3 registration-race reproduction.

## [P] WebMCP-Phalanx

[arXiv 2608.24017v1](https://arxiv.org/html/2608.24017v1), sections 3–7.

The architecture combines capability ownership, lifecycle/telemetry labels and
quarantine/privileged agents. Attack success requires malicious invocation plus
receipt of exfiltrated data; task completion is compared with a no-attack baseline.
Labels alone do not resolve semantic injection; same-origin capabilities prove
possession, not benevolence. Section 7 qualifies the implementation as a polyfill:
native provenance is assumed, some proposed components are unevaluated, and
post-execution monitoring cannot prevent the first effect. Cross-page residuals
involve retained instructions after navigation, unlike our simultaneous-tab S5.
Our frozen JavaScript objects are not evidence of native isolation. A proposed
source/recipient data-label experiment must be evaluated separately from this
paper's tool-trust labels and semantic agents.

## [W] WASP

[Paper, arXiv 2504.18575v1](https://arxiv.org/html/2504.18575v1), sections
3.2–3.4 and 4.1–4.2;
[official repository](https://github.com/facebookresearch/wasp) and
[README](https://raw.githubusercontent.com/facebookresearch/wasp/main/README.md).

WASP separates attacker objectives from injection templates (plain text and URL)
and task-aware versus task-agnostic attackers. End-to-end ASR uses goal-specific
final-state rules; intermediate ASR uses an LLM judge of diversion. Utility under
attack checks legitimate task outcomes; clean tasks provide another baseline.
Evaluated mitigations include system-prompt defenses and instruction hierarchy.
There is no WebMCP registration taxonomy to assign to S2–S4. Our deterministic
agent deliberately removes the model-capability bottleneck, so its booleans are
not comparable to WASP ASR percentages. Keep attempted diversion separate from
completed harm. The repository also documents API-backed evaluators; this lab's
oracle is deliberately deterministic.

## [C] Chrome security guidance

[Agent security considerations](https://developer.chrome.com/docs/agents/security),
published 2026-06-09. It distinguishes malicious manifests from contaminated
outputs and recommends origin restrictions, confirmations, input limits,
spotlighting and content/intent checks. It asks for security evaluation without
unnecessary capability loss but specifies no universal ASR/utility formula.
Our fully compromised marker follower does not evaluate semantic classifiers or
human confirmation, so D0/D1/D2 cannot rank this entire guidance stack.

[Tool security](https://developer.chrome.com/docs/ai/webmcp/secure-tools), updated
2026-09-01. It documents document.modelContext, annotation hints, and explicit
cross-origin exposure via exposedTo; read-only tools can disclose private data.
The lab's agent-level union of several tabs is an orchestration choice, not proof
that website scripts can access one another's tools under native defaults.
The shim needs an explicit supported-subset statement, not a claim of full API
conformance. Browser versions and draft revisions must accompany native tests.

## Consequences for this experiment (verifier interpretation)

These are verification requirements, not new claims about the cited systems:

- Name the tested mechanisms precisely. S3 triggered after search_products is
  a changed timing case. S4 with an already registered attacker tab is a
  cross-tab fallback case. Preserve the original requested variants as separate
  pending coverage, or explicitly narrow the claims.
- Record the lab's broader success predicate: non-CRM receipt of the canary or
  unauthorized state changes can count even without attacker-network receipt.
  Publish subpredicates so comparisons do not mix different definitions.
- Pair attacked utility with each defense's clean utility. A defense blocking
  all writes can pass a leak check while failing the user's task.
- Separate pinning, projection and taint evidence. A failed attack before
  reaching a mechanism says nothing about that mechanism's effectiveness.
- Freeze policy revisions and report amendments. Earlier commits establish
  artifact order; they do not establish attack-blind rule authoring when the
  scenario brief was already known.
- Preserve coarse D2 when exploring finer labels. Compare a separately named
  variant on the same tasks and security checks; no measured regressions on a
  finite set are not a general proof of safety.

## Verification limits

Internet access was available. All four requested source families were read.
This note uses the explicit paper versions above; it does not establish that
there are no later revisions. Published implementations, native Chrome behavior,
MCP-B API compatibility and CaMeL were not independently executed/validated.
CaMeL remains a follow-on research pointer, not evidence supporting this table.
D1 is only the lab's rule-based baseline; no external product is evaluated.
