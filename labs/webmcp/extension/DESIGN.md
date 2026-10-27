# The WebMCP lab in your own browser: an MV3 extension (design + spike)

**Status:** spike, mode (a) only (AI2-96). Loads unpacked; nothing is submitted to
a store. The plan for modes (b) and (c) is AI2-97.

The WebMCP lab ([`../README.md`](../README.md)) shows the kernel governing browser
agents, but only on our machine: Playwright drives a Chromium, and the kernel is the
`harness` binary. The goal here is a demo anyone can run without booking a session.
They install an extension, open a page, press a button, and watch the kernel refuse
an attack in their own browser. The spike shows the core of that works today:

- **D0:** the hijacked model leaks the CRM record to the shop's server.
- **D3:** the kernel, compiled to WebAssembly and running inside the extension,
  refuses the injected call before the page's handler runs. The task still
  completes.
- **Parity:** every verdict on the way equals the lab's committed trace for the same
  cell, which the native binary produced.

![The side panel after a D3 run: the injected search is refused by the kernel's confidentiality rule, the order still goes through](docs/panel-D3.png)

---

## 1. Shape

```
 page (MAIN world)              page (ISOLATED world)        extension
 ─────────────────              ─────────────────────        ─────────
 hook.js                        bridge.js                     background.js  (service worker)
  document.modelContext   ◄──►   carries messages     ◄──►     kernel: harness_wasm gate / project
  registry + handlers            reads page text               adapter.js: projection, pinning,
  governed(): ask, then run      reads the sim log                 taint, flow labels (the lab's D2/D3)
                                                               registry of every tab's tools
                                                               broker: open / read / list / call
                                                                        ▲
                                                               sidepanel.js  (the agent + the UI)
                                                                 CompromisedAgent (the lab's file)
```

There are three roles, kept apart on purpose:

- **The hook** owns the interception point. It runs no policy.
- **The service worker** is the governor. It holds the kernel and all session state,
  and it is the only place that decides.
- **The side panel** is one agent among possible others. It decides nothing.

## 2. The interception point

The lab's interception point is its Playwright shim. Here it is `src/hook.js`, a
content script declared with `"world": "MAIN"` and `"run_at": "document_start"`. It
runs in the page's own JavaScript realm before any page script, and it owns
`document.modelContext.registerTool`.

- **Every handler runs only through `governed()`.** The hook stores each
  registration's handler and invokes it only from `governed(entry, args)`.
  `governed()` asks the service worker for the kernel's verdict on *this
  registration* with *these arguments*, and runs the handler only on `ALLOW`. A
  refusal never enters the handler, and the result reports `entered: false`, as the
  lab's shim does (verifier finding F3).
- **The gate does not depend on the caller.** The side panel's agent reaches a tool
  through the worker, which asks the hook to execute it, and the hook asks the
  worker for a verdict. A browser-provided agent goes through the same function, see
  the next point.
- **When the browser provides WebMCP itself**, the hook wraps it. Chrome now ships
  an early preview of WebMCP (`document.modelContext.registerTool`, `getTools`,
  `executeTool`, unregistering by `AbortSignal`), currently in an origin trial and
  subject to change. When it is present, the hook shadows `registerTool` on the
  instance. The browser then receives the page's descriptor with `execute` replaced
  by `governed()`, so the browser's own `executeTool`, and any agent that calls
  through it, meets the gate. This branch is written but **untested**: no browser
  the spike's test runs in has the preview enabled.
- **The page cannot forge verdicts.** The hook shares its realm with page scripts,
  so anything on `window` (postMessage, DOM events) can be observed and forged by
  the page. The hook creates a *detached* DOM node and hands it to the bridge
  (ISOLATED world, declared first, already listening) through one synchronous event
  at document_start, before any page script exists. From then on the two talk only
  through events dispatched on that node, which no page script can reach. Built-ins
  are captured first, as in the lab's shim (verifier finding F1). The spike's test
  floods `window` with forged handshakes and forged `ALLOW` replies while the agent
  calls a tool: the handler does not run, and the call is ABSENT as the kernel says.
- **Fail-closed:** if the bridge never connected, `governed()` refuses
  (`no_governor`).

### The limits of an in-page hook

These are stated, not hidden:

- **The handler's environment.** A same-origin script can change what a handler
  does (its closures, `fetch`, globals) without touching the registry. This is the
  lab's S3b, unchanged.
- **Registering around the hook.** With native WebMCP, a hostile same-origin
  script could call the prototype's original `registerTool` (for example, from a
  fresh same-origin iframe realm) and register a tool the hook never saw. For the
  agents this extension runs (modes a–c), that changes nothing: they execute only
  through the extension's registry. For the browser's own agent, the hook's gate
  covers the tools registered through it. A complete gate for that agent would
  need a browser API ("ask this extension before executing a WebMCP tool"), which
  no browser offers.
