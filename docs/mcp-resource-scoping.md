# Scoping an MCP connector to a resource subtree

**Status:** design proposal, no code. **Date:** 2026-10-30.
**Builds on:** D51 (the world owns the surface), D61 (a new scope changes target-less
verdicts), D75 (discovery projection), D79 (scoped capabilities rename and limit MCP
arguments), D86 (the argument-schema vocabulary the kernel enforces), D95 (facts that
fail closed when absent), D96 (add-ons supply facts, never verdicts).

A hosted SaaS connector such as the Notion MCP server acts **as the user**, with the
user's access to the whole workspace. The vendor does not let you narrow it to one page
and the pages below it. Every call still names its target, though: a page or database
ID, a URL, a `collection://` URI, a `parent` on create. This note asks how a world can
say «this connector may touch page P and everything under it, and nothing else», and
how the kernel can hold that line deterministically.

Most of the answer already exists. Capability scoping pins and limits argument values
today. The one missing piece is **membership in a set that grows**: the descendants of P
are not known when the world is written.

## What capability scoping already does

Three mechanisms already narrow what a tool call may name:

| Mechanism | Where | What it limits |
|---|---|---|
| `arg_constraints` on a base action (D86) | `harness_types::arg_schema`, enforced in `world_kernel::schema` on every call | `const`, `enum`, numeric bounds, lengths, items, properties. A keyword the kernel cannot enforce fails the compile. `pattern` is refused on purpose. |
| Scoped capabilities (D79) | `ScopedCapabilityDef`, `ArgSource::{Literal, Input{as, one_of, prefix}, ContextRef}` | A narrowed verb over a base action: a pinned value, a renamed argument, a value list or a string prefix. `projected: false` takes the wide base action off the surface. |
| Discovery projection (D75) | `harness_preview::project` | Which names exist for the model at all. |

The object-capability case for a single page is already a committed demo.
`docs/demos/confluence-docs/confluence-docs.world.yaml` grants updates to one
tenant/page pair with `arg_constraints: { cloudId: { const: … }, pageId: { const: … } }`,
and it leaves `getConfluencePageDescendants` undeclared, so the tool is absent.
`crates/cli-harness/tests/fixtures/my-jira.world.yaml` shows the scoped form: issues
starting `PLAT-` only, a fixed `cloudId`, and the wide base tools kept off the surface.

For a Notion subtree whose structure is known when the world is written (one page,
four databases, their data sources), static scoping already goes a long way. The tool
names below are the hosted Notion server's. The schemas are cut down to the arguments
that matter; take the upstream's real schemas with `harness mcp-author`.

```yaml
mcp_surface: { name: Project_X_Notes, upstream: notion }

base_actions:
  - name: notion-query-data-sources
    action_type: Read
    side_effect: Read
    projected: false
    backing: !McpServer { server: notion, tool: notion-query-data-sources }
    schema: { type: object, required: [data_source_id], properties: { data_source_id: { type: string } } }
  - name: notion-create-pages
    action_type: Write
    side_effect: External
    projected: false
    backing: !McpServer { server: notion, tool: notion-create-pages }
    schema:
      type: object
      required: [parent, pages]
      properties:
        # Nested objects are closed by default (D86), so the pinned parent's keys are declared.
        parent: { type: object, properties: { type: { type: string }, data_source_id: { type: string } } }
        pages: { type: array }

scoped_capabilities:
  - name: query_project_table
    base_action: notion-query-data-sources
    description: Query one of Project X's tables.
    args:
      data_source_id: !Input { as: table, one_of: ["collection://<ds-1>", "collection://<ds-2>"] }
  - name: add_project_row
    base_action: notion-create-pages
    description: Add a row to Project X's main table.
    args:
      parent: !Literal { type: data_source_id, data_source_id: "<ds-1>" }
      pages: !Input { as: rows }
```

This works today, through `harness mcp-gateway`. It gives the model **reads over the
known tables and inserts into them**, and nothing else in the workspace. The upstream's
search, fetch-anything and move tools do not exist for it.

## Where static scoping stops

1. **The set grows.** Every row in a Notion database is a page with its own ID, created
   after the world was written. «Fetch row R» or «update row R» needs R to be a member of
   the subtree, and no `enum` written in advance can list it. `prefix` does not help
   either, because Notion IDs carry no hierarchy.
