# D47 — Commands fail closed without a sandbox; E8 is a pluggable isolation adapter


**Date:** 2026-07-24.

- **Context:** A security review (finding #10) found `CommandHandler` spawned subprocesses while
  ignoring the `ExecutionSpec`'s `NetworkPolicy` and `FilesystemPolicy` — a child could open sockets
  or write outside the writable roots regardless. Truly confining a subprocess is an OS-level concern
  (namespaces / seccomp / a microVM), which is epic **E8** (isolated FS roots, network-off-by-default,
  kill-tree) and is not yet built. `NetworkPolicy` has no "allow-all" variant and `build_execution_spec`
  stamps the runtime policy onto every command spec, so there is no honest "unrestricted" fallback.
- **Decision:** The executor does **not** silently run a command whose policy it cannot enforce.
  `CommandHandler` carries an explicit `Confinement` posture and **fails closed** on `Execute`
  (`ExecError::SandboxRequired`) unless the caller has explicitly accepted unconfined execution
  (`CommandHandler::unconfined()`). `Simulate` is always allowed (it spawns nothing). The one
  production opt-in is `agent-core`'s live command loop, marked explicitly; it should become
  operator-configurable. When E8 lands, an active OS sandbox becomes a third posture and the
  `unconfined` acknowledgment retires.
- **E8 stays its own epic — a pluggable isolation adapter, not hand-rolled syscalls.** We weighed
  pulling E8 forward to *enforce* rather than refuse. Deferred: (a) the gate is E8's seam regardless —
  a no-sandbox / old-kernel / non-Linux host still needs the fail-closed fallback, and network egress
  control is not cheap even with a sandbox; (b) the clean unprivileged sandbox crates (`landlock`,
  `birdcage`/`extrasafe`, `cap-std`) are not in the offline build cache, so a real slice means
  vendoring deps or hand-writing `landlock_*`/seccomp — security-critical code under time pressure,
  the exact overclaim the thesis warns against ("governed ≠ confined"). When built, E8 should be a
  **pluggable backend** (namespaces / gVisor / Firecracker-class microVM, plausibly a hosted
  remote-plane sandbox) that satisfies the gate — not bespoke syscalls.
- **Why:** Running a subprocess while ignoring the policy it was handed is a silent authority leak.
  Fail-closed-by-default with a single explicit, greppable opt-in makes the honesty auditable, keeps
  the live-agent capability, and defines the interface E8 plugs into. This is the two-layer posture —
  governance decides *whether* an action may run; isolation contains its *blast radius* — argued
  independently by LangChain's "Agents Need Their Own Computer" (microVM isolation *alongside* policy
  controls, "not instead of").
- **Alternatives rejected:** keep running unconfined and only annotate the trace (does not stop the
  out-of-root write — unacceptable for a high-severity finding); refuse all command `Execute` with no
  opt-in (disables the live agent's command capability before E8); add the posture to the sealed
  `ExecutionSpec` (it is execution-side config, not kernel policy — keep the spec sealed, cf. D45).
- **Related:** D9 (ExecutionSpec boundary), E3 (execution boundary), E8 (Layer-0 OS sandbox),
  finding #9 (fs_guard symlink — same "policy guard now, E8 OS backstop later" framing), findings
  #11/#12 (kill-tree, WebHandler egress — same seam).
