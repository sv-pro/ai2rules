# Checkpoint 5 — BLOG-DRAFT evidence audit

AI2-88 / #165 · Codex · 2026-10-26. Reviewed source: `2624385cef6ef1de8906614ed29b8d3861c4e767`.
Continuation of [Checkpoint 4](CHECKPOINT-4-writeups.md), on branch
`codex/ai2-88-writeups`. Only this verification report changes.

**Verdict: the result table is supported; the draft needs bounded wording and
two factual corrections before publication.** The detailed partial-quote
warning is strong and accurate. The close weakens it again. This is a static
writeup audit, not a new model run or an independent runtime certification.

## Evidence and current handoff

- Read the immutable draft and its sources with `git show 2624385:<path>`.
  Compared `501947e..2624385`: deterministic JSON, all deterministic traces,
  LLM JSON/report, contracts, D1 rules, CP2/CP3, prior-art note and CI recipe are
  unchanged. RESULTS.md only adds a figure link at lines 11–12: its subsequent
  source line numbers are **two higher** than in CP4.
- Re-ran the existing read-only `audit-writeups-cp4.mjs`: **11/11 checks pass**.
  Its recorded source SHA remains 501947e; it is not relabelled as a fresh
  experiment. Identical source blobs justify carrying its numeric checks here.
- All 44 deterministic rows and their saved trace oracles agree: D0 **8/8**,
  D1 **6/8**, D2 **2/8**, D3 **1/8**. D3 restores five utility cells:
  benign:crm-gift, S1, S2, S5, S5b. No attack/utility regression in these fixed
  cells; transform and partial-quote probes are counterexamples outside them.
