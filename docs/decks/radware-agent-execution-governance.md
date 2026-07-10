# Agent Execution Governance — internal proposal deck (Radware)

10-slide internal proposal. **Goal:** position *Agent Execution Governance* as a
missing / under-formalized layer in the Agentic AI & API Security portfolio, and
ask for a **cheap PoC — not a product commitment**.

**Audience:** department head · security researcher · PM · API security engineer.
**Tone:** credible and concise, for a skeptical internal security/product crowd.
Grounding: [`docs/THESIS.md`](../THESIS.md), [`docs/GLOSSARY.md`](../GLOSSARY.md),
[`docs/demos/jira-copilot/`](../demos/jira-copilot/README.md).

---

## Slide 1 — The ask: a 4-week PoC, not a product

**Key message:** We're asking for a decision on a small, time-boxed experiment —
evaluate *Agent Execution Governance* as a layer in our Agentic AI / API Security
story, using a working open reference implementation we already have.

- **Decision requested:** approve a 4-week, 1–2 person PoC. No roadmap commitment,
  no headcount ask beyond that.
- **Claim to test:** enterprises will need to govern *what AI agents are allowed to
  execute* — deterministically, at the protocol seam — and today no one in our
  portfolio (or most competitors') formalizes that layer.
- **PoC scope:** a governance gateway in front of a real MCP server (Jira), shaping
  the tool surface per policy, with a full audit trail — demoed on GitHub Copilot
  and Claude Code.
- **Success criteria (pre-agreed):** destructive tools provably unreachable; identical
  policy across ≥2 agent hosts; decision log replayable; a scorecard we can publish.
- **Cost of being wrong:** ~1 engineer-month. Cost of being late: watch slide 5.

**Suggested visual:** single "decision box" — *Approve 4-week PoC* — with three
small stamps: `no product commitment` · `existing code` · `defined exit criteria`.

**Speaker note (עברית):**
אני פותח עם ההחלטה כדי לחסוך לכם זמן: אני מבקש אישור ל-PoC של ארבעה שבועות, לא
מחויבות למוצר. יש לנו מימוש עובד שאפשר להישען עליו, אז ההשקעה היא בעיקר אינטגרציה
והערכה. כל שאר המצגת נועדה לשכנע אתכם שהשכבה הזאת אמיתית, שהשוק זז לכיוון שלה,
ושהיא יושבת בדיוק בתפר שבו רדוור חזקה — תעבורת API. אם בסוף אתם לא משתכנעים
בתזה, לפחות תדעו בדיוק מה אנחנו בודקים ומה קריטריון היציאה.

---

## Slide 2 — Agents are execution loops, not chatbots

**Key message:** The unit of risk in agentic AI is not generated *text* — it's the
*execution loop*: an LLM with real authority, ingesting untrusted input on every turn.

- An agent = model → tool call → result → model, repeated. The model *acts*; output
  is side effects, not prose.
- Agents inherit broad ambient authority: repos, shells, APIs, tickets, credentials,
  internal MCP servers.
- Meanwhile every loop iteration ingests untrusted text: web pages, issue bodies,
  API responses, retrieved documents.
- Prompt injection is therefore an **authority-boundary problem**, not a prompt
  problem — the attacker's goal is a tool call, not a rude sentence.
- Conclusion: the control point that matters is the **action boundary**, not the
  text channel.

**Suggested visual:** the agent loop as a circle (model → tool call → effect →
observation → model), with red arrows of untrusted input entering every node and a
single highlighted cut-point at "tool call".

**Speaker note (עברית):**
הנקודה כאן היא לשנות את מסגרת החשיבה. כולם מדברים על "מה המודל אמר"; הסיכון האמיתי
הוא "מה המודל עשה". סוכן הוא לולאת ביצוע שמחזיקה הרשאות אמיתיות ובולעת קלט לא-מהימן
בכל סיבוב. לכן הזרקת פרומפט היא לא בעיית ניסוח אלא בעיית גבול סמכויות — והמקום
הנכון לשים בו בקרה הוא נקודת הביצוע עצמה, לא הטקסט.

---

## Slide 3 — MCP is the agent's action vocabulary — in the open, on the wire

**Key message:** MCP standardizes exactly the thing we need to govern: `tools/list`
declares what an agent *can* do, `tools/call` is it *doing* it — and both are
observable API traffic.

- MCP (Model Context Protocol) is the emerging standard connecting agents to tools;
  adopted across Copilot, Claude, Cursor, JetBrains, and enterprise vendors
  (Atlassian, GitHub, AWS…).
- `tools/list` = the agent's complete action vocabulary; `tools/call` = the
  execution event. The whole risk surface is enumerated in-band.
- This is JSON-RPC over a transport — i.e., **API traffic**. It can be proxied,
  filtered, and audited like any API.
- One enforcement point at this seam is **host-agnostic**: the same policy governs
  every IDE/agent that connects through it.
- What today's stacks do at this seam: mostly authn/authz and logging. What almost
  no one does: *shape which tools exist per context*.

**Suggested visual:** wire diagram — agent host(s) on the left, MCP server on the
right, a proxy box in the middle intercepting `tools/list` (filter) and
`tools/call` (gate); annotate "JSON-RPC = API traffic".

**Speaker note (עברית):**
זה השקף שמחבר אותנו הביתה. MCP הוא בעצם ה-API שבו הסוכן מגלה ומפעיל את היכולות
שלו: tools/list זה אוצר הפעולות, tools/call זו הפעולה עצמה. זו תעבורת JSON-RPC
רגילה — בדיוק סוג התעבורה שאנחנו יודעים לתווך, לסנן ולנטר. ומכיוון שכל המארחים
מדברים את אותו פרוטוקול, נקודת אכיפה אחת בתפר הזה מכסה את כולם. הפער בשוק: כולם
עושים אימות ולוגים, כמעט אף אחד לא מעצב את המשטח עצמו.

---

## Slide 4 — Execution governance is not guardrails

**Key message:** Guardrails ask a model "does this look malicious?" Execution
governance makes the dangerous action structurally impossible. These are different
layers — and only one of them can give guarantees.

- Guardrails = a stochastic classifier in the trust path. If security depends on a
  classifier being right 100% of the time, it's already lost.
- Governance = a **deterministic** decision: pure function of (request, context,
  compiled policy). Same inputs → same decision → replayable, auditable.
- The strongest control is **absence, not refusal**: a capability the agent cannot
  even see cannot be jailbroken, argued with, or retried.
- Guardrails remain useful for content risks (toxicity, data leakage in text); they
  complement governance, they don't substitute for it.
- Analogy for this audience: guardrails ≈ anomaly detection; execution governance ≈
  a positive security model / firewall policy for agent actions.

**Suggested visual:** two-column contrast table (Guardrails vs Execution
Governance): stochastic/deterministic · advisory/binding · per-prompt/per-action ·
"detects" / "makes impossible".

**Speaker note (עברית):**
קהל סקפטי ישאל: "יש כבר guardrails, מה חדש?" ההבדל מהותי. Guardrails הם מסווג
סטטיסטי בתוך נתיב האמון — והוא חייב לצדוק תמיד. משילות ביצוע היא פונקציה
דטרמיניסטית: אותם קלטים, אותה החלטה, ניתנת לשחזור מלא. והבקרה החזקה ביותר היא
היעדר, לא סירוב — יכולת שהסוכן בכלל לא רואה אי אפשר לשכנע אותה. בשפה שלנו: זה
positive security model לפעולות של סוכנים, לא עוד גלאי אנומליות.

---

## Slide 5 — The market is formalizing this layer right now

**Key message:** Every major platform vendor shipped or announced agent-governance
primitives in the last year. The layer is being defined *now* — and mostly inside
their own walled gardens, which leaves the neutral, in-line seam open.

- **Microsoft:** Agent Governance Toolkit (deny-by-construction policy middleware,
  MCP security gateway) + Entra Agent ID for agent identity.
- **NVIDIA:** NeMo Guardrails / agent safety stack — guardrail-centric, reinforcing
  that "guardrails" is where most vendors stop.
- **Palo Alto Networks:** Prisma AIRS + the Protect AI acquisition — AI runtime
  security explicitly on the roadmap of our direct competitor.
- **AWS:** Bedrock Guardrails and **AgentCore** (gateway, identity, policy for
  agents) — governance as a first-class managed primitive.
- Pattern: each governs **its own** platform. Enterprises running mixed agents
  (Copilot + Claude + internal) need a **vendor-neutral enforcement point** — the
  same gap WAF/API security filled for web apps.

**Suggested visual:** 2×2 or quadrant logo map — axes "platform-bound ↔ neutral" ×
"guardrails ↔ execution governance"; incumbent logos cluster in platform-bound /
guardrails; the target quadrant (neutral + execution governance) nearly empty.

**Speaker note (עברית):**
זה שקף הדחיפות. מיקרוסופט, אנבידיה, פאלו אלטו ו-AWS — כולם הוציאו בשנה האחרונה
רכיבי משילות לסוכנים. חשוב לדייק מול קהל ביקורתי: רובם עוצרים ב-guardrails או
במשילות בתוך הפלטפורמה של עצמם. ארגון אמיתי מריץ ערבוב של Copilot, Claude וסוכנים
פנימיים — והוא יצטרך נקודת אכיפה ניטרלית בקו התעבורה, בדיוק כמו ש-WAF מילא את
התפקיד הזה לאפליקציות ווב. החלון הזה פתוח עכשיו, והוא לא יישאר פתוח שנתיים.

---

## Slide 6 — Portfolio fit: from posture & detection to authority governance

**Key message:** Radware already owns the seam this runs on. AISPM tells you what
agents exist; Agentic AI Protection detects attacks; **Agent Authority Governance**
would decide, deterministically and in-line, what each agent may execute.

- Our DNA is **in-line, real-time decisioning on API traffic** — WAF, API
  Protection, Bot Manager. MCP is API traffic.
- Portfolio ladder: **AISPM** = posture ("what agents/tools exist, misconfigured?")
  → **Agentic AI Protection** = detection ("is this interaction an attack?") →
  **missing rung: authority** ("what is this agent *allowed to execute*, here, now?").
- Execution governance reuses muscles we have: policy engines, inline proxying,
  per-endpoint schemas, audit pipelines — applied to `tools/list` / `tools/call`.
- Differentiator vs. platform vendors: we're neutral. One policy across every agent
  host and every MCP server, enforced where we already sit.
- This is an *extension* of the API security story ("agents are the new API
  consumers"), not a new business.

**Suggested visual:** three-rung ladder or pyramid — Posture (AISPM) → Detection
(Agentic AI Protection) → Authority (Agent Execution Governance), with the top rung
highlighted as the gap; side note mapping each rung to existing Radware assets.

**Speaker note (עברית):**
כאן אני מתחבר לפורטפוליו. יש לנו כבר שתי שכבות: AISPM עונה על "מה קיים ומה
מוגדר לא נכון", ו-Agentic AI Protection עונה על "האם זו התקפה". חסרה השכבה
השלישית: "מה הסוכן הזה מורשה לבצע, כאן ועכשיו" — החלטה דטרמיניסטית בקו התעבורה.
זה לא עסק חדש בשבילנו; זה בדיוק השריר של API Protection מופעל על פרוטוקול חדש.
והיתרון שלנו מול ענקיות הפלטפורמה הוא הניטרליות — מדיניות אחת על כל המארחים.

---

## Slide 7 — The World Manifest: a compiled, contextual action surface

**Key message:** Policy is declared once in a human-reviewable manifest, compiled
into an immutable artifact, and enforced by table lookup at runtime — the agent's
"world" is whatever the manifest says exists.

- A **World Manifest** (YAML) declares the actions, argument schemas, scopes, and
  trust requirements that exist for a given context. Everything undeclared is
  **absent**, not blocked.
- It compiles to an immutable, hash-addressed **CompiledWorld** — versioned,
  diffable, reviewable in a PR like any security policy.
- Runtime enforcement is a pure function: lookup + schema validation + trust check.
  No model call, no heuristics, microseconds not seconds.
- The same compiled policy is projected to every host: the gateway filters
  `tools/list` and gates `tools/call` against it.
- Familiar shape on purpose: think *OpenAPI spec + positive security model*, for
  agent actions instead of HTTP endpoints.

**Suggested visual:** pipeline — `world.yaml` → compiler → `CompiledWorld (hash
v1.3)` → gateway → per-host projected tool surface; a small YAML snippet showing
3 allowed Jira actions.

**Speaker note (עברית):**
זה הלב הטכני, ואני שומר אותו פשוט. מניפסט אחד ב-YAML מגדיר איזה עולם קיים עבור
הסוכן: אילו פעולות, עם אילו סכמות, ובאיזו רמת אמון. כל מה שלא הוצהר — פשוט לא
קיים. המניפסט מתקמפל לארטיפקט חתום וגרסתי, כמו כל מדיניות אבטחה שעוברת code
review. בזמן ריצה ההחלטה היא lookup דטרמיניסטי — בלי מודל, במיקרו-שניות. למי
שמכיר API Protection: זה OpenAPI עם מודל אבטחה חיובי, רק לפעולות של סוכנים.

---

## Slide 8 — Core principles (what makes it defensible)

**Key message:** Five principles separate this from "yet another policy engine" —
and each one is a concrete, testable property, not a slogan.

- **ABSENT ≠ DENY.** Undeclared tools are removed from `tools/list` — the agent
  can't see, retry, or socially-engineer around what doesn't exist. DENY is
  reserved for declared-but-currently-forbidden, with distinct audit semantics.
- **Capability shaping.** The tool surface is *projected* per context (role, task,
  environment) — not one global allowlist, a compiled world per situation.
- **Stochastic at design time, deterministic at runtime.** An LLM may help *draft*
  the manifest; a human reviews it; runtime never consults a model. No LLM in the
  trust path.
- **Monotonic taint.** Once a session touches untrusted content, its trust level
  only ratchets down — a poisoned document can't later drive a privileged write.
- **Provenance & replay.** Every decision logs inputs, world version, and outcome;
  any incident can be re-executed bit-for-bit for forensics and compliance.

**Suggested visual:** five icon tiles in a row (ghost-tool = ABSENT, stencil =
shaping, split brain/gear = design vs runtime, one-way ratchet = taint, ledger =
provenance), one line each.

**Speaker note (עברית):**
אלה חמשת העקרונות שהופכים את זה למשהו שאפשר להגן עליו מול חוקר אבטחה. הכי חשוב:
ההבדל בין ABSENT ל-DENY — כלי שלא מופיע ברשימה אי אפשר לתקוף, לנסות שוב או לשכנע.
עיצוב יכולות אומר שהמשטח נגזר מההקשר, לא רשימה גלובלית. ה-LLM מותר בזמן תכנון,
אסור בזמן ריצה. Taint מונוטוני מבטיח שסשן שנגע בתוכן לא-מהימן לא יוכל אחר כך
לבצע כתיבה מוסמכת. וכל החלטה ניתנת לשחזור מלא — זה זהב לפורנזיקה ולרגולציה.

---

## Slide 9 — Demo: shaping the Jira MCP surface (working today)

**Key message:** This isn't a slideware architecture — a single Rust binary already
governs the Jira MCP surface identically for VS Code Copilot, JetBrains Copilot,
and Claude Code.

- One manifest declares: Jira **reads + `add_comment` only**. Every destructive
  tool (`delete_issue`, `bulk_create`, transitions) is **ABSENT** — it never
  appears in any host's tool picker.
- "Clean up old issues" → the delete tool simply doesn't exist for the agent. No
  prompt battle, nothing to bypass.
- Taint floor live: a session that ingested untrusted content gets `add_comment`
  **DENIED** — same request, different provenance, different decision.
- Every ALLOW / DENY / ABSENT lands in an append-only audit log; decisions replay
  deterministically.
- Zero-friction demo: self-contained mock Jira, no credentials; swapping in the
  real Atlassian MCP server is a config change, not a rewrite.

**Suggested visual:** side-by-side screenshots — Copilot's tool picker *without*
the gateway (full Jira surface incl. delete) vs. *with* it (reads + comment only);
below, two audit-log lines showing ALLOW vs. taint-DENY of the same tool.

**Speaker note (עברית):**
כאן אני מראה שזה רץ, לא רק מצויר. בינארי אחד ב-Rust יושב בין המארח לשרת ה-MCP של
Jira. מניפסט אחד — קריאה ותגובה בלבד — וכל הכלים ההרסניים פשוט לא מופיעים, באותה
צורה בדיוק ב-Copilot וב-Claude Code. תבקשו מהסוכן למחוק טיקט — אין לו עם מה. ואז
הדגמת ה-taint: אותה בקשת תגובה בדיוק נחסמת ברגע שהסשן נגע בתוכן לא-מהימן. הכול
נרשם ביומן שניתן לשחזור. ה-PoC בעצם לוקח את זה ומריץ על תרחיש שלנו.

---

## Slide 10 — Governability as an axis, a business, and a 4-week test

**Key message:** "How governable is an agent platform?" is a question no one owns
yet. A cheap PoC lets Radware test whether we should — with research, marketing,
and product upside even if the answer is "feature, not product".

- **Evaluation axis:** publish a *governability scorecard* for agent hosts/MCP
  stacks (does the host expose an enforcement seam? per-call hooks? drift
  detection?). Thought-leadership our research team can own.
- **Monetization paths (to be tested, not promised):** capability in API
  Protection / Agentic AI Protection · a governed MCP gateway offering · policy &
  audit as a compliance add-on (EU AI Act / audit-trail demand).
- **PoC plan:** wk 1–2 gateway on an internal MCP scenario + Radware-relevant
  policies; wk 3 red-team it (injection, taint, drift); wk 4 scorecard + go/no-go
  readout.
- **Exit criteria:** destructive actions unreachable under attack; one policy, ≥2
  hosts; full replayable audit; honest verdict incl. "not for us".
- **The ask, again:** 4 weeks, 1–2 engineers, existing codebase. Decision today is
  only: run the experiment.

**Suggested visual:** timeline bar (4 weeks, 3 phases → go/no-go diamond) next to a
mini scorecard mock (3 hosts × 4 governability criteria with ✓/✗).

**Speaker note (עברית):**
סוגר במקום שבו התחלתי — ההחלטה. שלושה דברים על השולחן: ציר הערכה חדש,
"governability", שאף אחד עוד לא מחזיק בו וצוות המחקר שלנו יכול לבעול; כיווני
מוניטיזציה שנבחנים ב-PoC ולא מובטחים; ותוכנית של ארבעה שבועות עם קריטריוני יציאה
ברורים, כולל תשובה כנה של "זה פיצ'ר ולא מוצר" או אפילו "זה לא בשבילנו". העלות היא
חודש-מהנדס; המחיר של לגלות את השכבה הזאת דרך ה-datasheet של פאלו אלטו גבוה בהרבה.
מה שאני מבקש היום זה רק לאשר את הניסוי.

---

*Sources for claims: Microsoft AGT positioning and the ABSENT/DENY, taint, and
manifest mechanics are documented in [`docs/THESIS.md`](../THESIS.md) §3, §5, §8;
the demo facts in slide 9 come from
[`docs/demos/jira-copilot/README.md`](../demos/jira-copilot/README.md) and
[`SCORECARD.md`](../demos/jira-copilot/SCORECARD.md). Verify current vendor
product names (Prisma AIRS, AgentCore, Entra Agent ID, NeMo) against public
announcements before presenting externally.*
