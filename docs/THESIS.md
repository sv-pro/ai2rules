# Agentic Governance at the Stochastic–Deterministic Border

*The intellectual spine that unifies the fragments. This is the "one project"
behind the harness, `context-engine`, `cedar-world-playground`, the IntentOS
crates, and the serving substrate. Everything else — the monorepo, the paper, the
blog series — derives from this document.*

Status: **positioning draft** (v0.1, 2026-06-26). Maturity of each fragment is
called out honestly in §7; do not read this as "all shipped."

---

## 1. The thesis (one sentence)

> **As we hand agents real authority, safety and trust cannot come from a smarter
> LLM watching a dumber one. They come from compiling stochastic intent into a
> deterministic, auditable substrate — at every layer: action, capability,
> knowledge, and intent.**

The category is **Agentic Governance**: governing what an autonomous agent may
perceive, represent, and execute. The mechanism is **the stochastic–deterministic
border**: *design-time stochastic, runtime deterministic.* The LLM proposes; a
deterministic kernel disposes. **No LLM ever sits in the trust path.**

## 2. The problem: Agentic Governance

A local agent (Claude Code, Codex CLI, Gemini CLI, Aider, …) inherits the
developer's **full ambient authority** — workspace write access, git remotes,
package managers, shell, SSH keys, cloud credentials, MCP servers, persistent
memory — while ingesting **untrusted text on every turn**: file contents, issue
bodies, dependency metadata, web pages, command output, MCP results.

So prompt injection is not a prompt problem. It is an **authority-boundary
problem**. The dominant defense — put a second LLM in front to ask *"is this
malicious?"* — is a losing battle: if your security depends on a stochastic
classifier being right 100% of the time, you are already compromised. (See
[`blog/.../ai-aikido.md`](../blog/src/content/blog/ai-aikido.md) and
[`why-deny-is-dangerous.md`](../blog/src/content/blog/why-deny-is-dangerous.md).)

The same problem recurs above the action layer. An agent that *retrieves* from a
knowledge base can be poisoned by a malicious document; an agent that *remembers*
can have its memory corrupted; an agent that *plans* can be redirected. Each is an
authority/trust boundary. **Agentic Governance is the discipline of drawing those
boundaries structurally instead of asking an LLM to police them.**

## 3. The mechanism: the stochastic–deterministic border

One move, stated once:

```
DESIGN / INGESTION TIME (stochastic)        RUNTIME (deterministic)
────────────────────────────────────        ────────────────────────────────
LLM drafts world manifests             →     compiled, hash-addressed world
LLM distills prose into facts/rules    →     provenanced, versioned store
LLM proposes a typed intent            →     table lookup · schema · taint join
LLM extracts an open knowledge graph   →     Personalized PageRank retrieval
            (creativity, synthesis)                 (replayable, auditable)
```

The border has three properties that make it trustworthy, all inherited from the
source repos (§5):

1. **The deterministic side is a pure function** of `(intent, context, compiled
   world)`. Same inputs → same decision → fully replayable.
2. **Absence, not refusal, is the primary control.** Dangerous capabilities are
   not blocked by a rule the model can argue with; they *do not exist* in the
   agent's world. `ABSENT ≠ DENY`.
3. **Trust is monotonic and provenanced.** Taint can only join and increase;
   every value carries origin, trust, and lineage; nothing untrusted silently
   becomes authoritative.

## 4. Five layers, one move

Every fragment is the same border move applied to a different governed resource.

### 4.1 Action — *what the agent does*
- **Fragment:** the `cli-agent` harness (`world-kernel`, `compiler`, executor).
- **Border move:** `ToolCall → sealed IntentIR → ExecutionSpec`. An `IntentIR` is
  unforgeable outside the kernel; its existence is a validity witness. A
  `WorldManifest` (LLM-drafted, human-reviewed) compiles to an immutable
  `CompiledWorld`; runtime enforcement is table lookup, schema validation, hash
  verification, and typed state machines. Only `ExecutionSpec` crosses into real
  execution.
- **Status:** most mature. M1–M3 complete; M4 hardening + M5 in-browser WASM kernel
  in progress. This is the flagship reference implementation of the border.

### 4.2 Capability — *what world exists for an actor*
- **Fragment:** `cedar-world-playground`.
- **Border move:** policy + world definition → deterministic **projection** of a
  per-agent capability surface, resolving to `ALLOW / DENY / ABSENT / SIMULATE`.
  Asks "what executable world exists for this actor?" rather than "can this actor
  do this action?"