- **Only WebMCP calls are governed.** Clicks, typing, form submissions and
  navigation are not WebMCP calls, so the kernel never sees them. See §7.

## 3. The kernel in WASM

`crates/harness-wasm` already compiled the real compiler and kernel to WebAssembly
for the blog's playground, but it exported only `preview`, `default_world` and
`version`. The spike adds the two runtime operations:

| export | the native operation it is | input → output |
|---|---|---|
| `gate(world_yaml, request_json)` | `harness gate --world` (gate ABI, D24) | `GateRequest` → `GateResponse`, or `{ok:false,error}` where the CLI exits non-zero |
| `project(world_yaml, request_json)` | `harness project --world` (D72) | projection request → response, or `{ok:false,error}` |

Both are thin shims over `harness_preview::{gate, project}`, the same functions the
CLI calls, so there is no second implementation. The world is manifest YAML,
compiled on every call, as `--world` is read on every invocation. For the lab's
worlds, which have a handful of actions, that was not noticeable in the spike; it
was not measured.

**Parity, three ways:**

1. **Native/WASM parity.** `crates/cli-harness/tests/one_kernel.rs` gains entry
   point **(g)**, the WASM `gate` export compiled for the host. It must return the
   *full* response the `harness gate` CLI returns, on every case in `cases.yaml` and
   `flow-cases.yaml` (flow sidecar included). `tests/project.rs` holds `project` to
   the CLI the same way, error shape included.
2. **Freshness.** `scripts/check-wasm-freshness.mjs` loads the committed wasm32
   artifact and a fresh build. It requires them to answer four `gate`/`project`
   calls identically, besides the existing preview cases, and it fails if the
   committed artifact has no `gate` export at all. Before this change was copied
   in, it failed with exactly that.
3. **End to end.** The spike's test compares the verdicts the extension's kernel
   gives in a real browser with the lab's committed native traces (§9).

**Roots are out of scope.** The native adapter resolves and canonicalizes `roots`
paths against the filesystem before compiling. A browser has no filesystem, so the
WASM engine compiles the manifest as written. A world with relative or symlinked
roots would answer differently. That is why the parity suite covers the pathless
case sets, as the MCP gateway's does.

**Size.** The artifact grows from 660 KB to 790 KB (220 → 272 KB gzipped), because
of the request/response (de)serialization behind the two new exports. It is still
one artifact, shared by the playground and the extension, under one freshness
check. DECISIONS D96 records the choice against a second, extension-only build.

## 4. Where the adapter state lives

All of it lives in the **service worker**, as one plain object:

- the registry of every tab's live registrations, with their identities;
- the adapter's pins, the session taint, and the flow ambient;
- the digest-keyed value labels;
- the trace.

The worker is the one place the CRM tab's read and the shop tab's call meet, which
is the whole point of S5/S5b: the session spans tabs.

- **Restarts.** MV3 stops a worker after 30 s of inactivity. Every hook therefore
  writes the object to `chrome.storage.session`: in memory, never on disk, and
  cleared when the browser closes. The worker reloads it on wake. During a run, the
  side panel's long-lived port keeps the worker alive (Chrome 114+).
- **The adapter.** `src/adapter.js` is the lab's `d2-ai2rules.js` on `kernel.gate` /
  `kernel.project` instead of a child process. The differences are mechanical:
  state in one serializable object, WebCrypto digests, and post-call state keyed by
  a call id, because the verdict and the result arrive as two messages. The worlds
  are not re-implemented. `build.mjs` evaluates the lab's own `worldYaml(task)` and
  ships the YAML as data, and a test holds the two equal.
- **Identity.** Each registration's identity is
  `t<tab>.<document generation>:<seq>`, with the generation advancing when
  Chrome's `documentId` changes. The origin is taken from the message sender
  (`sender.origin`, set by the browser), never from anything the page says, and is
  mapped to the lab's origin names. An origin the demo does not know is `unknown`,
  which no world declares.

## 5. The demo pages

