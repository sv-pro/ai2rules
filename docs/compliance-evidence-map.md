# Compliance evidence map: control → artifact → obligation

*Status: draft for maintainer review (AI2-67 / #142). Clause numbers checked against
the sources listed at the end on 2026-10-23.*

Reviewers of an agent deployment tend to ask two questions about each control: *which
obligation is this evidence for*, and *what does it leave behind that I can inspect*.
This page answers both for the controls ai2rules ships today, one row per control.

**What this page is not.** It is not a compliance claim. Nothing here says that
ai2rules, or a system that uses it, meets any law, standard or framework.
Each row names an artifact and the obligations it can be *evidence for*. Whether that
evidence is enough, and whether an obligation applies at all, is for the organisation
deploying the agent and its assessors to decide. In particular:

- **EU AI Act.** Articles 9–15 are requirements for *high-risk* AI systems (Chapter III,
  Section 2). Article 19 is addressed to providers and Article 26 to deployers of those
  systems. A coding agent is not high-risk by default. The rows show what evidence
  exists *if* a deployment is in scope. The Digital Omnibus on AI changes when the
  high-risk obligations apply; check the current dates at the Commission's AI Act
  Service Desk, not here.
- **NIST AI RMF** is voluntary. Its subcategories describe outcomes, so a control can
  contribute evidence to one, never complete it.
- **ISO/IEC 42001** Annex A controls are organisational. A tool can supply records an
  AI management system uses; it cannot hold the certification.
- **OWASP Top 10 for Agentic Applications** names risks, not obligations. A row cites a
  risk when the control is a mitigation for it.

**Read the last column.** Several controls hold fully only in some runtimes. The
"limit" column says what the *default* configuration delivers, so a row never claims
more than you get from `harness init` and the shipped binaries. The three conditions
that come up most:

- On **Claude Code**, an `ABSENT` verdict refuses the call only when the hook runs
  with `--enforce-absent`. Without it the verdict passes through to Claude Code's own
  permission flow ([`cc_hook.rs`](../crates/cli-harness/src/cc_hook.rs), header
  comment).
- The **Claude Code and Antigravity hooks fail open** on a process failure: no
  kernel binary, an unreadable event, an unreadable or uncompilable world. A broken
  world lets every call through to the host's own flow. A configurable broken-world
  mode is open work ([#128](https://github.com/sv-pro/ai2rules/issues/128) / AI2-56).
- **Staged commit and execution receipts (D74)** are a tested prototype. The default
  agent loop does not use them ([`staged-effect-commit.md`](staged-effect-commit.md)).

Paths below are relative to the repository root. Every artifact named exists on
`main`; a named test can be run with `cargo test --offline -p <crate> <test name>`.

---

## The map

Framework shorthand: **EU** = EU AI Act article (paragraph); **NIST** = AI RMF 1.0
subcategory; **ISO** = ISO/IEC 42001:2023 Annex A control; **OWASP** = Agentic Top 10
(2026) entry. A dash means no clause is an honest fit.

| Control | Artifact: what it leaves, where | Evidence for | Limit in the default configuration |
|---|---|---|---|
| **Taint floor.** Once a session has read untrusted content, it cannot reach network egress, external writes, credentials or durable memory. A hard invariant runs before policy; a manifest or a human approval cannot override it ("without a policy-defined safe path"). ([architecture §6](harness-architecture.md), minimum hard invariants; [THESIS](THESIS.md)) | The rule: `transition_policies` in [`scripts/starter-world.yaml`](../scripts/starter-world.yaml). The kernel check: [`crates/world-kernel/src/invariants.rs`](../crates/world-kernel/src/invariants.rs), tests `tainted_external_action_violates_floor`, `tainted_target_still_blocks`; [`crates/world-kernel/src/lib.rs`](../crates/world-kernel/src/lib.rs) `tainted_value_cannot_reach_network`. On hosts: [`crates/cli-harness/tests/cc_hook.rs`](../crates/cli-harness/tests/cc_hook.rs) `tainted_egress_is_denied_by_the_taint_floor`, `grant_mode_still_denies_tainted_egress`, `unwritable_taint_sidecar_denies_the_escalating_call` ([D59](../DECISIONS.md#d59--an-unrecordable-taint-escalation-fails-closed-because-fail-open-covers-process-failures-and-this-is-not-one)); the same cases in [`agy_hook.rs`](../crates/cli-harness/tests/agy_hook.rs) and [`mcp_gateway.rs`](../crates/cli-harness/tests/mcp_gateway.rs). | **EU** 15(5) (resilience against attempts to alter use or outputs). **NIST** MEASURE 2.7. **ISO** A.7.5 (provenance, here of runtime inputs, not training data). **OWASP** ASI01 Agent Goal Hijack, ASI02 Tool Misuse, ASI06 Memory & Context Poisoning. | The floor `DENY` binds on every host by default. On a host, taint is kept in a per-session sidecar the user can write; the agent's file tools are kept off it, but a person or an approved shell command can delete it ([D82](../DECISIONS.md#d82--machine-wide-governance-a-user-scope-for-convenience-a-managed-scope-for-control), consequence). Hooks fail open on a broken world (AI2-56). What counts as untrusted is what the world declares: in the starter world a session becomes tainted on its first network call, and an untrusted ingest directory has to be added by hand. |
| **`ABSENT` and discovery projection.** An action the world does not project does not exist for the agent: it is filtered from the tool list and refused if called, as its own verdict, distinct from `DENY`. ([D72](../DECISIONS.md#d72--discovery-projection-is-a-sibling-wire-abi-the-world-owns-existence-and-the-host-owns-schema), [D75](../DECISIONS.md#d75--discovery-projection-lives-beside-gate-in-harness-preview)) | [`crates/world-kernel/src/lib.rs`](../crates/world-kernel/src/lib.rs) `known_but_unprojected_action_is_absent`, `decide_unknown_is_distinct_from_absent`; [`crates/harness-preview/src/project.rs`](../crates/harness-preview/src/project.rs) `filters_absent_without_rewriting_host_schema`; [`crates/cli-harness/tests/project.rs`](../crates/cli-harness/tests/project.rs); [`crates/cli-harness/tests/mcp_gateway.rs`](../crates/cli-harness/tests/mcp_gateway.rs) `tools_list_hides_destructive_tools`; [`agy_hook.rs`](../crates/cli-harness/tests/agy_hook.rs) `absent_passes_through_by_default_and_denies_under_enforcement`. | **EU** 9(2)(d) (risk-management measures), 15(5). **NIST** GOVERN 1.4. **ISO** A.9.4 (intended use). **OWASP** ASI02 Tool Misuse, ASI03 Identity & Privilege Abuse. | Holds as stated in `agent-core`, the MCP gateway and `harness project`. On **Claude Code and Antigravity** a hook cannot remove native tools from the model's surface, and `ABSENT` refuses the call only with `--enforce-absent`; the default shim ([`.claude/hooks/world-gate.sh`](../.claude/hooks/world-gate.sh)) does not pass it. |
| **Approval binding.** An `ASK` grant is an `AuthorizationInstance` bound to the exact action, arguments, resource, principal, world and manifest epoch, with an expiry and one use, consumed before the effect. ([D73](../DECISIONS.md#d73--ask-authorization-is-an-exact-durable-single-use-claim-made-before-effect)) | [`crates/trace-store/src/approval.rs`](../crates/trace-store/src/approval.rs) tests `mint_approve_then_consumed_once`, `principal_resource_and_expiry_are_enforced`, `concurrent_consumers_get_exactly_one_claim`, `manifest_drift_voids_the_approval`; [`crates/agent-core/src/lib.rs`](../crates/agent-core/src/lib.rs) `authorization_binds_the_kernel_effective_action`; benchmark scenario `approval-substitution-and-replay` in [`results/REPORT.md`](benchmarks/mcp-governance/results/REPORT.md). | **EU** 14(4)(d) (decide not to use, or override, in a given situation). **NIST** MAP 3.5. **ISO** A.9.2 (processes for responsible use). **OWASP** ASI09 Human-Agent Trust Exploitation, ASI03. | Used by the `agent-core` loop (`harness` REPL) and by the benchmark's reference host. The shipped REPL keeps the store in a per-run temporary directory, deleted when the session ends ([`main.rs`](../crates/cli-harness/src/main.rs), session storage comment). **On hosts, `ASK` goes to the host's own prompt and the person's answer is not recorded** (G2, [#138](https://github.com/sv-pro/ai2rules/issues/138) / AI2-65). The Gate ABI's `approval_token` is a correlation id, not a grant. |
| **Signed, chained approval and commit logs.** Every line carries an HMAC-SHA256; lines are chained and the tail is head-anchored, so a forged, edited, deleted or reordered line fails the whole load. ([D67](../DECISIONS.md#d67--the-approval-log-is-signed-and-an-approval-is-bound-to-the-policy-that-granted-it), [D78](../DECISIONS.md#d78--signed-logs-are-chained-and-head-anchored-a-missing-line-is-tampering-not-history)) | [`crates/trace-store/src/integrity.rs`](../crates/trace-store/src/integrity.rs) (HMAC with RFC 4231 known-answer tests); [`crates/trace-store/src/chain.rs`](../crates/trace-store/src/chain.rs) `a_forged_or_missing_head_is_refused`; [`approval.rs`](../crates/trace-store/src/approval.rs) `an_appended_forged_approval_is_refused`, `a_modified_line_is_refused`, `deleting_the_consumed_line_does_not_revive_the_grant`, `deleting_a_middle_line_breaks_the_chain`. | **EU** 12(1) (automatic recording of events), for the approval events only. **NIST** MEASURE 2.8. **ISO** A.6.2.8 (event logs). **OWASP** ASI03. | Covers the approval log and the D74 commit log, **not** the decision trace or the host decision log. The key sits beside the log, so it defends against write-without-read. A saved copy of an old head can roll the log back in a new process, pinned by `known_limit_a_matching_old_snapshot_fools_a_new_process`. No retention period is provided (EU 19(1) and 26(6) ask for at least six months; that is the deployer's storage). The key is read from `/dev/urandom`; these tests fail on native Windows today (AI2-57). |
| **Read-only control plane.** The agent can read the world that governs it but not write it; a policy change is a proposal merged by a deterministic non-agent tool. ([D57](../DECISIONS.md#d57--the-control-plane-is-read-only-to-the-agent-policy-changes-are-proposed-and-merged-only-by-deterministic-non-agent-tools)) | The rules `- { path: "./.claude", access: Read }` and `- { path: "./.agents", access: Read }` (D92, both hosts' control planes) in [`scripts/starter-world.yaml`](../scripts/starter-world.yaml), carried verbatim by both dogfood worlds (pinned by `crates/compiler/tests/committed_worlds.rs`); [`crates/cli-harness/tests/init.rs`](../crates/cli-harness/tests/init.rs) `the_agent_cannot_write_the_policy_that_governs_it`, `the_agent_can_still_read_the_policy`; machine scope: [`init_machine.rs`](../crates/cli-harness/tests/init_machine.rs) `the_user_world_protects_the_files_that_can_switch_it_off`. | **EU** 15(5). **NIST** GOVERN 1.4; MANAGE 4.1 (change management). **ISO** —. **OWASP** ASI10 Rogue Agents (an agent cannot rewrite its own limits). | Structured file tools get `DENY`. The **shell path is `ASK`, not `DENY`**: `sed -i`, `echo >` and the like against the same files reach a human prompt (D57, known residual). A person, or a user-scope `disableAllHooks`, can still switch the project or user hook off; only the managed scope (below) removes that. |
| **Replay and drift reports.** The same trace replayed against the same world reproduces every verdict; replayed against a changed world, the report lists each verdict that would move. For hosts, past Claude Code sessions can be replayed against a world before it is enforced. (PLAN E4) | Library: [`crates/trace-store/src/replay.rs`](../crates/trace-store/src/replay.rs) (`replay`, `drift_report`), [`bundle.rs`](../crates/trace-store/src/bundle.rs); tests in [`crates/trace-store/src/lib.rs`](../crates/trace-store/src/lib.rs) `replay_against_same_world_reproduces_every_decision`, `drift_report_flags_a_changed_verdict`, `bundle_round_trips_and_replays`; demo [`crates/trace-store/examples/trace_demo.rs`](../crates/trace-store/examples/trace_demo.rs). Hosts: `harness replay-transcripts` ([`replay_transcripts.rs`](../crates/cli-harness/src/replay_transcripts.rs), tests in [`tests/replay_transcripts.rs`](../crates/cli-harness/tests/replay_transcripts.rs)). | **EU** 9(6), 9(8) (testing before use); 72(2) (review of performance data after deployment). **NIST** MEASURE 2.4, MEASURE 3.1; MANAGE 4.1 (change management). **ISO** A.6.2.4 (verification and validation), A.6.2.6 (operation and monitoring). **OWASP** —. | Replays **decisions**, not outcomes: it shows what the kernel would answer, not what the tool did. `replay` and `drift_report` are a library API and a demo, not a CLI command over the REPL's trace (which is deleted with its temp directory). `replay-transcripts` reads path scope from today's filesystem, so deleted paths and cross-OS replays report `missing_path` (module header). |
| **Host decision log.** With `--log`, the Claude Code hook appends one JSON line per call: session, tool, call summary, verdict, rule, the answer given to the host, taint and manifest hash. The policy console reads it. | [`crates/cli-harness/src/cc_hook.rs`](../crates/cli-harness/src/cc_hook.rs) (`decision_line`, `call_summary`); tests in [`tests/cc_hook.rs`](../crates/cli-harness/tests/cc_hook.rs) `decision_log_records_every_verdict_with_its_answer_and_taint`, `decision_log_records_a_silent_passthrough_as_a_null_answer`, `decision_log_keeps_the_target_not_the_content`, `an_unwritable_decision_log_does_not_change_the_verdict`; wired by [`docs/demos/policy-console/setup.sh`](demos/policy-console/setup.sh). | **EU** 12(1), 12(2) (events relevant to monitoring); 14(4)(a) (monitor operation, detect anomalies); 26(5) (deployer monitoring). **NIST** MEASURE 2.4. **ISO** A.6.2.6, A.6.2.8. **OWASP** —. | **Off by default**: the shipped shim does not pass `--log`, and Antigravity's hook has no log yet. Plain JSONL: not signed, not chained, not redacted (the call summary keeps up to 160 characters of a command or URL, which can carry a secret). It records the verdict and the hook's answer, not the person's answer or the outcome (G2, #138). A log that cannot be written is reported on stderr and the call proceeds. |
| **Design decision record.** Every choice that closes off an alternative is logged with the alternatives and why, including known limits and retractions. | [`DECISIONS.md`](../DECISIONS.md) (D1–D82); the epic plan and acceptance invariants in [`PLAN.md`](../PLAN.md). | **NIST** GOVERN 4.2 (teams document risks and impacts). **ISO** A.6.2.3 (documentation of design and development). **EU** —, **OWASP** —. | A project record, not runtime evidence. It documents ai2rules' own design, not a deployment's risk assessment. |
| **Governance benchmark with a must-fail baseline.** Three scenarios, two targets, two transports; per-scenario PASS/FAIL with observed downstream effect counts, no aggregate score; CI fails if ai2rules fails *or* the deliberately weak baseline passes. ([D76](../DECISIONS.md#d76--a-governance-benchmark-publishes-per-scenario-evidence-and-asserts-its-baseline-still-fails)) | [`docs/benchmarks/mcp-governance/results/REPORT.md`](benchmarks/mcp-governance/results/REPORT.md) and [`results.json`](benchmarks/mcp-governance/results/results.json), regenerated by [`scripts/run-governance-bench.sh`](../scripts/run-governance-bench.sh) (CI job `governance-bench` in [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)); runner [`crates/govbench`](../crates/govbench/src/lib.rs), oracle test [`crates/govbench/tests/pack.rs`](../crates/govbench/tests/pack.rs) `a_correct_verdict_without_evidence_is_not_a_pass`. | **EU** 9(8) (testing against prior-defined metrics). **NIST** MEASURE 2.7; MEASURE 2.13 (effectiveness of the evaluation itself, which the must-fail baseline checks). **ISO** A.6.2.4. **OWASP** ASI03, ASI09. | A **seed of three MCP scenarios**, not coverage of a threat model. The ai2rules target is the kernel *plus the benchmark's reference host*, a consume-then-invoke boundary ai2rules does not ship as a command (report, Targets). It does not exercise the Claude Code or Antigravity hooks. |
| **Managed scope.** `harness init --managed` installs the hook in Claude Code's admin-only managed settings, with the kernel copied in and no user-side kill-switch or binary override. ([D82](../DECISIONS.md#d82--machine-wide-governance-a-user-scope-for-convenience-a-managed-scope-for-control)) | [`crates/cli-harness/src/init_machine.rs`](../crates/cli-harness/src/init_machine.rs); tests in [`tests/init_machine.rs`](../crates/cli-harness/tests/init_machine.rs) `managed_install_writes_policy_kernel_and_a_shim_without_escape_hatches`, `the_managed_shim_ignores_every_user_side_escape_hatch`; runbook and limits in [`machine-wide-governance.md`](machine-wide-governance.md). | **EU** 26(1) (deployer's technical and organisational measures). **NIST** GOVERN 1.4. **ISO** A.9.2. **OWASP** —. | Not the default: `harness init` governs one folder, `--user` can be switched off by any project's settings. Managed scope binds Claude Code only, not other agents or direct API use. Taint state stays user-writable; a missing kernel fails open; local administrators can edit managed settings; whether `claude --bare` skips managed hooks is unverified ([limits section](machine-wide-governance.md#what-the-managed-scope-still-does-not-stop)). |
| **Secret redaction.** Before a decision record exists, its arguments are masked by the world's `observability.redact` patterns and by a value scanner for issuer-shaped tokens, auth headers, URL passwords and private keys. ([D66](../DECISIONS.md#d66--the-audit-log-scans-values-for-secrets-and-refuses-to-guess)) | [`crates/trace-store/src/secrets.rs`](../crates/trace-store/src/secrets.rs) tests `issuer_prefixed_tokens_are_masked_in_a_command`, `a_password_in_a_url_is_masked_and_the_user_is_kept`, `ordinary_values_are_left_alone`; [`redact.rs`](../crates/trace-store/src/redact.rs); [`crates/trace-store/src/lib.rs`](../crates/trace-store/src/lib.rs) `store_redacts_secrets_before_disk`. | **NIST** MEASURE 2.10 (privacy risk examined and documented). **EU** —, **ISO** —, **OWASP** —. | Applies to `trace-store` records (`agent-core`). **The host decision log is not redacted** (row above). The scanner knows a fixed set of shapes by design; a secret in another shape passes unless the world names its field. |
| **Budgets.** Per-session counters for commands, network calls, file writes and tokens travel with each request; a world that declares a counted limit refuses a request that omits them. ([D64](../DECISIONS.md#d64--budget-counters-are-carried-session-state-and-a-world-that-counts-calls-requires-them)) | [`crates/world-kernel/src/disposition.rs`](../crates/world-kernel/src/disposition.rs) (`charge` beside `budget_exceeded`); [`crates/harness-preview/src/gate.rs`](../crates/harness-preview/src/gate.rs) tests `an_allowed_call_charges_the_budget_it_consumes`, `carried_usage_at_the_limit_denies`, `a_budgeted_world_refuses_a_request_with_no_counters`; [`world-kernel/src/lib.rs`](../crates/world-kernel/src/lib.rs) `over_command_budget_replans`. | **EU** 15(4) (resilience to errors and faults). **NIST** MEASURE 2.6 (fails safely). **ISO** —. **OWASP** ASI08 Cascading Failures. | Over budget is `REPLAN`. Claude Code and Antigravity have no replan channel, so the hooks **fall through to the host's own permission flow rather than deny** (D64, known limit). Tokens are not charged at the gate. Counters on a host sit in a user-writable sidecar, like taint. |

---

## Following one row, end to end

An assessor holding EU AI Act Article 12(1) ("automatic recording of events") and
wanting to see what ai2rules records about human approvals:

1. Find **12(1)** in the *Evidence for* column: the signed-logs row (and the host
   decision log, which is off by default).
2. Read the limit first: approval events only, no retention period, key beside the log.
3. Open the decision, [D78](../DECISIONS.md#d78--signed-logs-are-chained-and-head-anchored-a-missing-line-is-tampering-not-history),
   for the design and its alternatives.
4. Open the code, [`crates/trace-store/src/approval.rs`](../crates/trace-store/src/approval.rs),
   and find the test `deleting_the_consumed_line_does_not_revive_the_grant`.
5. Run it:

   ```bash
   cargo test --offline -p trace-store deleting_the_consumed_line_does_not_revive_the_grant
   ```

The same path works for every row: clause → row → limit → decision → file → test.

## Obligations with no row

Listed so a gap is not mistaken for coverage:

- **EU 14(4)(e), stop the system safely; NIST MANAGE 2.4, disengage or deactivate.** The
  policy console ([D81](../DECISIONS.md#d81--the-policy-console-writes-the-world-file-the-hook-it-already-reads-does-the-rest),
  [`docs/demos/policy-console/`](demos/policy-console/README.md)) switches actions off
  at the next call, with `--enforce-absent`. A single Stop position is open work (G7,
  [#136](https://github.com/sv-pro/ai2rules/issues/136)). Killing a call already running
  is E8.4, open.
- **EU 19(1) and 26(6), keep logs at least six months.** ai2rules writes logs; it does
  not retain them. Where they live and for how long is the deployer's choice.
- **Outcome records.** No artifact links a decision to the effect it allowed on a host.
  Execution receipts exist only in the D74 prototype ([`staged-effect-commit.md`](staged-effect-commit.md)).
- **Fail-closed on a broken world.** The hooks fail open (#128 / AI2-56), which an
  assessor reading EU 15(4) or NIST MEASURE 2.6 should weigh against every host row.
- **Data governance (ISO A.7 apart from provenance), bias, and model evaluation.** Out of scope: ai2rules governs actions, not models or training data.

## Sources

Checked on 2026-10-23. Clause numbers on this page come from these, not from memory.

- **EU AI Act**: Regulation (EU) 2024/1689 of the European Parliament and of the
  Council of 13 June 2024 laying down harmonised rules on artificial intelligence (OJ L,
  12.7.2024), [EUR-Lex, ELI](https://eur-lex.europa.eu/eli/reg/2024/1689/oj).
  The article texts were read on the European Commission's
  [AI Act Service Desk](https://ai-act-service-desk.ec.europa.eu/en/ai-act/article-12),
  which reproduces the EUR-Lex consolidated text and marks Digital Omnibus changes:
  Articles [9](https://ai-act-service-desk.ec.europa.eu/en/ai-act/article-9),
  [12](https://ai-act-service-desk.ec.europa.eu/en/ai-act/article-12),
  [14](https://ai-act-service-desk.ec.europa.eu/en/ai-act/article-14),
  [15](https://ai-act-service-desk.ec.europa.eu/en/ai-act/article-15),
  [19](https://ai-act-service-desk.ec.europa.eu/en/ai-act/article-19),
  [26](https://ai-act-service-desk.ec.europa.eu/en/ai-act/article-26),
  [72](https://ai-act-service-desk.ec.europa.eu/en/ai-act/article-72). None of the
  paragraphs cited carried an Omnibus mark on that date.
- **NIST AI RMF**: *Artificial Intelligence Risk Management Framework (AI RMF 1.0)*,
  NIST AI 100-1, January 2023, [doi:10.6028/NIST.AI.100-1](https://doi.org/10.6028/NIST.AI.100-1),
  Tables 1–4. Subcategory texts were read from the PDF.
- **ISO/IEC 42001:2023**, *Information technology — Artificial intelligence —
  Management system*, [iso.org/standard/81230](https://www.iso.org/standard/81230.html).
  **The text is paywalled and was not read.** The Annex A control numbers and titles
  used here come from public summaries: A.6.2.4, A.6.2.6, A.6.2.8, A.7.5, A.9.2 and
  A.9.4 agree across [ISMS.online](https://www.isms.online/iso-42001/annex-a-controls/a-6-ai-system-life-cycle/)
  ([A.9](https://www.isms.online/iso-42001/annex-a-controls/a-9-use-of-ai-systems/)) and
  [blckalpaca](https://blckalpaca.at/en/knowledge-base/ai-agents/iso-42001-ai-management-system/annex-a-controls-iso-42001);
  A.6.2.3 is from ISMS.online alone. Check them against a licensed copy before relying
  on them.
- **OWASP Top 10 for Agentic Applications for 2026**, OWASP GenAI Security Project,
  published 9 December 2025, [genai.owasp.org](https://genai.owasp.org/resource/owasp-top-10-for-agentic-applications-for-2026/).
  Entry names as given in OWASP's [announcement](https://genai.owasp.org/2025/12/09/owasp-top-10-for-agentic-applications-the-benchmark-for-agentic-security-in-the-age-of-autonomous-ai/).

Gap G8 of the *Designing AI Agents* gap review (book ch. 9 §9.7, pp. 390–391) asked for
this page.
