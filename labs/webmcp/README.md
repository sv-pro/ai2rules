# WebMCP attack lab

A deterministic lab that measures which WebMCP attacks get past **per-call
rule-based controls** and which get past **ai2rules' compiled world + taint**
(ALLOW / DENY / ABSENT: "Some actions are denied. Others do not exist."). Epic:
[#160](https://github.com/sv-pro/ai2rules/issues/160) (AI2-83).

The agent is assumed compromised. The default agent is a deterministic
CompromisedAgent that obeys every instruction it reads. The lab asks what the
execution layer lets through, and judges it only by observed effects.

- Contracts — what the lab promises, and what the tests pin: [`CONTRACTS.md`](CONTRACTS.md)
- D1 rules and why an admin would write them: [`docs/D1-RULES.md`](docs/D1-RULES.md)
- LLMAgent stretch column (a real model on a subset, `--agent llm`): [`docs/LLM-AGENT.md`](docs/LLM-AGENT.md)
- Write-ups (pending verification): one-page takeaway [`docs/TAKEAWAY.md`](docs/TAKEAWAY.md) · blog outline with a claims checklist [`docs/BLOG-OUTLINE.md`](docs/BLOG-OUTLINE.md) · blog draft [`docs/BLOG-DRAFT.md`](docs/BLOG-DRAFT.md)
- Results (generated): `results/RESULTS.md`, `results/results.json`, `results/traces/`

## Run

Needs Node ≥ 22. Playwright downloads its own Chromium on first install. D2 needs
the `harness` binary from this repository.

```bash
cargo build --release -p cli-harness        # from the repo root, for D2
cd labs/webmcp
npm install && npx playwright install chromium
cp .env.example .env                        # optional: API key, ports (gitignored)
npm test                                    # contract tests
npm run matrix                              # every scenario × D0/D1/D2
npm run cell -- benign:shop-buy D2          # one cell; trace in results/traces/
npm run demo                                # live demo page on http://127.0.0.1:4180
```

**The demo** (`demo/`) is a page for showing the lab to people: pick a defense,
run S5b (or S5, or the no-attack baseline) and watch the agent's steps arrive:
the tabs it opens, the tools it calls and why, each verdict, what actually ran,
and the oracle's result. It runs the same `runCell` as the matrix, against the
same servers and the real `harness` binary, and only renders the trace records
as they are written. Its traces go to a temp directory, never to `results/`.
`WEBMCP_DEMO_PORT` changes its port.

## Layout

```
src/shim/            document.modelContext shim (browser-side)
src/servers.js       shop / crm / attacker / widget origins
src/browser.js       Playwright: tabs, listing, execute-by-registration
src/env.js           the agent's interface; every call goes through the defense
src/agent/           CompromisedAgent · LLMAgent (stretch column)
src/defenses/        D0 none · D1 rule-based baseline · D2 ai2rules (real kernel) · D3 ai2rules + the kernel's flow contracts (labels carried by the adapter)
src/tasks.js         the benign tasks (frozen)
src/oracle.js        effect-based judging
pages/<origin>/      benign pages
scenarios/<id>/      one attack each: scenario.js + pages/<origin>/ overlays
verification/        the independent verifier's checks (#165)
```