The spike does not copy the lab's pages. `serve.mjs` serves `pages/` and
`scenarios/<id>/pages/` with the same layering as `src/servers.js`, and injects one
script as each page's first: `demo/sim-server.js`, a **simulated server**. It answers
the page's own `fetch('/api/…')` in the page, from the lab's data (`CATALOG`,
`CRM_RECORDS` with the canary, `SHOP_ACCOUNT`) and with the lab's routes. Each origin
holds only its own data, so the CRM records never reach the shop's page.

Every request the simulated server receives is logged in a panel attached to
`<html>`, outside `<body>`, so the agent's read of the page text does not see it.
The log is what the demo judges a leak by: what the shop's server would have
received.

![The shop tab after a D0 run: the simulated server's log shows the CRM record, account code included, arriving in a search query](docs/shop-D0.png)

**Hosting on ai2rules.dev.** With no backend left, the pages are static files. The
one requirement is **distinct origins**: the kernel's actions are named
`<origin>__<tool>`, and the attack is a cross-origin flow. Paths on one host would
collapse CRM and shop into one origin. So the hosted demo needs one subdomain per
lab origin, for example `crm.demo.ai2rules.dev`, `shop.demo.ai2rules.dev` and
`widget.demo.ai2rules.dev`, plus a build step that writes what `serve.mjs` serves
today into static files. The extension maps those exact origins to lab names, and
its content scripts and host permissions match only them.

## 6. Agent modes, ranked by setup cost

### (a) The built-in hijacked model (the spike)

No model and no key. The side panel runs the lab's `CompromisedAgent`
(`src/agent/compromised.js`, copied verbatim by the build). It is deterministic,
obeys every injected instruction, and models a fully hijacked reasoning layer, so
every run is the same and the only variable is the policy. Setup: install the
extension and open the demo. This is the self-serve demo.

### (b) Claude with the user's own API key (side panel)

A real model, still in the extension. The side panel runs a tool-calling loop over
the Messages API:

- **Tools.** The tools offered to the model are the kernel's projection of the live
  registrations, plus `read_page`. Every call goes through the same broker, hook and
  kernel as mode (a).
- **Reuse.** The lab already has this loop for Node (`LLMAgent`, AI2-89). It moves
  to the panel with `fetch` and the
  `anthropic-dangerous-direct-browser-access: true` header, which the API needs for
  a direct call from a browser origin.
- **The key.** The user pastes it into the panel. It is kept in
  `chrome.storage.session` by default (memory only, gone when the browser closes);
  remembering it in `chrome.storage.local`, which is plain text on disk, is an
  explicit opt-in. The key is sent only to `api.anthropic.com`. Page scripts cannot
  read extension storage.
- **Costs.** The user pays for tokens. The model may refuse the injection, so a run
  measures the model as much as the kernel, and runs differ (the API rejects
  sampling parameters). The demo copy has to say both.

### (c) Claude Code without a key (native messaging)

The user's existing Claude Code drives the browser through an MCP server (the lab's
live stand, AI2-95, already bridges Claude Code to WebMCP pages). Here that bridge
reaches the extension through **native messaging**:

- A small host program, installed once, is registered under
  `NativeMessagingHosts\<name>` (a registry key on Windows, a JSON file on
  macOS/Linux) with `allowed_origins: ["chrome-extension://<id>/"]`.
- The extension calls `chrome.runtime.connectNative`. An open native port keeps
  the worker alive (Chrome 105+).
- The MCP server in Claude Code talks to the host, so its `list` / `call` /
  `read` requests become the same broker operations as the side panel's.

No API key changes hands, and the user's own Claude Code plan pays. The setup cost is
the highest of the three: install the host (an `npx` installer can write the
manifest and the registry key), add the MCP server to Claude Code, and know the
extension's id (stable once published, or pinned with a `key` in the manifest). The
kernel stays in the extension. The host only relays, so there is still one
decision path.

## 7. What cannot work

- **Claude in Chrome is not gated.** It drives pages through clicks, typing,
  screenshots and the DOM, not through WebMCP tool calls. Nothing it does passes
  through `registerTool`'s handlers, so this extension never sees its actions.
- **The same holds for any agent that does not use WebMCP**, whether a
  computer-use agent or a script typing into forms. Governing those means governing
  DOM actions, which is a different interception point (and a different product
  question).
- **Calls the page makes to itself are not agent calls.** A page that calls its own
  handler function directly is running its own code, and the hook does not
  pretend otherwise.
- **The limits in §2 stand:** the handler's environment (S3b), and tools
  registered around the hook by hostile same-origin script under native WebMCP.