- **Status:** conceptual + executable demo (TS). The clearest *illustration* of
  the harness's `CompiledWorld` idea, and the origin of the `ABSENT≠DENY`
  vocabulary.

### 4.3 Knowledge — *what the agent believes*
- **Fragments:** `context-engine` (governance) + HippoRAG-2-style retrieval
  (recall). See [`papers/hipporag2-vs-context-engine.md`](papers/hipporag2-vs-context-engine.md).
- **Border move:** an LLM distills prose into typed **Facts / Rules / Capsules**
  at ingestion; retrieval is deterministic and **governed** — first-class
  provenance (`doc_id` + line span), a `status: active/deprecated/invalid`
  lifecycle, and **gated promotion** (investigation → fact/rule only under
  human-approval / multi-source / repeated-evidence policy). Rules are normative,
  executable objects; capsules are non-normative navigation only. Principle:
  *correctness > completeness.*
- **Status:** `context-engine` is fairly mature (Postgres/pgvector/DSPy). HippoRAG
  2 is external, peer-reviewed evidence that the recall core works; context-engine
  is the governance shell on top. **These are separable layers** — governed recall
  = the memory analog of what the harness does for actions.

### 4.4 Intent — *what the agent wants*
- **Fragments:** `intent-memory-engine` / `intentos-core` (Rust).
- **Border move:** "AI Aikido" — use stochastic embeddings for *navigation* but
  keep the core runtime deterministic: a traversable **Triad graph**
  (`Task | Context | Outcome`), never an LLM call per query. `intentos-core`'s
  `invariant_guard` extends this to deterministic **Rule/Finding** checking over a
  code graph — the same "rules as auditable objects" stance as the knowledge layer.
- **Status:** early. `intent-memory-engine` is a working library; `intentos-core`
  is V0.1 (retrieval still placeholder). Conceptually load-bearing, not yet proven.

### 4.5 Substrate — *what it all runs on*
- **Fragments:** `llm-service-stack` / `personal-llm-box`.
- **Border move:** the owned, local, cost-tracked serving plane (LiteLLM gateway,
  Ollama, caching) — the box the border runs inside. Keeps the *stochastic* side
  self-hosted and observable so the deterministic side can be trusted end-to-end.
- **Status:** working infra. **Peripheral to the thesis** — supporting cast, not a
  governance layer. Included for completeness; a candidate to keep out of the core
  narrative.

## 5. Shared primitives

The border is built from a small kit of primitives, distilled from the source
reference repos (`docs/harness-architecture.md` §2) and reused at every layer:

| Primitive | From | Meaning |
|---|---|---|
| **Closed action ontology** | `agent-hypervisor` | absent actions don't exist, aren't merely forbidden |
| **Sealed intent** | `agent-hypervisor` | execution intent is unforgeable outside the kernel |
| **Monotonic taint** | `agent-hypervisor` | taint joins and increases, never silently drops |
| **Provenance on values** | `agent-hypervisor` | origin, trust, lineage, content identity on every value |
| **Process boundary** | `agent-hypervisor` | the policy layer owns no direct handler callables |
| **Trace replay** | `agent-hypervisor` | every decision reproducible from recorded inputs + world version |
| **ABSENT ≠ DENY** | `safe-mcp-proxy` | ontology outcomes ≠ policy outcomes (behavior + audit meaning differ) |
| **Three decision stages** | `safe-mcp-proxy` | ontology → policy → effect virtualization |
| **Descriptor drift detection** | `safe-mcp-proxy` | hash descriptors at projection; block on runtime change |
| **Capability projection** | `mcp-tool-projection` | a tool surface is *projected* per actor, not exposed wholesale |

The claim of coherence is exactly this: **the same primitive kit governs actions,
capabilities, knowledge, and intent.** That's what makes five fragments one project.

## 6. The Flywheel (the method)

The project is produced by a self-feeding loop (`docs/FLYWHEEL.md`):

- **Discovery (radar)** — an agent (dogfooding the harness) scans arXiv `cs.CR`,
  HN, and security RSS for new injection / sandbox / capability attacks.
- **Development (engine)** — each real threat becomes a failing test, then a patch
  to the manifest / taint engine / execution boundary that neutralizes it.
- **Advocacy (megaphone)** — the fix becomes a blog post + VHS demo (attack
  succeeds without the harness, `DENY`/`ABSENT` with it); reader objections feed
  back into Discovery.

The method *is* part of the thesis: governance research is only credible if it is
driven by real attacks and demonstrated, not asserted.

## 7. Roadmap to consolidation

