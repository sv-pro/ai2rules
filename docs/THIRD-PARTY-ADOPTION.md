# Third-Party Adoption Plan (`repos/3p`)

How the project should relate to the 20 external repos parked under `repos/3p`:
what to **adopt** (use as a tool in our own loop), what to **incorporate** (pull
ideas / a protocol / code into harness paths), what to **publish against** (use as
related-work and Flywheel fuel), and what to **ignore**. Framed against the five
layers of [`THESIS.md`](THESIS.md) (Action · Capability · Knowledge · Intent ·
Substrate). The two repos that occupy our ground get their own decisions —
[`DECISIONS.md`](../DECISIONS.md) **D27** (Agent Governance Toolkit) and **D28**
(MGP) — and a related-work section in [`THESIS.md`](THESIS.md) §8.

Status: **plan v0.1 (2026-06-27).** Maturities differ wildly; nothing here is
"shipped." Each item names effort and a caveat honestly.

---

## How to read this

Every repo gets one of four **dispositions**:

| Disposition | Meaning | The user's verb |
|---|---|---|
| **Adopt-as-tool** | Run it in our own dev/dogfooding loop; no code merge | *adoption* |
| **Incorporate** | Pull ideas / a protocol / (MIT/Apache) code into harness paths | *incorporating* |
| **Position-against** | Use as related-work + Flywheel discovery corpus | *publishing* |
| **Ignore** | Reference only; no action | — |

