//! The Public MCP Governance Benchmark — executable smoke pack (issue #64 / AI2-5).
//!
//! Three scenarios, two targets, one deterministic oracle, no LLM. The pack asks
//! the smallest questions that separate a governed MCP surface from an ungoverned
//! one:
//!
//! 1. **Discovery-cache isolation** — can a discovery answer computed for a
//!    privileged principal reach a lower-privileged one?
//! 2. **Approval substitution and replay** — is one human "yes" bound to one
//!    exact effect, once?
//! 3. **Cross-principal handle reuse** — is an authorization handle usable by
//!    anyone who holds it?
//!
//! ## The shape that keeps it honest
//!
//! - Scenarios are **data** ([`scenario`]); the runner ([`run`]) walks their steps
//!   and the oracle ([`oracle`]) checks their expectations. Neither knows which
//!   target is running.
//! - Targets ([`target`]) implement one narrow interface — discover / authorize /
//!   invoke — and receive no scenario identity, so a target cannot special-case a
//!   case it is about to fail.
//! - The downstream effect counter lives in the mock upstream ([`upstream`]),
//!   which the *runner* owns. PASS therefore requires an observed decision **and**
//!   an observed effect count, never a target's own claim about either.
//! - Every verdict is recorded in the full vocabulary
//!   ([`target::Verdict`]): `ABSENT`, `ALLOW`, `DENY`, `ASK`, `ERROR_CLOSED`,
//!   `ERROR_OPEN`, `UNKNOWN` are distinct outcomes and are never collapsed.
//! - Results are published per scenario. There is deliberately no aggregate score.

pub mod accept;
pub mod oracle;
pub mod pack;
pub mod report;
pub mod result;
pub mod run;
pub mod scenario;
pub mod target;
pub mod targets;
pub mod upstream;

pub use pack::Pack;
pub use result::{BenchResult, RunResult};
pub use run::run_scenario;
