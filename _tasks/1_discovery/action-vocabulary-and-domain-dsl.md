# Discovery: a vocabulary of verbs instead of shell, extended through a domain language

**Opened 2026-10-21 as an idea; not yet designed.** It is recorded here so it can be
developed. The base of it is already in the architecture. What is new is how the
vocabulary grows without a human reviewing every generated script.

## The idea

Do not give the agent a broad instrument (shell, `curl`, an unrestricted API) and then try
to police what it does with it. Give it a **vocabulary of actions**: named verbs that
translate to fixed commands, scripts or tool calls. This is MCP-shaped and not new in
itself.

The objection writes itself: it limits what the agent can do. The answer has to be a way
to **extend the vocabulary**, by the user or by the agent itself, which raises the real
question: **who reviews the new scripts**, when most users are not programmers? Taken
further, every domain could ship its own **domain-specific language** as a plugin: agents
write programs in it, but those programs cannot do just anything.

## What already exists

- `docs/harness-architecture.md` §7: "Start with a small ontology. Avoid raw shell as a
  model-visible capability." It lists constrained verbs (`run_tests`, `git_status`,
  `git_diff`, `cargo_check`).
- `docs/harness-architecture-antigravity.md`: "Command DSL Compilation (Verbs: git, test,
  view, edit)". One line on a layer diagram, not developed.
- `DECISIONS.md` (the March-cluster supersession): `safe-agent-runtime-pro` ("typed models /
  capability DSL / presets") was folded into the manifest schema, and
  `agent-world-compiler-poc` ("least privilege from observed execution") was retired as
  spent.
- **Built:**
  - scoped capabilities: `run_tests` always runs `pytest`, with locked arguments stripped
    and literals injected (invariant 12);
  - D79: renamed and limited arguments, hidden base actions, `mcp_surface`;
  - `harness mcp-author`: a page where a **non-programmer** builds a vocabulary from an MCP
    server's tools by picking, renaming and limiting them, with no code.

## The key move: the language reviews, not a person

Make safety a property of the **language**, not of reviewing each program. The precedent
is eBPF: Linux loads programs nobody has read, because a verifier proves in advance that
they cannot do anything outside their contract. Review shifts from "read every script" to
"design the language once".

## The ladder

1. **Fixed vocabulary.** Exists: scoped capabilities, D79.
2. **Extension without code.** Exists: `mcp-author`. The user composes verbs from existing
   tools through a form. The "review" is a table of what is allowed, readable by anyone.
3. **Composition (recipes).** A new verb is a sequence of existing verbs with bound
   parameters ("prepare a filing pack = open matter → collect documents → save draft").
   - It adds no new power: the recipe's effect set is the union of its steps', computed
     statically and shown in plain words ("this recipe writes drafts and emails the client
     of record").
   - Every step still passes the gate.
4. **A domain language.**
   - The agent writes programs whose primitives are the domain's verbs (`open_matter`,
     `draft`, `send_to_client_of_record`, `file_with_court`). There is no escape hatch:
     no shell, no arbitrary network.
   - The interpreter is part of ai2rules. Before running, it computes the program's effect
     set statically. While running, every primitive call goes through `gate`.
5. **New primitives.** This is the only rung that needs a programmer, and it is rare.
   Primitives ship as signed **domain packs** ("law-firm pack", "accounting pack"),
   reviewed by the vendor or the firm's IT. These packs are the ready-made per-industry
   configurations that were already the go-to-market idea.

## Two payoffs beyond the original idea

- **It also solves taint granularity.** Inside its own interpreter, the provenance of every
  value is known exactly, so there is no guessing by string matching as on the hook path
  (see `taint-scope-on-the-hook-path.md`). That is argument-level taint (PACT L2) for
  free. CaMeL is built this way: the model writes a plan in a restricted Python-like
  language, and a custom interpreter tracks data flow and capabilities.
- **The vocabulary can be mined, not guessed.** `harness replay-transcripts` (#108)
  already shows which commands real sessions run (`git`, `grep`, `cat`, `cargo test`, …).
  The most frequent shapes are verb candidates. This revives `agent-world-compiler-poc`'s
  "least privilege from observed execution", now with an instrument behind it.

## On "it limits too much"

For a law firm the limit **is** the product: nobody there needs `curl`, they need "send to
the client of record". Two modes follow:

- **Open world, governed** (developers): shell stays, under the kernel.
- **Closed vocabulary, extended through the language** (industries).

## Precedents to study

- **CaMeL** (Google DeepMind, 2025): a restricted interpreter with data-flow and capability
  tracking around an LLM-written plan. Already a §8 contrast case for its LLM role; its
  interpreter design is what matters here.
- **eBPF verifier:** untrusted programs made safe by static verification, not review.
- **Starlark** (Bazel's configuration language): deterministic, hermetic, no I/O except
  what the host provides.
- **GitHub Safe Outputs:** the agent declares operations, and a privileged processor
  executes them with validation.
- **Cedar:** a policy language designed for analysis. Relevant to "the language is the
  review".
- **Claude Skills:** extend an agent's vocabulary with instructions, but draw no boundary.
  A useful contrast for positioning.

## Open questions

- [ ] Recipes (rung 3) can probably be expressed in the existing manifest (a scoped
      capability whose backing is a sequence). Does that need a new `ArgSource` or a new
      backing kind?
- [ ] Rung 4: embed an existing hermetic language (Starlark-like) with domain primitives,
      or design a minimal one? What is the smallest DSL that expresses the law-firm story?
- [ ] How are effect sets shown to a non-programmer before a program runs, so that
      approval is meaningful (ties to `ASK` and `AuthorizationInstance`)?
- [ ] Domain pack format: signing, versioning, how a pack extends a world manifest.
- [ ] A mining pass over `replay-transcripts` output: the top N command shapes as verb
      candidates, measured as the share of calls a candidate vocabulary would cover.

## Done when

A design note that answers the open questions and settles rungs 3 and 4 (recipes in the
manifest, and the choice of DSL), plus a prototype of the law-firm story written as a DSL
program whose effect set is computed before it runs. Then promote it to `PLAN.md` as an
epic.

## Related

- `taint-scope-on-the-hook-path.md`: rung 4 gives exact argument-level provenance.
- `microsoft-365-copilot-integration.md`: domain packs are a candidate offering.
- `docs/demos/law-firm/`: the first domain to express as verbs.
