//! Self-contained replay bundles (E4.5).
//!
//! A bundle pairs the world's authoring manifest with a trace, so the decisions
//! can be replayed offline anywhere — no original repository needed. Replay
//! recompiles the world from the bundled manifest, so it is pinned to the exact
//! ruleset that produced the trace.

use std::io;
use std::path::Path;

use compiler::{compile, CompileError};
use harness_types::WorldManifest;
use serde::{Deserialize, Serialize};

use crate::record::TraceRecord;
use crate::replay::{replay, ReplayReport};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bundle {
    pub manifest: WorldManifest,
    pub records: Vec<TraceRecord>,
}

impl Bundle {
    pub fn new(manifest: WorldManifest, records: Vec<TraceRecord>) -> Self {
        Self { manifest, records }
    }
}

pub fn export_bundle(path: impl AsRef<Path>, bundle: &Bundle) -> io::Result<()> {
    let json = serde_json::to_string_pretty(bundle).map_err(io::Error::other)?;
    std::fs::write(path, json)
}

pub fn import_bundle(path: impl AsRef<Path>) -> io::Result<Bundle> {
    let bytes = std::fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

/// Recompile the bundled world and replay the bundled trace against it.
pub fn replay_bundle(bundle: &Bundle) -> Result<ReplayReport, CompileError> {
    let world = compile(&bundle.manifest)?;
    Ok(replay(&bundle.records, &world))
}