Honest current state: this is a **synthesis of fragments at very different
maturities**, not a shipped monorepo. Consolidation, smallest-leverage-first:

1. **Adopt this doc as the spine.** Link it from the harness `README.md` and
   reference it from each fragment's top-level doc so the five layers point home.
2. **Normalize vocabulary across fragments.** *(First cut done:
   [`GLOSSARY.md`](GLOSSARY.md).)* Make `ABSENT/DENY/SIMULATE`, taint, provenance,
   and "design-time stochastic / runtime deterministic" mean the same thing in
   every repo's docs. Next: reference the glossary from each fragment's top doc.
3. **Pick the umbrella form** — meta-repo with submodules vs. a docs-only umbrella
   site vs. a Cargo/workspace consolidation of the Rust pieces. (Decision deferred;
   record in `DECISIONS.md` when taken.)
4. **Prove the cross-layer claim.** *(First cut done.)*
   `cargo run -p agent-core --example poisoned_knowledge_demo` shows a poisoned
   *document* (knowledge layer, retrieved as a tainted MCP result) failing to
   escalate into a forbidden *action* (action layer): the taint floor flips an
   *identical* `fetch_web` from `ALLOW` (baseline session) to `DENY` (session that
   retrieved first), proving it is provenance — not content — doing the work. Next:
   wire the *real* `context-engine` retriever behind the MCP transport (replacing
   the mock) so the document is genuinely distilled, not scripted.
5. **Publish the thesis** — blog series first (Flywheel), paper second (HippoRAG-2
   / Cedar / capability-security as related work; harness + governed memory as the
   system; injection/taint benchmarks as evidence).

## 8. Related work (the two that occupy this ground)

Two external projects land directly on this thesis's territory and sharpen it by
contrast. (Full survey + adoption verdicts for all of `repos/3p`:
[`THIRD-PARTY-ADOPTION.md`](THIRD-PARTY-ADOPTION.md).)

**Microsoft's Agent Governance Toolkit (AGT)** shares the *goal* almost verbatim —
deny-by-construction, "the difference between asking an agent to behave and making it
incapable of misbehaving" — but reaches it by **in-process policy middleware**: a
`default_action: allow` engine evaluating deny rules, with (its own SECURITY note) the
policy engine and agents on the *same* process boundary. That is governance by *policy
decision*. The border governs by **structure instead of decision**: the dangerous
capability is `ABSENT` — it does not exist in the compiled world (§3, *absence not
refusal*), not denied by a rule a model can argue with; trust is **monotonic and
provenanced** (§3, *trust is monotonic*), never silently downgraded; and the policy
layer **owns no handler callables** (§5, process boundary). AGT's MCP Security Gateway (tool poisoning, descriptor drift) parallels
`safe-mcp-proxy`, and its OWASP-Agentic-Top-10 / red-team corpus is direct Flywheel
input — we treat AGT as the strongest incumbent to *position against*, not adopt
(`DECISIONS.md` D27). *(Note: AGT ships a package named "Agent Hypervisor," distinct
from our source `repos/agent-hypervisor`.)*

**HKUDS's Memory Governance Protocol (MGP)** is the knowledge-layer analog: it
standardizes governed memory as a protocol — a full lifecycle
(`Write → … → Revoke → Purge`), per-request policy context ("who acts, for whom, under
what constraints"), and a queryable audit trail, explicitly *peer to MCP*. MGP
standardizes the **interface** to governed memory; the border's knowledge layer (§4.3)
adds the move MGP leaves open — the stochastic→deterministic **distillation** of prose
into typed Facts / Rules / Capsules at ingestion, with deterministic governed recall.
The two **compose**: MGP as the wire contract, the distillation border as what sits
behind it. We therefore align vocabulary to MGP and defer adopting its runtime until a
second consumer exists (`DECISIONS.md` D28).

---

*Companion docs: [`PRINCIPLES.md`](PRINCIPLES.md) (forced vs arbitrary + a
trial/proof/reject program), [`GLOSSARY.md`](GLOSSARY.md) (normalized vocabulary),
[`harness-architecture.md`](harness-architecture.md) (action layer, canonical),
[`papers/hipporag2-vs-context-engine.md`](papers/hipporag2-vs-context-engine.md)
(knowledge layer), [`FLYWHEEL.md`](FLYWHEEL.md) (method),
[`THIRD-PARTY-ADOPTION.md`](THIRD-PARTY-ADOPTION.md) (what to do with `repos/3p`),
`repos/*/` (the fragments).
Cross-layer proof: `agent-core/examples/poisoned_knowledge_demo`.*