**Hard constraints (don't regress):**

- **Never `git add repos/`** — `AGENTS.md` forbids committing the reference repos;
  `repos/3p/*` are *not* Cargo workspace members. Incorporation means *copying an
  idea or a permissively-licensed snippet into a harness path with attribution*,
  never a submodule or path-dependency on `repos/3p`.
- **`OpenViking` is AGPLv3 — ideas only.** Never vendor or link its code. Every
  other repo below is MIT or Apache-2.0 (incorporation-safe with an attribution
  line); see the licensing table at the end.
- **Decisions outrank code.** Anything that touches the kernel/manifest contract
  goes through a `D<n>` entry first (per `maintain-decisions-log`).

---

## Snapshot

| Repo | License | Layer | Disposition | First action |
|---|---|---|---|---|
| **agent-governance-toolkit** (MS) | MIT | Action/Capability | Position-against | D27 + THESIS §8; mine OWASP corpus for Flywheel |
| **MGP** (HKUDS) | MIT | Knowledge/Intent | Position-against + Incorporate | D28; align GLOSSARY vocabulary |
| **OpenSandbox** (Alibaba) | Apache-2.0 | Substrate/Executor | Incorporate (spike) | E8 spike: back `ExecutionSpec` with it |
| **icm** (rtk-ai) | Apache-2.0 | Knowledge/Intent | Incorporate (ref impl) | Study feedback-loop for intent layer |
| **codegraph** | MIT | Knowledge (illustration) | Adopt-as-tool | Wire MCP into our dev loop |
| **rtk** (rtk-ai) | Apache-2.0 | Tooling | Adopt-as-tool | Use as token proxy in sessions |
| **agent-skills** (addyosmani) | MIT | Method | Adopt-as-tool | Lifecycle skills for our dev |
| **andrej-karpathy-skills** | *(none)* | Method | Incorporate (ideas) | Fold 4 principles into AGENTS.md |
| **mattpocock-skills** | MIT | Method | Incorporate (ideas) | Borrow user/model-invoked split |
| **agentscope** (Alibaba) | Apache-2.0 | Capability | Incorporate (ideas) | Permission-system + sandbox patterns |
| **agentmemory** | Apache-2.0 | Knowledge | Incorporate (ideas) | 4-tier consolidation, `governance_delete` |
| **OpenViking** (Volcengine) | **AGPLv3** | Knowledge | Incorporate (ideas only) | Filesystem-paradigm + trajectory observability |
| **ragflow** (InfiniFlow) | Apache-2.0 | Knowledge | Position-against | Recall benchmark / related-work only |
| **claude-context** (Zilliz) | MIT | Knowledge | Adopt-as-tool (optional) | Heavier alt to codegraph; needs Milvus |
| **ruflo** (ruvnet) | MIT | Action/Method | Position-against (skeptical) | Mine governance-plane + harness-threat-model idea |
| **hermes-agent** (Nous) | MIT | Intent/Method | Ignore (borrow loop idea) | Self-improving loop as intent-layer reference |
| **deer-flow** (ByteDance) | MIT | Substrate | Ignore | Architecture reference |
| **openclaw** | MIT | Substrate | Ignore (borrow conventions) | AGENTS.md review-discipline exemplar |
| **google-skills** | Apache-2.0 | Method | Ignore (unless GCP) | Only if Substrate runs on GCP |
| **claw-code** | MIT | — | Ignore | Self-described "museum exhibit" |

---

## The plans

### A. Position-against (publishing / Flywheel)

**`agent-governance-toolkit` (AGT) — the strongest incumbent.** Microsoft, MIT, 992
conformance tests, and *our headline almost verbatim* ("incapable of misbehaving,"
not "ask it to behave"). But it enforces with **in-process policy middleware**
(`default_action: allow` + deny rules; policy engine and agents share one process
boundary — its own SECURITY note). That is governance by *policy decision*; the
border governs by *ontology + taint + process boundary* (`ABSENT≠DENY`, monotonic
provenanced taint, no handler callables in the policy layer). Actions:
1. **D27** records the positioning (done alongside this plan).
2. **THESIS §8** states the contrast in paper-grade prose (done).
3. **Flywheel:** ingest AGT's OWASP-Agentic-Top-10 mapping + PromptDefense 12-vector
   + `agt red-team scan` as a *discovery* corpus — each becomes a failing test then a
   manifest/taint patch (`FLYWHEEL.md` §Discovery).
4. **Blog candidate:** "Why a deny-rule isn't a boundary — ontology + taint vs
   policy-middleware," AGT as the foil. Add to `docs/BLOG_PLAN.md`.
- *Caveat:* AGT ships a package literally named **Agent Hypervisor** — collides by
  name with our source `repos/agent-hypervisor` (different thing). Disambiguate in
  public writing.

**`MGP` — the knowledge-layer protocol.** See D28 + THESIS §8. Dual disposition: it
is both the related-work anchor for governed memory *and* a vocabulary to
incorporate (below).

**`ruflo` — mine one idea, skeptically.** Massive kitchen-sink (314 MCP tools); its
own `CLAUDE.md` flags many perf claims as *unverified*. Worth exactly one borrowed
idea: its **MetaHarness `score`/`genome`/`threat-model`** framing — a harness scoring
itself for governance readiness — is a plausible Flywheel self-audit tool. Do **not**
adopt the runtime. (D24 already rejected `agent-harness-generator`/MetaHarness as a
*foundation*; this is narrower — borrow the self-audit notion only.)

### B. Incorporate (ideas / protocol / code into harness paths)

**`OpenSandbox` — candidate backend for `ExecutionSpec` (E8).** Our executor stops at
a structured spec; OpenSandbox supplies the missing physics: gVisor/Kata/Firecracker
isolation, per-sandbox egress controls, a credential vault (inject secrets without
exposing them to the workload). This is the most credible "what runs the spec."
- **Action:** an E8 spike — can `ExecutionSpec` lower onto OpenSandbox's sandbox API,
  and does its egress-allowlist subsume the D21 tinyproxy floor? Effort: medium.
  Decision gate before any dependency (it's a large platform; we may borrow the
  *egress + credential-vault patterns* rather than the whole thing).

**`icm` — Rust reference for the Intent layer's closed loop.** Single-binary,
MCP-native, `memories` (temporal decay) + `memoirs` (typed knowledge graph) +
**`feedback`** (record a correction, search past mistakes before predicting). That
feedback loop is exactly the closed loop THESIS §4.4 wants and `intent-memory-engine`
lacks. **Action:** study `icm`'s feedback store + hybrid-retrieval as a reference
implementation; Apache-2.0 means we may lift specific code with attribution. Effort:
small (read) → medium (port the feedback pattern).

**`MGP` — align the Knowledge layer's external vocabulary.** Per D28: align
`GLOSSARY.md` + context-engine's *surface* to MGP's lifecycle
(`Write→…→Revoke→Purge`) and per-request policy context; defer speaking the protocol
on the wire until a second consumer exists. Effort: small (vocabulary), gated (runtime).

**`andrej-karpathy-skills` + `mattpocock-skills` — into our own agent discipline.**
Karpathy's four principles (think-first, simplicity, surgical changes, goal-driven)
paraphrase cleanly into `AGENTS.md` (no LICENSE on that repo — paraphrase the ideas,
which are Karpathy's public post; don't copy the file). mattpocock's **user-invoked
vs model-invoked skill split** is a clean model for our own `skills/`. Effort: small.

**`agentscope` / `agentmemory` — idea mining (no integration).** agentscope's
fine-grained **permission system** + pluggable sandbox backends inform the Capability
layer; agentmemory's **4-tier consolidation** (working→episodic→semantic→procedural),
RRF hybrid search, and a first-class **`governance_delete`** tool inform the Knowledge
layer. Read, extract, don't depend. Effort: small.

**`OpenViking` — ideas only (AGPLv3).** Two ideas are genuinely border-shaped: the
**filesystem paradigm** for unifying memories/resources/skills (a deterministic,
inspectable namespace instead of an opaque vector blob), and **visualized retrieval
trajectory** (retrieval as an auditable path, not a black box) — that is the
knowledge-layer analog of our trace-replay. **Never vendor the code** (AGPL); transcribe
the design only.

### C. Adopt-as-tool (our own loop + dogfooding)

**`rtk` — token proxy, now.** Rust, offline, <10ms, 60–90% output reduction. Zero
thesis risk, immediate cost win, and it *is* the stochastic→deterministic posture
(deterministic filter in front of the model). Adopt in our own sessions. Effort:
tiny.

**`codegraph` — deterministic code graph, now.** tree-sitter→SQLite graph over MCP,
**extraction is deterministic (AST, not LLM)** — a clean illustration of the border
applied to *code knowledge*. Doubles as a dev tool and a demo asset. Its `CLAUDE.md`
("adapt the tool to the agent, don't try to change the agent") is worth internalizing.
Effort: tiny (wire MCP) — high payoff.

**`agent-skills` (addyosmani) — lifecycle skills.** `/spec /plan /build /test /review
/ship` map onto how we already work and feed the Flywheel's Development arm. Adopt
selectively. Effort: tiny.

**`claude-context` — optional, heavier.** Semantic code search but needs an external
vector DB (Milvus/Zilliz). codegraph is the lighter local-first choice; keep this in
reserve only if we outgrow codegraph. Effort: small, deferred.

### D. Ignore (reference only)

`ragflow` (mature GraphRAG — a *recall benchmark* and related-work cite, too heavy to
integrate), `deer-flow` (LangGraph super-harness — architecture reference),
`hermes-agent` (borrow only the self-improving-loop *idea* for the Intent layer),
`openclaw` (product; its `AGENTS.md` review discipline is an exemplar to skim),
`google-skills` (GCP-specific — only if the Substrate lands on GCP), `claw-code`
(self-described museum exhibit).

---

## Sequenced roadmap

**Wave 0 — now, zero-risk (this change set + immediate adopts):**
- D27 + D28 + THESIS §8 (this change).
- Adopt `rtk` and `codegraph` into the dev loop.
- Add the AGT-vs-policy-middleware post to `docs/BLOG_PLAN.md`.
- README/PLAN pointer to this doc on commit (per `update-readme-on-commit`).

**Wave 1 — low-cost incorporation:**
- Fold Karpathy + mattpocock skill ideas into `AGENTS.md` / `skills/`.
- Align `GLOSSARY.md` knowledge-layer terms to MGP (D28).
- Stand up the AGT OWASP/PromptDefense corpus as Flywheel discovery inputs.

**Wave 2 — spikes (decision-gated):**
- `OpenSandbox` as the `ExecutionSpec`/E8 backend (or borrow its egress +
  credential-vault patterns); record the outcome as a `D<n>`.
- Port `icm`'s feedback loop as the Intent layer's closed-loop reference.

**Wave 3 — publish:**
- Related-work section in the paper (AGT + MGP + ragflow/HippoRAG-2 + Cedar).
- Blog: border vs policy-middleware (AGT); governed memory + distillation (MGP).

---

## Licensing & hygiene

| License | Repos | Incorporation rule |
|---|---|---|
| MIT | agent-governance-toolkit, MGP, codegraph, claude-context, agent-skills, mattpocock-skills, hermes-agent, deer-flow, ruflo, openclaw, claw-code | Copy with an attribution line + license notice |
| Apache-2.0 | OpenSandbox, icm, rtk, ragflow, agentscope, agentmemory, google-skills | Copy with attribution + NOTICE; patent grant is a plus |
| **AGPLv3** | **OpenViking** | **Ideas only — never vendor or link code** |
| none | andrej-karpathy-skills | Paraphrase the (public) ideas; don't copy the file |

- Attribution lands in the file header where code/ideas are lifted, and (for any
  vendored snippet) a `THIRD_PARTY_NOTICES`-style note.
- Reconfirm a repo's license at the moment of incorporation — these were read on
  2026-06-27 and upstreams relicense.

## Cross-references

- [`DECISIONS.md`](../DECISIONS.md) D27 (AGT positioning), D28 (MGP interop), D23
  (the five-layer thesis), D24 (host-neutral ABI — why we don't adopt a generator).
- [`THESIS.md`](THESIS.md) §8 (related work), §4 (the five layers), §5 (shared
  primitives).
- [`FLYWHEEL.md`](FLYWHEEL.md) (Discovery → Development → Advocacy), [`BLOG_PLAN.md`](BLOG_PLAN.md).