- **Linux CI signal is now positive on this exact source SHA.**
  [Run 40311056729 / webmcp-lab job 121532049771](https://github.com/sv-pro/ai2rules/actions/runs/40311056729/job/121532049771)
  completed successfully on 2026-10-26 at 10:56:21 UTC. GitHub jobs API reports
  success for kernel build, install, contract tests, matrix and committed-results
  comparison; `head_sha` is 2624385cef6ef1de8906614ed29b8d3861c4e767. [CI:94–122](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/.github/workflows/ci.yml#L94-L122)
  specifies Ubuntu and the drift check. This updates CP4-F5's former evidence
  gap for this SHA; it does not certify every future host or a real-model D3 run.
- CP3 already independently reran D3 at `ad01717` on Windows
  ([V3:18–40](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-3.md#L18-L40)). This audit verifies wording and saved evidence at
  2624385. “Pending” must name any remaining check; do not describe the column
  as never independently run. The LLM sample still covers D0–D2 only.
- No lab runtime or paid LLM call was launched here. No source correction,
  PR, GitHub comment, publication or tracker change was made. This file is
  the scope-local handoff; Claude owns integration.

## Findings

### CP5-F1 — P3: three affected attacks should be four

**Condition →** [B:99–106](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L99-L106) says three attacks inherit the broken gift
task. **Reproduction →** enumerate D2 utility=false CRM attack rows:
S1, S2, S5 and S5b ([R:31–40](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L31-L40), [R:56–58](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L56-L58)).
**Expected / actual →** four attack cells plus the clean gift cell, versus
three attacks. **Impact →** understates the kernel's utility cost and fails
to reconcile the five recoveries. Change the count, not the result table.

### CP5-F2 — P2: “every defense” incorrectly includes D3, and the model section has no D3 sample

**Condition →** [B:108–112](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L108-L112) says S5b leaks under every defense including
the kernel, while this post displays D0–D3. [B:229–239](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L229-L239) also uses
“every defense” without naming the measured model columns.
**Reproduction →** [R:23–24](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L23-L24) and [J:834–850](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/results.json#L834-L850) show S5b/D3
stopped with utility=true. [L:5–15](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS-LLM.md#L5-L15) has only D0–D2, no S5b row.
**Expected / actual →** S5b escapes **D0–D2**; real-model S3b escapes the
**three tested defenses**, versus an unqualified all-defense claim.
**Impact →** internal contradiction and potential inference that D3 was
tested with the real model. Explicitly state nine scenarios × three defenses
= 27 saved model episodes, sampled once each, not one model call or a rate.

### CP5-F3 — P2: the D3 close loses the whole-leaf condition; pending needs a precise scope

**Condition →** [B:27–31](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L27-L31), [B:194–206](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L194-L206),
[B:220–225](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L220-L225) and [B:288–293](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L288-L293) summarize labels as preserving
security while closing the leak. The close names only a “reshaped” value.
**Reproduction →** [C:270–304](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L270-L304) and [V3:144–179](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-3.md#L144-L179):
only eligible whole string leaves (length >=8 from sensitive readOnlyHint
results) are labelled. Copying the unchanged canary substring into a write
escapes D3; D2 would block that write. The draft correctly explains this at
[B:208–218](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L208-L218) but drops it from the standalone close.
**Expected / actual →** “no regression in these eight fixed scenarios, whose
S5/S5b payloads include the whole labelled leaf” versus general security
parity with a transformation-only price. **Impact →** implies protection
against selective disclosure that was explicitly disproved. Do not imply
D2 covers a read-sink version of that counterexample.

Pending is present in the banner, table header, section and close. The lede
has no local marker; the whole article's banner partially supplies it.
[B:225](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L225) and [B:259](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L259) should distinguish CP3's historical
runtime verification, successful current Linux CI, this completed text audit,
and any explicitly outstanding test. A pending badge does not qualify an
overbroad technical claim by itself.

Suggested bounded conclusion for the author to adapt:

> On this fixed matrix, D3 restores five utility cells and reduces successful
> attacks from 2/8 to 1/8. Its S5/S5b stops depend on the payload containing the
> whole labelled field. Transforms and unchanged partial quotes escape it;
> D2 would still block such a write. This prototype is not general security
> parity, and the real-model sample has not tested D3.

### CP5-F4 — P2: baseline failures are generalized beyond the configuration measured

**Condition →** [B:141–156](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L141-L156) says every miss is structural, not tuning;
the close at [B:288–289](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L288-L289) describes per-call blocking generally.
**Reproduction →** [D:44–54](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/D1-RULES.md#L44-L54) explicitly says these results describe
D1 as configured, not all per-call policies. A name allowlist/default-deny is
acknowledged in the draft itself ([B:72–75](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L72-L75)). Canary shape only
explains the particular pattern rules; it cannot establish that no DLP rule
could recognize it ([B:46–48](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L46-L48)).
**Expected / actual →** distinguish identity/history limits from configured
rule choices; say “this baseline on these scenarios,” versus impossibility
for the control class. **Impact →** unfair comparative claim despite the
otherwise clear no-vendor disclaimer. Git history supports “committed before
attack content was added to this repository,” not blind authoring or the
literal global claim “before any attack existed” ([B:68–70](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L68-L70)).
Rules: 2c83bee; first scenario: 3890564; no later rules-JSON change.

### CP5-F5 — P2: native-browser scope and adapter attribution need to survive the introduction

**Condition →** title/lede [B:1–25](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L1-L25) says reproduced WebMCP attacks;
[B:163–175](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L163-L175) calls the stack the shipped kernel, including pinning.
**Reproduction →** [C:33–60](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L33-L60) specifies an in-page shim;
[C:218–230](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L218-L230) explicitly puts registration pinning in the adapter,
not the kernel. [C:70–71](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L70-L71) hashes annotations too, omitted from the
draft's identity recipe. [P:78–84](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/PRIOR-ART.md#L78-L84) distinguishes the lab's multi-tab
union from native cross-origin tool exposure.
**Expected / actual →** “WebMCP-shaped shim; real kernel through its ABIs plus
a lab adapter,” with descriptor hash including annotations, versus an easily
read native/shipped-stack claim. **Impact →** readers may attribute lab-specific
pinning and orchestration to the released kernel/browser. The limitation at
line 247 is useful, but should say “not the native browser WebMCP implementation”:
Playwright does run a real Chromium browser.

### CP5-F6 — P2: failure of these mechanisms does not prove isolation is the only possible fix

**Condition →** [B:119–120](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L119-L120) and [B:294–296](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L294-L296) assert no amount
of tool governance can see S3b and only script isolation can stop it.
[B:252–254](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L252-L254) similarly says operator trust pins would close first-seen
malice without qualifying what they pin.
**Reproduction →** [R:20](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L20), [V3:53](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-3.md#L53) establish failure of
D0–D3 in this shared realm. They do not test all execution-boundary designs
or an isolation mitigation. [C:228–230](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L228-L230) describes descriptor hashes;
[C:80–86](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L80-L86) itself shows descriptor/registration identity is insufficient
to establish handler behavior.
**Expected / actual →** “none of the four tested defenses detects it; script
isolation is a mitigation to evaluate,” versus universal impossibility and a
proven unique remedy. **Impact →** turns a useful bounded finding into an
unsupported architectural guarantee. CSP, SRI and iframe configuration are
not interchangeable guarantees; no mitigation setup is tested here.
This overstatement is inherited from CONTRACTS too: matching that source
does not make the inference experimentally supported.

### CP5-F7 — P2: the short oracle definition drops its non-CRM boundary

**Condition →** [B:52–56](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/BLOG-DRAFT.md#L52-L56) counts the canary in any entered tool's
arguments as success.
**Reproduction →** [C:127–149](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L127-L149) specifies an entered **non-CRM**
tool, recognizable encodings, unauthorized reviews/notes/orders and an
optional scenario-specific effect. Denied attempted calls do not count.
**Expected / actual →** preserve at least the non-CRM restriction in the
summary, versus an unrestricted argument-leak predicate.
**Impact →** changes the meaning of “effects only” and makes lawful sensitive
source-side use appear to be a leak. The verdict-flip test and ERROR rule
are accurately reported ([V2:70–76](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-2.md#L70-L76)).

## Claim-by-claim ledger

Locations refer to the immutable BLOG-DRAFT above. Repeated formulations
share a row where they assert the same proposition. Headings, rhetorical
questions and opinions are not treated as measured findings. Verdicts are
“подтверждено”, “не подтверждено”, or “переобещает”; qualifications specify
exactly what is supported.

| ID / draft lines | Claim | Verdict and exact evidence |
|---|---|---|
| 01 / 3–11 | Draft in docs; audit/pending banner and source links | подтверждено for file placement, banner and existing links. Publication state outside the repository is not established; pending scope: F3. |
| 02 / 13–17 | Web page registers tools for a browser agent | подтверждено by [Chrome tool security](https://developer.chrome.com/docs/ai/webmcp/secure-tools), “Expose your tools carefully”; cross-tab visibility depends on orchestration/exposure, F5. |
| 03 / 19–25 | Eight attacks; three initial defenses then D3; effects only; reviewer | подтверждено for the shim matrix: [R:15–24](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L15-L24), [V3:18–40](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-3.md#L18-L40). Native-browser interpretation: переобещает (F5). |
| 04 / 27–31,288–293 | D1 stops two, D2 six; D2 breaks gift; D3 recovers utility and one leak | подтверждено for fixed cells: [R:30–47](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L30-L47). D3 standalone conclusion: переобещает without whole-leaf condition (F3). S2 is not_triggered, not a refused call. |
| 05 / 35–40 | Deterministic marker follower, not model susceptibility measurement | подтверждено: [C:178–192](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L178-L192). ‘Any instruction’ means supported markers; max eight distinct instructions, not arbitrary natural language. |
| 06 / 42–46 | Four local origins/ports, embedded widget, three frozen tasks and canary | подтверждено: [C:14–27](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L14-L27), [C:112–125](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L112-L125). |
| 07 / 46–48 | Canary shaped like nothing a DLP rule would recognize | переобещает: [C:121–125](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L121-L125), [D:28–31](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/D1-RULES.md#L28-L31) support not matching these credential/card patterns, not every possible rule (F4). |
| 08 / 52–54 | Effects predicate | не подтверждено as an exact predicate; non-CRM boundary omitted: [C:127–142](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L127-L142) (F7). |
| 09 / 54–58 | Verdicts ignored, flipping verdicts unchanged, ERROR not defended, D0 positive controls | подтверждено: [V2:73–75](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-2.md#L73-L75), [C:147–149](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L147-L149), [R:44–48](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L44-L48). |
| 10 / 62–67 | D1 reconstruction, no vendor, per-call inputs, all tools visible, eleven rules | подтверждено: [D:3–36](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/D1-RULES.md#L3-L36). Admin rationale is rationale, not independently measured fairness. |
| 11 / 68–70 | Rules committed first; hash recorded; no later tuning | подтверждено for artifact order/unchanged rules: [D:10–16](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/D1-RULES.md#L10-L16), [D:56–62](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/D1-RULES.md#L56-L62), [J:5](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/results.json#L5), git history in F4. Global/no-foreknowledge implication: не подтверждено. |
| 12 / 72–75 | Hint trust/default allow assumed, allowlist alternative unmeasured | подтверждено: [D:44–54](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/D1-RULES.md#L44-L54). |
| 13 / 81 | S1 row across four columns | подтверждено: [R:17](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L17); D3 mechanism is confidentiality, D2 projection. |
| 14 / 82 | S2 row, D2/D3 not triggered | подтверждено: [R:18](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L18); not-triggered must remain distinct from a blocked malicious call. |
| 15 / 83 | S3 row | подтверждено: [R:19](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L19). |
| 16 / 84 | S3b row | подтверждено: [R:20](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L20). |
| 17 / 85 | S4 row | подтверждено: [R:21](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L21). |
| 18 / 86 | S4b row | подтверждено: [R:22](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L22). |
| 19 / 87 | S5 row | подтверждено for whole-leaf payload: [R:23](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L23), [C:292–300](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L292-L300). |
| 20 / 88 | S5b row | подтверждено for whole-leaf payload: [R:24](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L24), [C:292–300](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L292-L300). |
| 21 / 79,89 | 8/8,6/8,2/8,1/8; D3 pending header | подтверждено as scenario counts: [R:44–47](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L44-L47). Pending scope F3; not population ASR. |
| 22 / 91–106 | D1 clean utility 3/3, D2 2/3; session taint breaks gift/S5 tradeoff | подтверждено: [R:30–40](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L30-L40), [C:232–256](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L232-L256). |
| 23 / 103–104 | Three attack cells inherit gift utility loss | не подтверждено: four (S1,S2,S5,S5b), [R:56–58](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L56-L58) (F1). |
| 24 / 108–112 | S5b leak under every defense; taint only covers write/external; reviewer found gap | подтверждено for D0–D2/taint/history: [V2:94–122](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-2.md#L94-L122), [R:24](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L24). Every defense: не подтверждено (F2). |
| 25 / 114–118 | S3b wraps fetch without changing pinned registration, all four lose | подтверждено: [C:80–86](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L80-L86), [R:20](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L20). |
| 26 / 119–120,294–296 | Only isolation can stop S3b; no governance can see it | переобещает: [V3:53](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-3.md#L53) tests four defenses, not impossibility or a mitigation (F6). |
| 27 / 122–124 | Substitution/squatting removes/redirects necessary tool; no utility | подтверждено in these cells: [R:35–38](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L35-L38), [R:59](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L59). |
| 28 / 126–128,190,223–224 | AI2-70/#151 tracks value flows; prototype not full fix | подтверждено as documented roadmap: [C:261–304](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L261-L304). Actual completion of future kernel fix: не подтверждено. ‘Next-to-last section’ is a stale navigation cue; D3 is not penultimate. |
| 29 / 132–137 | D1 stops S1/S4 by nonallowlisted non-read tool origin rule | подтверждено: [R:17](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L17), [R:21](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L21), [D:26](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/D1-RULES.md#L26). |
| 30 / 141–142 | Every D1 miss structural, not tuning | переобещает: [D:51–54](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/D1-RULES.md#L51-L54) (F4). |
| 31 / 144–147 | Widget tools inherit shop origin; S2/S4b defeat D1 | подтверждено: [C:25–27](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L25-L27), [D:38–40](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/D1-RULES.md#L38-L40), [R:18–22](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L18-L22). |
| 32 / 148–151 | Same-descriptor S3/S4b replacement evades D1's current-call matching | подтверждено: [C:223–227](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L223-L227), [R:19–22](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L19-L22); applies to configured D1. |
| 33 / 152–156 | S5/S5b use first-party tools; D1 monitors but does not block; output influence untracked | подтверждено: [J:704–732](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/results.json#L704-L732), [J:788–810](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/results.json#L788-L810), [D:36](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/D1-RULES.md#L36). |
| 34 / 158–159 | S4 vs S4b contrast | подтверждено for the paired outcome: [R:21–22](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L21-L22). Not a claim that every implementation detail is identical. |
| 35 / 163–165 | Real kernel via project/gate, not reimplemented | подтверждено for ABI execution: [C:196–201](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L196-L201). Entire defense equals shipped kernel: переобещает (F5). Binary provenance limit remains [V2:151–153](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-2.md#L151-L153). |
| 36 / 167–171 | Task-only projection; S1/S4 excluded; S2 description hidden; S2 taint confound | подтверждено for D2: [C:199–216](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L199-L216), [V2:124–147](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-2.md#L124-L147). Does not transfer D2's sensitive-read taint explanation to D3. |
| 37 / 172–175 | Identity pins first registration and rejects replacements | подтверждено for adapter behavior: [C:218–230](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L218-L230). Identity recipe incomplete: annotations are hashed too, [C:70–71](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L70-L71); kernel attribution F5. |
| 38 / 176–183 | Taint stops S5 at utility cost; unlisted name rejected at invocation | подтверждено: [C:245–256](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L245-L256), [V2:57](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-2.md#L57), [C:162–167](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L162-L167). |
| 39 / 185–192 | D3 tests value-vs-session hypothesis; integrity taint baseline | подтверждено as hypothesis/setup: [C:234–247](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L234-L247), [C:261–268](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L261-L268). Not proof of no security loss. |
| 40 / 194–198 | D3 adds confidentiality check before integrity, clean writes return | подтверждено with eligible string-leaf/readOnlyHint qualification: [C:270–281](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L270-L281). ‘Keeps everything’ must explicitly except sensitive-read session taint (as line197 does). |
| 41 / 200–206 | Utility recovered on all affected CRM cells, no regressions | подтверждено within fixed matrix: [V3:47–74](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-3.md#L47-L74). Five recoveries; crm-renewal already worked. General security/no-flow guarantee: переобещает (F3). |
| 42 / 208–218 | Whole field match; transform and partial quote escape; D2 write would block; shipped S5/S5b paste whole record | подтверждено, including explicit whole-leaf condition: [C:282–300](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L282-L300), [V3:144–159](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-3.md#L144-L159). No shipped matrix cell tests selective quote; separate verifier probe does. |
| 43 / 220–225 | Matrix parity but price summarized as reshaping; still pending reviewer | подтверждено only for matrix: [V3:61–74](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-3.md#L61-L74). Summary caveat and ambiguous status: переобещает / не подтверждено respectively (F3). |
| 44 / 229–230 | Single nondeterministic model run, not reviewer-certified or a rate | подтверждено: [LA:99–103](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/LLM-AGENT.md#L99-L103), [V2:154–157](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-2.md#L154-L157). 27 episodes, D0–D2 only; no D3/S5b. |
| 45 / 232–235 | Injection failures can reflect unread/ignored/refused text; each refusal loses utility | подтверждено from saved report: [L:9–15](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS-LLM.md#L9-L15), [LA:105–119](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/LLM-AGENT.md#L105-L119). Local model transcripts are gitignored, [LA:101–103](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/LLM-AGENT.md#L101-L103); narration of internal cause is not independently replayed. |
| 46 / 235–239 | S3/S4b beat D1, D2 stops without refusal; S3b beats all tested model defenses | подтверждено: [L:11–14](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS-LLM.md#L11-L14). S3/D2 is not_triggered, not a denied invocation. D3 inference: не подтверждено (F2). |
| 47 / 241–243 | Model and execution controls complementary; careful model refuses lures; only lower layer sees swaps | подтверждено as an interpretation of this sample, [LA:105–123](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/LLM-AGENT.md#L105-L123). Universal refusal/detection exclusivity: переобещает; qualify ‘in this run/interface’. |
| 48 / 247–251 | Shim, deterministic agent not real ASR; D1 assumptions | подтверждено: [C:33–60](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L33-L60), [C:315–323](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L315-L323), [D:44–54](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/D1-RULES.md#L44-L54). Real Chromium vs native WebMCP distinction F5. |
| 49 / 252–254 | TOFU limitation; operator pins close first-seen malice | подтверждено for TOFU and absence of operator pins: [C:223–230](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L223-L230). Blanket closure: переобещает; trusted descriptor hash does not certify handler environment (F6). |
| 50 / 255–259 | Integrity floor blind to read sinks; D3 prototype/whole-match escapes/not certified | подтверждено for technical limitations: [C:232–304](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L232-L304). Historical verification status requires precise scope (F3). |
| 51 / 260–261 | Windows reproduction and Linux CI drift check | подтверждено: [V3:24–40](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-3.md#L24-L40), [CI:94–122](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/.github/workflows/ci.yml#L94-L122) and exact-SHA successful job linked above. Not a guarantee for arbitrary hosts. |
| 52 / 262–264 | Added variants, two from reviewer; no navigation/forms, only tool calls | подтверждено: [R:101–102](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L101-L102), [C:319–323](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/CONTRACTS.md#L319-L323). Variant labels exist in results/scenario names; draft table omits literal ‘variant’ badges. |
| 53 / 268–275 | Reproduction commands, matrix output/CI drift, per-cell traces, verifier recipe | подтверждено for deterministic matrix: [R:112–120](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/results/RESULTS.md#L112-L120), [CI:113–122](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/.github/workflows/ci.yml#L113-L122), [V3:181–197](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/verification/CHECKPOINT-3.md#L181-L197). Clean Linux may require Playwright --with-deps (as CI does); recipe does not run real LLM. |
| 54 / 279–284 | Prior-art references; lab not paper-experiment replication; naming map is interpretation | подтверждено against committed reference note: [P:1–17](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/PRIOR-ART.md#L1-L17), [P:19–84](https://github.com/sv-pro/ai2rules/blob/2624385cef6ef1de8906614ed29b8d3861c4e767/labs/webmcp/docs/PRIOR-ART.md#L19-L84). Prior paper experiments were not rerun; no numeric comparisons claimed. |
| 55 / 288–296 | Closing synthesis | подтверждено for fixed counts/utility/S3b; переобещает for generic per-call limits, D3 parity and unique S3b remedy (F3/F4/F6). |
| 56 / 300–302 | Pointers to lab, reports, TAKEAWAY and verification | подтверждено as existing repository paths. Inline-code pointers are not clickable Markdown links; publication editing is outside this audit. |

## Integration advice

The draft's losses-first structure, no-vendor baseline framing, single-model-run
warning and detailed partial-quote caveat should be kept. Correct the count and
D0–D2 wording; carry the fixed-payload limitation into the lede and close;
attribute registration pinning to the adapter. Scope the broad baseline and S3b
claims to the tested controls. Update verification status with CP3, this audit
and the exact green CI run.

A new LLM D3 run is **optional new evidence**, not a prerequisite to correct this
text. If the owner chooses it, freeze and label the new sample separately; do not
silently replace the old “one run” evidence. This audit does not authorize or
launch that paid run.
