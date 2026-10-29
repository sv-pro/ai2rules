# AI2-111: business profile v0 and a Cedar spike

**Linear:** [AI2-111](https://linear.app/ai2rules/issue/AI2-111/agent-audit-business-profile-v0-and-a-cedar-spike)
· **Branch:** `claude/ai2-111-cedar-spike` · **Spike files:** [`ai2-111-cedar-spike/`](ai2-111-cedar-spike/)
· **Opened:** 2026-10-29, from the agent-audit dry run on the WebMCP stand.

## Question

Can the business layer of an agent audit be data instead of hand work? Is
Cedar the right format for it, and does verifying a company's policy need
Lean 4?

## Result, in one paragraph

Yes, on this stand. The business layer fits in a 30-line profile. Written as
fixed-class YAML and as Cedar policies, it derives the same facts. Those facts,
joined with the tool layer, compile into D3 worlds byte-identical to the lab's
own for all three matrix tasks and the live assistant, so the committed matrix
follows. Verifying a company's policy splits into two checks, and neither needs
a hand proof:

- **the business rules**, checked symbolically by Cedar's SymCC (proven in Lean
  by AWS);
- **the compiled world**, checked by the real kernel over its whole finite label
  space.

Lean would add one thing: a proof that the kernel itself does what that
enumeration assumes. Not needed for the first audits.

## How to run

```bash
cargo build --release -p cli-harness                    # repo root: the kernel
(cd labs/webmcp && npm ci)                              # the lab's modules
cd _tasks/2_development/ai2-111-cedar-spike
(cd cedar-check && cargo build)                         # own workspace, not a kernel member
CVC5=/path/to/cvc5-1.3.1 ./cedar-check/target/debug/cedar-check . > report.json
node compile-world.mjs                                  # part 1 + part 2, exit 0 = all hold
node compile-world.mjs --control                        # the check must fail on the D2 world
```

cvc5 1.3.1 is the official release from github.com/cvc5/cvc5. Without `CVC5`,
`cedar-check` skips the symbolic part and still runs the rest.

## What was measured (2026-10-29, Windows, cli-harness 0.7.0 built from `f0606e0`)

| Check | Result |
|---|---|
| `profile.yaml` facts = Cedar facts (concrete authorizer, 56 requests) | equal |
| Cedar facts, enumerated = symbolic (SymCC + cvc5, any request, any entity store) | equal; symbolic derivation 69 ms |
| Compiled world = lab's `worldYaml`: shop-buy, crm-gift, crm-renewal, live assistant | 4 of 4 byte-identical |
| Live world through `harness gate`, every tool × mode × ambient × argument labels | 112 requests, 3.7 s, P1–P4 hold |
| Positive control: same check on the D2 world (no labels, no approvals) | P1 (58), P2 (10), P4 (12) fail, as they must |
| Property: no private data to search (`northwind.cedar`) | holds |
| Property: same, on `northwind-loosened.cedar` ("search is only a read") | fails; counterexample is the lab's S5b leak |
| Property: payments over 10,000 need the CFO (`books.cedar`) | holds |
| Property: invoice data only to its own vendor (`books.cedar`) | holds |
| Property: CFO rule on `books-loosened.cedar` (threshold typo 100000) | fails; counterexample: an unapproved payment of 34,465 |
| Regression: loosened vs original, both domains | each caught with one counterexample; original vs itself clean |

Each property check takes under 10 ms. The four world properties:

- **P1.** Private data never reaches a tool on another system: DENY, `confidentiality_violation`.
- **P2.** An order asks a person (ASK) in interactive mode and is DENY unattended.
- **P3.** Forbidden and unknown tools are ABSENT in every context.
- **P4.** The CRM read is ALLOWed and its result labelled private.

## Findings

1. **Two layers, one world.** The tool layer is the inventory: which tools exist, their
   side-effect class and arguments. The business layer is the owner's answers: which
   systems hold private data, what needs approval, what must not exist. They compile
   into one D3 world. The world is a pure function of three inputs (task capabilities,
   `sensitiveOrigins`, `approvals`), which is why the join is simple. Keep two sources,
   compile to one artifact. Do not make the owner edit the world.
