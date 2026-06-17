//! Compile-time (design-time) errors. Distinct from the runtime `BuildError`.

/// Reasons a manifest fails to load, validate, or compile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    /// The manifest text could not be parsed into a `WorldManifest`.
    Parse(String),
    /// `world_id` is empty.
    EmptyWorldId,
    /// Two base actions (or two scoped capabilities) share a name.
    DuplicateAction(String),
    /// A scoped capability and a base action share a name.
    NameCollision(String),
    /// A scoped capability references a base action that does not exist.
    UnknownBaseAction { capability: String, base: String },
    /// Any other structural problem with a human-readable detail.
    Invalid(String),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::Parse(detail) => write!(f, "manifest parse error: {detail}"),
            CompileError::EmptyWorldId => write!(f, "world_id must not be empty"),
            CompileError::DuplicateAction(name) => write!(f, "duplicate action name: {name}"),
            CompileError::NameCollision(name) => {
                write!(
                    f,
                    "scoped capability {name} collides with a base action of the same name"
                )
            }
            CompileError::UnknownBaseAction { capability, base } => {
                write!(
                    f,
                    "scoped capability {capability} references unknown base action {base}"
                )
            }
            CompileError::Invalid(detail) => write!(f, "invalid manifest: {detail}"),
        }
    }
}

impl std::error::Error for CompileError {}
