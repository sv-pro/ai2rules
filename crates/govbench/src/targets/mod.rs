//! The two targets this seed pack compares.
//!
//! [`weak`] is an intentionally weak reference gateway: real policy, real
//! filtering logic, and three implementation defects that are common in the wild.
//! [`ai2rules`] is this repository's kernel, reached through its shipped surfaces.

pub mod ai2rules;
pub mod weak;

pub use ai2rules::{Ai2rules, Transport};
pub use weak::{WeakGateway, WeakPolicy};
