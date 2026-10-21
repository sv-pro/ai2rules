# Law-firm story demo

A three-minute demo for people who do not write code. An AI legal assistant at a
fictional firm, Cohen & Levi, reads a client email with a hidden instruction. It
then tries to send the case file out, upload it, delete the copies, and move
money. Each verdict comes from the real kernel deciding over a real world
([`law-firm.world.yaml`](law-firm.world.yaml)). Nothing is executed. Every
decision is written to an audit log and replayed at the end.

## Run it

One command, from any folder. It needs no `harness init`, no credentials and no
network.

```bash
harness demo --story law-firm
```

**Getting the binary:**

- **From source** (Linux, WSL, macOS or Windows with Rust): run from a checkout of
  this repository.

  ```bash
  cargo run -q -p cli-harness -- demo --story law-firm
  ```

- **From npm**: `npx ai2rules-harness demo --story law-firm` works once a release
  includes this flag. Releases up to 0.6.0 do not have it.

The output is plain ASCII, so it reads the same in PowerShell, Windows Terminal,
WSL or an SSH session. Make the terminal wide (about 90 columns) and the font big.

## What to say

| Step | On screen | What to say |
|---|---|---|
| 1–2 | `ALLOW`, session clean | "This is an ordinary assistant doing ordinary work: it opens the case file and saves a memo." |
| 3 | The email, with its hidden instruction; `ALLOW`, session **now TAINTED** | "A client email arrives. The bottom part is white text, invisible to a person and perfectly readable to the AI. The assistant is allowed to read mail. But from this moment the session has read something from outside the firm." |
| 4–5 | `DENY (taint_invariant)` | "The assistant does what the email told it to. It is not caught by a filter guessing at bad intent: once a session has read outside text, it has no way out. Email, upload, every channel." |
| 6–7 | `ASK`, then `DENY` overnight | "Deleting case files always needs a partner. At night, with nobody to ask, the answer is no." |
| 8 | `ABSENT` | "The assistant was never given the trust account. That is not a refusal: for it, the action does not exist." |
| 9 | `ALLOW` | "Its work goes on. Only the exits are closed." |
| 10 | Control session, `ALLOW` | "The same kind of email, from a session that never opened the inbox, is allowed. The only difference is what the assistant read." |
| Audit | `Replay: 10/10` | "Every decision is in a log, and replaying the log gives the same answers. That is what you show an auditor or a client." |

## Questions you will get

- **"Is this a mock-up?"** No. It is the real decision engine. The demo skips
  only the execution step: nothing is actually sent or deleted.
- **"So it detects prompt injection?"** No, and it does not need to. It does not
  try to recognize the attack. It makes the attack's goal impossible: whatever the
  email said, the session can no longer send anything out.
- **"Does a model make these decisions?"** No. The decisions are deterministic.
  The same request gives the same answer every time, and the replay proves it.
- **"Which AI tools does it work with?"** Today: Claude Code, OpenCode,
  Antigravity, DeepSeek Harness, and any MCP client through `harness mcp-gateway`.
- **"What about what we send to the model itself?"** Not covered yet: this
  governs what the assistant *does*. Keeping sensitive text out of the prompt is
  the next module.

## Under the hood

- The story is in [`crates/cli-harness/src/story.rs`](../../../crates/cli-harness/src/story.rs).
- The session is carried from step to step the way a host adapter carries it.
  Nothing marks the session tainted by hand: reading the email does, through the
  kernel's own post-call taint.
- The outbound refusals are the kernel's hard taint invariant (invariant 7),
  which no manifest can relax. The world does not declare them.
- Kept honest by `crates/cli-harness/tests/demo.rs` and `scripts/check-demos.sh`,
  which fail if a verdict changes.
