//! Taint classification for the kernel (E2.2).
//!
//! The taint *primitives* — `Taint::join`, `TaintedValue`, `TaintContext`,
//! `Provenance`, and per-channel defaults — already live in
//! `harness_types::provenance`. This module adds only the kernel-side policy
//! classification the invariants and disposition stages share, so there is a
//! single source of truth for "which side effects leave the local boundary".
//!
//! Monotonicity is preserved structurally: `IRBuilder::build` reads taint from a
//! [`harness_types::TaintContext`] (a join of upstream outputs) and never lowers
//! it, so taint can only ever increase across a pipeline — including across
//! sessions, since taint rides `Provenance`/`TaintContext`.

use harness_types::SideEffectClass;

/// Side-effect classes that move data outside the local, in-session boundary.
///
/// The hard taint invariant (see [`crate::invariants`]) forbids a `Tainted`
/// value from driving any of these surfaces — network egress, external side
/// effects, credential access, durable memory writes, or persistent writes.
/// This is the structural floor behind acceptance invariant 7.
pub fn externally_effectful(side_effect: SideEffectClass) -> bool {
    matches!(
        side_effect,
        SideEffectClass::Network
            | SideEffectClass::External
            | SideEffectClass::Credential
            | SideEffectClass::Memory
            | SideEffectClass::PersistentWrite
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_surfaces_are_classified() {
        for se in [
            SideEffectClass::Network,
            SideEffectClass::External,
            SideEffectClass::Credential,
            SideEffectClass::Memory,
            SideEffectClass::PersistentWrite,
        ] {
            assert!(externally_effectful(se), "{se:?} should be external");
        }
    }

    #[test]
    fn local_surfaces_are_not_external() {
        for se in [
            SideEffectClass::None,
            SideEffectClass::Read,
            SideEffectClass::FilesystemWrite,
            SideEffectClass::Process,
        ] {
            assert!(!externally_effectful(se), "{se:?} should be local");
        }
    }
}
