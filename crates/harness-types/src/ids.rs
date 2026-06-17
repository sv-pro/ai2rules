//! Strongly-typed identifiers and content/descriptor hashes.
//!
//! Stringly-typed ids and hashes are a known anti-pattern (architecture §11);
//! every id is a distinct newtype so they cannot be confused at a boundary.

macro_rules! id_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default,
            serde::Serialize, serde::Deserialize,
        )]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }
    };
}

id_newtype!(
    /// Identifier of a compiled world.
    WorldId
);
id_newtype!(
    /// Hash of the authoring manifest a world was compiled from.
    ManifestHash
);
id_newtype!(
    /// Deterministic hash of an exposed tool/action descriptor.
    DescriptorHash
);
id_newtype!(
    /// Hash identifying a payload's content.
    ContentHash
);
id_newtype!(
    /// Identifier of an agent session.
    SessionId
);
id_newtype!(
    /// Identifier correlating all trace records for one request.
    TraceId
);
id_newtype!(
    /// Provider-assigned identifier for a single tool call.
    CallId
);
id_newtype!(
    /// Identifier of a `Perception`.
    PerceptionId
);
id_newtype!(
    /// Identifier of an `ApprovalToken`.
    ApprovalTokenId
);
id_newtype!(
    /// Name of an action in the world ontology.
    ActionName
);