2. **The binding between layers appears twice.** Cedar's `permit` policies restate the
   inventory's tool → system binding, because "back to the system it came from" needs
   it. Whether the two copies agree is itself a check an audit should run.
3. **Approval needs a "nobody present" answer.** "A person approves orders" has no
   meaning in an unattended run. The kernel turns ASK into DENY in background mode, so
   the compiled background world leaves approvals out, as the lab's matrix world does.
   The profile should say what happens when nobody can approve. Today it is implicit
   fail-closed.
4. **Per-company verification needs no hand proof.**
   - *Business rules:* SymCC answers always-allows / always-denies, implication,
     equivalence and disjointness for every request, with counterexamples. AWS proved
     the analyzer in Lean once; each company's policy is checked by running it.
     Counterexamples show synthesized entities with empty ids and need post-processing
     to read well.
   - *Compiled world:* the kernel's flow verdict depends on the tool, mode, ambient,
     argument labels and taint, which is a finite space (here 2 × 2 × 3^args). So
     enumerating it through the real binary is a complete check of the world,
     relative to that binary. One honest gap: this run held taint at `clean` and the
     channel at `user_prompt`. Taint only tightens, but a complete check should
     enumerate it too.
5. **Where Lean would fit.** The enumeration rests on one claim: the verdict does not
   depend on argument values. That holds while the world uses no `arg_constraints`
   (D86) and no scoped-capability argument rules. The claim is tested (656 tests, the
   matrix), not proven. A Lean model of the kernel's decision function would turn the
   enumeration into a proof and make value constraints checkable symbolically. That
   is research on the scale of months. Worth doing when "proven" becomes part of what
   is sold, or when a world uses value constraints. Not for the first audits.
6. **What Cedar adds over the kernel, and what it cannot do.**
   - *Adds:* value thresholds with routing ("over 10,000 → CFO") and per-entity rules
     ("to its own vendor"). These are gap 6 of the dry-run report; no world field
     expresses them today.
   - *Cannot:* track data flow. In `northwind.cedarschema` the host supplies
     `private_from` from the labels D3 already carries, so labelling stays the
     kernel's and the host's job.
7. **Prior art spotted, not evaluated:** the crate `cedar-agent-schemas` (VeritasActa),
   a Cedar schema for agent action verbs.

## D98 draft (for the maintainer; not in DECISIONS.md yet)

> **D98 — The business layer: owners write a fixed-class profile; it compiles to the
> D3 world and to Cedar; Cedar verifies, the kernel decides.**
>
> *Decision.* An audit's business layer is authored as a profile in fixed classes:
> entities with a confidentiality class; actions with an effect class, an approver and
> `allowed`. It compiles to two artifacts:
> - the D3 world (sources, sinks, `approval_required`, absence), which the kernel
>   enforces unchanged;
> - a Cedar policy set, used to verify the profile (properties, regression between
>   versions, with counterexamples via SymCC).
>
> Rules the world cannot express (value thresholds, per-entity routing) are evaluated
> from Cedar host-side, and may only tighten a kernel verdict to ASK or DENY. This is
> the add-on rule of D96, applied to a policy engine.
>
> *Alternatives.*
> - Cedar as the authoring format: owners do not read Cedar.
> - Cedar as the decider: contradicts D27, governance by structure, not by a policy
>   decision.
> - OPA/Rego: no verified analyzer; kept for integration export (D6 / E4.6).
> - A new DSL: deferred to the domain-language discovery.
>
> *Why.* In the AI2-111 spike, profile and Cedar derived identical facts. The compiled
> worlds were byte-identical to the lab's. SymCC caught both loosening edits with
> counterexamples, each check in milliseconds.

## Not done here

- A YAML → Cedar generator. Both files were written by hand from the same answers.
- Wiring Cedar evaluation into the host for value rules.
- A second stand. Accounts payable is modelled in Cedar only; there is no world or
  agent for it yet.
- `DECISIONS.md` and `PLAN.md` are untouched until D98 is accepted.