2. **One identifier, many spellings.** The same page arrives as a 32-hex UUID, as a
   dashed UUID, as a `notion.so/...` or `app.notion.com/p/...` URL with or without a
   title slug and query string, and its database as `collection://<uuid>` or
   `view://<uuid>`. A `const` or `enum` compares the raw string. That is safe, since a
   different spelling is denied, but brittle. It is only *correct* if every spelling is
   listed, or if the value is normalized before comparison.
3. **Tools with no single target.** Workspace search, user lookup and «list my private
   pages» have no argument to pin. Today the only scoped options are to keep them off
   the surface, or to pin a filter argument when the upstream offers one (Notion's search
   takes a `page_url` restricting results to a page and its descendants). The second
   option trusts the upstream's filter.
4. **Writes that escape through a second argument.** Moving a page, duplicating it, or
   setting a relation property that points at an outside page names a *second* resource.
   Pinning the first argument does not limit the second.

None of these is a gap in parameter scoping as such. Items 2–4 are what the world has to
spell out. Item 1 needs one new kind of limit.

## Proposal

### 1. Canonical resource identifiers, computed in the kernel

Canonicalizing an identifier is a pure string function, so it belongs in
`harness-types` beside the argument vocabulary, not in an adapter. A world declares a
**resource kind** per connector (`notion_page`, `notion_collection`). The kernel maps
every accepted spelling of an argument to one canonical form, for example
`notion:page:<32 lowercase hex>`:

- 32-hex and 8-4-4-4-12 dashed UUIDs;
- `https://www.notion.so/...` and `https://app.notion.com/p/...` URLs, taking the
  trailing 32-hex token after an optional title slug and ignoring the query and the
  fragment;
- `collection://<uuid>` and `view://<uuid>` for the collection kind.

Anything else, including a URL with two candidate tokens, is
`resource_unparseable` (DENY). It is never passed through. The same function runs in
the compiler on the world's own literals, so a world author can write any spelling, and
`const`/`enum` limits on a resource-kind argument compare canonical forms. That alone
fixes item 2 for the static case.

### 2. Resolver-backed predicates, with `within` as the first one

Today's limits are all **static**: `const`, `enum`, bounds, `one_of`, `prefix`. The
kernel decides them from the value and the world alone. This note adds a second family,
**resolver-backed predicates**. The world names a predicate over an argument. The
predicate's truth is deterministic for a given state of the outside world, but it is not
known when the world is written, so a resolver computes it, with as many upstream calls
as it takes. Subtree membership is the first predicate. Others have the same shape:

| Predicate | Resolver asks | Example use |
|---|---|---|
| `within: <scope>` | the resource's ancestor chain | a Notion page under P, a Confluence page under a space root |
| `same_container: <scope>` | the resource's tenant, space or project | a Jira issue in project PLAT, whatever its key |
| `created_by: <actor>` | the resource's author | edit only what this agent created |
| `not_shared_externally` | the resource's sharing settings | read a document only if no outside guest can see it |

Every predicate obeys the same four rules, so the kernel needs one mechanism, not one per
predicate:

1. The world declares the predicate, its resolver and its failure meaning; the compiler
   refuses a predicate name it does not know (D86).
2. The kernel canonicalizes the argument itself (§1) and accepts only a fact bound to
   that canonical identifier.
3. A missing fact denies.
4. A fact can satisfy its own predicate and nothing else; it never lifts another limit
   (D96).

The rest of this note works `within` through end to end.

```yaml
resource_scopes:
  project_x:
    kind: notion_page
    root: "<page-id>"            # any accepted spelling; canonicalized at compile time
    resolver: notion-ancestry    # the add-on that answers membership (§3)
    on_unknown: deny             # what a missing or failed answer means (D96)

scoped_capabilities:
  - name: read_project_page
    base_action: notion-fetch
    args:
      id: !Input { as: page, within: project_x }
```

`InputRule` gains `within: <scope>` beside `one_of` and `prefix`. All the limits that
are set apply together. The same limit is available as an `arg_constraints` entry
(`id: { within: project_x }`) for hosts that cannot rename tools (§7). That makes it the
first assertion keyword that is not JSON Schema, so it is listed in `arg_schema` as its
own kind and shares the D86 rule: a scope the compiler cannot resolve fails the world.

### 3. Membership is a fact from the host, never from the model

