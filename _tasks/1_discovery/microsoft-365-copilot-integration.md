# Discovery: can ai2rules govern anything in a Microsoft 365 Copilot shop?

**Opened 2026-10-21. Research item: nothing here is verified yet.** It came out of a
go-to-market conversation. Law and accounting firms are a natural audience for the
law-firm demo (`harness demo --story law-firm`), and most of them run **Microsoft 365
Copilot**. If ai2rules cannot touch Copilot at all, the pitch to those firms has to be
something else.

## What the 2026 surveys say (sources at the end)

- ILTA 2026 (500+ law firms): **76% use Microsoft 365 Copilot**, by far the leading tool.
  Legal-specific tools trail (2025 figures: CoCounsel 36%, Harvey 33%).
- Small firms lean on general chat tools instead: about a third of firms with 50 or fewer
  lawyers use public ChatGPT (ILTA 2025), and Clio reports 54–57% of solo and small-firm
  lawyers use general-purpose AI.
- CPA firms: 56–91% use generative AI depending on the survey, but only about 10% have it
  integrated across operations, and only 57% have a written AI policy people were trained
  on.

## Working hypothesis

**Core Copilot is closed.** There is no host hook comparable to Claude Code's
`PreToolUse`, so ai2rules cannot sit inside Copilot's own reasoning-to-action path.
Microsoft governs it with its own controls: Purview sensitivity labels, DLP for Copilot,
SharePoint and OneDrive permissions, and the admin center. **Do not position ai2rules
against Copilot.**

The seams worth investigating are what firms **attach to** Copilot:

1. **Copilot Studio agents and their tools.** Believed (verify): Copilot Studio can call
   MCP servers as agent tools. If so, `harness mcp-gateway` can front the firm's own
   systems (document management, practice management such as Clio, mail, accounting).
   Copilot stays closed, but what its agents do to client data passes through the kernel.
2. **Declarative agents, API plugins and Graph connectors.** Which of these execute
   actions against systems we could front, and which only read?
3. **The boundary with Purview.** Purview answers *which data* may be touched. It does not
   model *what may happen after the agent has read untrusted text*, which is the taint
   invariant and the law-firm demo's scenario. Confirm this gap precisely before claiming
   it.

## Why it matters for the pitch

**EchoLeak** (verify details; believed CVE-2025-32711, disclosed June 2025): a zero-click
issue in M365 Copilot where a crafted email led Copilot to exfiltrate data. Microsoft
patched it, but the class remains: an agent with mail and tool access reads untrusted text.
This is the law-firm demo's story, with the buyer's own product as the precedent.

## Product angles to evaluate

- **Gateway for Copilot agents:** `mcp-gateway` in front of the systems Copilot Studio
  agents act on (depends on seam 1).
- **Service, not software:** an AI governance assessment for Copilot firms.
  - oversharing review (Copilot surfaces whatever the user can reach);
  - Purview labels;
  - a written AI-use policy;
  - staff training, using the demo as the teaching material.

  The world manifest is the policy-as-code deliverable that later governs their agents.
  This earns revenue and trust while the product matures.
- **Shadow AI:** staff still paste client documents into personal ChatGPT. A
  prompt-redaction proxy (pseudonymize sensitive spans before any model sees them) fits
  here. That is a separate idea, not yet recorded.

## Questions to answer

- [ ] Does Copilot Studio support MCP servers as agent tools today? With which transport,
      authentication, and admin controls? Is anything comparable to a pre-call hook exposed?
- [ ] For declarative agents, API plugins and Graph connectors: which perform write
      actions, and through which call path could a gateway sit?
- [ ] What exactly does Purview DLP for Copilot block, at prompt time and at response
      time? Is there any notion of information flow ("after reading X, may not send to Y")?
- [ ] EchoLeak: confirm the CVE, timeline and mechanism from primary sources.
- [ ] Is there a Microsoft-sanctioned interception point (an Entra-registered proxy or
      gateway for agent tool calls) that a third party can use?

## Done when

A short memo with three answers, each backed by primary sources:

1. **Where ai2rules can sit:** concrete integration points, or "nowhere", with reasons.
2. **What we would demo:** for example the law-firm story with a Copilot Studio agent
   through `mcp-gateway`, if seam 1 holds.
3. **What we sell to a Copilot-only firm:** product, service, or both.

Then promote the result to `PLAN.md` (a new epic or an E16 skin) or close it as "service
only".

## Sources (from the 2026-10-21 search; not yet read in full)

- [LawNext: ILTA 2026 tech survey](https://www.lawnext.com/2026/09/which-ai-product-is-most-popular-among-law-firms-findings-of-iltas-tech-survey-may-surprise-you.html)
- [ILTA 2026 executive summary](https://higherlogicdownload.s3.amazonaws.com/ILTANET/ce7f3e74-fb70-402e-a1b3-5dc0abe72260/UploadedImages/Misc%20PDFs/2606_ILTA_Executive_Summary_FINAL.pdf)
- [NC Bar: by the numbers](https://ncbar.org/nc-lawyer/2026-05/by-the-numbers-what-surveys-show-about-law-firm-ai-adoption)
- [Ramp & CalCPA: Benchmarking the Modern CPA Firm 2026](https://ramp.com/reports/benchmarking-the-modern-cpa-firm-2026-or-calcpa-and-ramp)
- [CPAFMA 2026 survey](https://winsomemarketing.com/resources/professional-services-marketing-ai-governance-shocks-in-the-2026-cpafma-digitally-driven-firm-survey)

## Related

- `docs/demos/law-firm/`: the demo this research would extend to Copilot shops.
- `taint-scope-on-the-hook-path.md`: needed before any host integration is usable day to
  day.
- PLAN E16: the JIRA MCP demo on Copilot (GitHub Copilot) and Claude Code via the gateway.
  That is the closest existing precedent, but a different product from M365 Copilot.