## 8. MV3 constraints that matter

| constraint | what it means here |
|---|---|
| **MAIN-world content scripts** (`"world": "MAIN"`, Chrome 111+) | The hook runs in the page's realm at document_start. The ISOLATED bridge is declared first, so it is listening when the hook hands over the channel. The order was verified by the test, not assumed. |
| **WebAssembly in an extension** | The extension pages' CSP must allow `'wasm-unsafe-eval'` (Chrome 103+). The kernel is instantiated in the service worker from the packaged `.wasm` (`init({module_or_path: chrome.runtime.getURL(…)})`, lazily, because worker modules may not use top-level await). |
| **Service-worker lifetime** | Stopped after 30 s idle. A single event may not run past 5 minutes. Extension API calls (110+) and long-lived port messages (114+) extend it. Hence the state in `storage.session` (§4). |
| **Side panel** (`sidePanel`, Chrome 114+) | Opened from the toolbar button (`setPanelBehavior({openPanelOnActionClick:true})`). `sidePanel.open()` needs a user gesture. The panel page is an ordinary extension page; the test opens it as a tab. |
| **Permissions** | Spike: `sidePanel` and `storage`; host permission and content-script matches for `http://127.0.0.1/*` only. Published: only the demo subdomains, never `<all_urls>`. (b) adds `https://api.anthropic.com/*`. (c) adds `nativeMessaging`. "Govern WebMCP on any site" would be an *optional* host permission requested at runtime, not an install-time one. |
| **Storing a key** (b) | `chrome.storage.session` (memory) by default; `storage.local` is unencrypted on disk, so opt-in only. Never `sync`. |
| **Chrome Web Store review** | MV3 forbids remotely hosted code: every piece of logic must be in the package. The kernel is packaged, and the worlds are packaged too: a world fetched from a server and compiled by the kernel is close enough to "an interpreter for commands fetched from a remote source" to invite a rejection. Also needed: a single stated purpose, a privacy policy once a key or page content is handled, and a justification for every permission. Narrow host permissions keep the review short. |
| **Testing** | Chrome-branded builds ignore `--load-extension` since Chrome 137. Automated runs therefore use Playwright's Chromium (CI) or Edge, and people load the unpacked build from `chrome://extensions` or install it from the store. |

## 9. The spike

```
extension/
  DESIGN.md        this note
  build.mjs        assembles dist/ (kernel, agent, task, worlds and canary from their one source each)
  serve.mjs        the lab's pages + a simulated server, on 127.0.0.1:4301–4304
  demo/sim-server.js
  src/             manifest.json, hook.js, bridge.js, background.js, adapter.js, sidepanel.*
  test/extension.test.js
```

**Run it:**

```bash
cd labs/webmcp
npm run extension:build      # writes extension/dist/
npm run extension:serve      # serves the demo pages; leave it running
```

1. In Chrome, open `chrome://extensions`, turn on Developer mode, choose **Load
   unpacked**, and select `labs/webmcp/extension/dist`.
2. Click the extension's toolbar button. The side panel opens.
3. Pick **D0** or **D3** and press **Run the hijacked model**. CRM and shop tabs
   open, and the panel shows each call, the kernel's verdict and the result. The
   shop tab's footer shows what its server received.

**Checked by `npm run test:extension`** (Playwright, the unpacked extension in a
Chromium-based browser, headless):

- The build's worlds are the lab's `worldYaml(task)`.
- **D0:** the record leaks, and the order is placed.
- **D3:** the injected `search_products` is refused with mechanism
  `flow:confidentiality` (kernel rule `confidentiality_violation`). The handler is
  never entered, nothing leaks, and the order is placed.
- **Parity:** for both policies, the sequence of calls (name, arguments, trigger)
  and verdicts (decision, mechanism, kernel rule) equals the lab's committed trace
  `results/traces/s5b-read-sink-exfil--D{0,3}.jsonl`, which the native binary
  produced.
- **Forgery:** forged handshakes and forged `ALLOW` replies on `window` do not get
  a handler run.

Locally this ran in Edge (`EXT_CHANNEL=msedge`), because this machine's Playwright
Chromium does not start. In CI it runs in Playwright's Chromium.

**Not done in the spike:** modes (b) and (c); the static export and hosting on
demo subdomains; other scenarios (S1–S5 need only their pages and a scenario
picker); the native-WebMCP branch of the hook, which is untested; a store listing.