Whether page R lies under P takes I/O (asking Notion for R's ancestors), so per D96 the
kernel never computes it. An add-on, the **resolver**, runs at the host or in the
gateway and puts a fact into the gate call:

```json
"context": {
  "resources": {
    "args": {
      "id": {
        "canonical": "notion:page:3f2a…",
        "holds": { "within:project_x": true },
        "evidence": { "resolver": "notion-ancestry@1", "ancestors": ["notion:page:<root>", "…"], "observed_at": "2026-10-30T10:12:03Z" }
      }
    }
  }
}
```

The kernel judges the fact as follows:

- It canonicalizes the actor's value itself (§1) and **requires the fact's `canonical`
  to equal it**. A fact about a different resource than the one the call names is
  ignored as if absent, so a confused or hostile host-side bug cannot launder an
  outside ID through a fact about an inside one.
- `within: project_x` is satisfied only by `holds["within:project_x"] == true`. A
  missing fact, `false`, or a fact for another predicate or scope is DENY
  `resource_scope`. The rule fails closed
  the way a flow-contracted action without its sidecar does (D95).
- Composition only tightens (D96). A membership fact can satisfy a `within` limit; it
  cannot lift `one_of`, `const`, a projection or a taint rule.
- The fact is recorded in the decision log with its evidence, and replay uses the
  recorded fact instead of asking the resolver again (D96, «signals are evidence»).

**Why a new carrier.** D96 asks for existing carriers first. None fits. A flow label
(D95) says integrity and confidentiality, not «is a member of». Taint is one bit per
session. `ContextRef` supplies a value, it does not vouch for one. D96's own consequence
applies: «a finding counts only if it can be said in the kernel's vocabulary», and
membership is where the vocabulary has to grow. `context.resources` is additive in ABI
v1, so a host that sends nothing gets DENY on every `within` argument and no change
anywhere else.

### 4. The resolver

- **Trusted path.** It asks the upstream with the *operator's* connection (a
  `notion-fetch` the gateway issues itself, reading the returned ancestor path), never
  through the model's tool call and never from anything the model wrote.
- **Cache.** Membership is cached per canonical ID with a short TTL. A miss is a lookup,
  and a lookup failure follows `on_unknown` (`deny` is the default; there is no
  `allow`).
- **Root moves.** If P itself moves or is deleted, every cached answer is void. The
  resolver re-checks the root on a TTL as well.

### 5. Time of check and time of use

The fact is true when the gate decides. The page can move out of P before the gateway
forwards the call. For writes, the strong answer is to pin the parent with a `Literal`
(rows are created *in* a known table) and to treat a move as its own governed action
(§6). For reads, the gateway re-resolves just before forwarding when the cached entry is
older than the TTL. That narrows the window without closing it. The residual risk is a
read of a page that left P within the last TTL, and the note keeps it stated rather
than claimed away. A connector that returns the ancestor path with every fetched page
would let the gateway check the *result* too, and drop it if it is no longer inside.

### 6. Tools with no single target, and writes that name a second resource

| Tool shape | Default | Option |
|---|---|---|
| Workspace search | `projected: false` (absent) | Pin the upstream's subtree filter (`page_url: !Literal <root>`), **and** pass every result ID through the resolver, dropping non-members. A result list is an output channel, so the post-filter is a sink rule, not a courtesy. |
| User lookup, «list private pages», teams | absent | none; they enumerate the workspace by design |
| Fetch / update / comment on page R | `within` on R | — |
| Create page | `parent` pinned by `Literal`, or `within` on the parent | — |
| Move page R to parent Q | `within` on **both** R and Q | absent if not needed |
| Duplicate R | `within` on R and on the destination | absent if not needed |
| Relation property values | `within` on every item (`items: { within: … }`) | strip relations from the surface |

The rule is the same everywhere: **every argument that names a resource carries its own
limit**. A tool whose resource-naming arguments are not all declared is not served.

### 7. Host fit

- **`harness mcp-gateway`** is the full form. It serves scoped verbs (D79), projects the
  wide ones away, resolves membership before it forwards, and owns the TOCTOU re-check.
  It requires the connector to run behind the gateway: a local MCP server with the
  operator's token, not the vendor's hosted endpoint.
- **`cc-hook`** sees a hosted connector's calls as `mcp__<server>__<tool>` plus the
  arguments the model sent. It cannot rename or hide tools, so only the
  `arg_constraints` form of `within` applies, on the host tool's own argument names.
  Two cautions: for claude.ai connectors `<server>` is an opaque identifier, so a world
  pinned to it is pinned to one installation; and nothing in PreToolUse can post-filter
  a search result, so search stays absent there.
