//! Intent mapping: ontology pre-check (E5.3).
//!
//! The pre-IR mapper from architecture §6: it consults the *full* ontology so the
//! loop can give the model an immediate `UNKNOWN_TO_ONTOLOGY` (the action does not
//! exist anywhere) distinct from `ABSENT` (it exists but isn't projected here).
//! The kernel's `decide` remains the authoritative classifier — it returns
//! `KernelOutcome::UnknownToOntology` vs an `Absent` decision — so this is a
//! lightweight early signal, not a second policy path.

use harness_types::{ActionName, CompiledWorld};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mapping {
    /// In the compiled ontology (projected or not — the kernel decides which).
    Known,
    /// Not in the ontology at all → `UNKNOWN_TO_ONTOLOGY`.
    Unknown,
}

pub fn classify(world: &CompiledWorld, action: &str) -> Mapping {
    if world.in_ontology(&ActionName::new(action)) {
        Mapping::Known
    } else {
        Mapping::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compiler::compile_default;

    #[test]
    fn known_vs_unknown() {
        let world = compile_default();
        assert_eq!(classify(&world, "read_workspace"), Mapping::Known);
        assert_eq!(classify(&world, "send_email"), Mapping::Unknown);
    }
}