- **One-kernel parity (D61).** Declaring a scope changes the verdict of every call that
  arrives without a resolvable target: it becomes `resource_unparseable` or
  `resource_scope` DENY. As with `roots`, the shared case set gets a second world and its
  own cases (`resource-scope-world.yaml` / `resource-scope-cases.yaml`) rather than
  bolting the scope onto the demo world.

## Prior art: policy engines and relationship graphs already split facts from judgment

The split proposed here is the established one in authorization systems:

- **Cedar** (AWS). A request carries the principal, action, resource and context, and the
  application supplies the *entities*: their attributes and their parents. Cedar's
  `resource in Folder::"P"` is transitive membership over the parent edges the caller
  provided. Evaluation is pure and deterministic, with no I/O; loading the right slice of
  entities is the application's job. That is `within` almost exactly: the resolver is
  the entity loader, and the kernel is the evaluator.
- **OPA / Rego.** Policies read `input` plus `data`. External data arrives by bundles
  pushed ahead of time, or at evaluation through `http.send`. The second option makes a
  decision depend on a live call, and the decision log has to capture the response to
  stay explainable. That is the same problem as §4's evidence, solved there by recording.
- **Zanzibar-style relationship checks** (Google Zanzibar; OpenFGA, SpiceDB). «Is
  document D in folder P» is answered by traversing stored relation tuples, and the check
  returns a computed fact, with a consistency token pinning which snapshot it was
  computed on. A resolver for a connector without an ancestry API could be exactly this:
  a mirrored relation store, refreshed from the upstream, queried by the gateway.

What this note adds to that lineage is the pairing with an agent's tool call. The fact is
bound to the canonical identifier the *model* sent, the resolver never trusts the model,
and an absent fact denies instead of falling back to a default policy.

## Alternatives

- **Write the predicates in Cedar instead of the world manifest.** Plausible later, and
  related to the open policy-engine question. Cedar's entity model fits, and its
  evaluator is pure. It would put a second policy language beside the manifest, though,
  and a second evaluator beside the kernel. Revisit when there is more than one
  predicate to express.
- **Vendor-side scoping, where it exists.** Notion's classic internal integrations are
  shared page by page, and children inherit access. Run the vendor's open-source MCP
  server with such a token behind the gateway, and the upstream itself cannot see
  outside P. This is the strongest option when available and should be preferred. It is
  not available for the hosted connector, which acts as the user. The gateway's scoping
  still adds the audit trail and the narrowed surface on top.
- **Enumerate IDs statically.** Correct, and it works today (the confluence-docs demo).
  It fails the moment the subtree grows.
- **`pattern` on IDs.** Refused by D86 for dialect reasons, and useless here anyway:
  Notion IDs carry no hierarchy to match.
- **Let the model state the ancestry.** Rejected. It is a claim, not a fact, and the
  injection that wants out of P writes the claim.
- **Resolve ancestry inside the kernel.** Rejected. It is I/O, and the kernel stays pure
  (AGENTS.md invariants, D96).
- **A `ContextRef` for the root.** It would inject the root as a value, which pins
  *create-under-root* but says nothing about a row's membership.

## Open questions

- Should `within` live only in `InputRule` and `arg_constraints`, or also on
  `trust_pins`-style operator attestations for long-lived resources (D29)?
- Does the resolver's evidence belong in the decision log verbatim (ancestor IDs are
  workspace structure, and possibly sensitive), or as a hash with the full record kept
  beside the log?
- What TTL is defensible for reads, and should a write always re-resolve regardless of
  the cache?
- Is a generic `context.resources` the right shape, or should membership ride a
  generalized `context.signals` (D96 deferred that field until an experiment needs it)?
- The first experiment: the WebMCP lab or the governance benchmark (E18), with a
  scenario where an injected instruction asks the agent to fetch an outside page and
  the weak baseline (static `enum`) either over-denies rows or under-denies.

## Proposed decision

To be recorded as the next `D` entry once accepted: **an argument may carry a
resolver-backed predicate, `within: <scope>` first. The kernel canonicalizes the
identifier and accepts only a host-supplied fact bound to that canonical identifier. An
absent fact denies, and a fact never lifts another limit.**
