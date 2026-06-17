//! Append-only JSONL trace store (E4.1).
//!
//! One JSON object per line, opened in append mode and flushed per write — the
//! log only ever grows, and a crash leaves a prefix of valid lines.

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::record::TraceRecord;

pub struct TraceStore {
    path: PathBuf,
}

impl TraceStore {
    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one record as a JSON line. Append-only: never rewrites prior lines.
    pub fn append(&self, record: &TraceRecord) -> io::Result<()> {
        let line = serde_json::to_string(record).map_err(io::Error::other)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{line}")?;
        file.flush()
    }

    /// Read every record back, in write order. A missing file is an empty trace.
    pub fn read(path: impl AsRef<Path>) -> io::Result<Vec<TraceRecord>> {
        let file = match File::open(path.as_ref()) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        let mut records = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            records.push(serde_json::from_str(&line).map_err(io::Error::other)?);
        }
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{
        ContextSnapshot, DecisionRecord, OutcomeKind, OutcomeSummary, RecordPayload,
    };
    use harness_types::{
        ActionName, ContentHash, Decision, ExecutionMode, ManifestHash, Provenance, SessionId,
        SourceChannel, Taint, TraceId, WorldId,
    };
    use serde_json::json;

    fn sample(seq: u64, action: &str) -> TraceRecord {
        TraceRecord {
            trace_id: TraceId::new("t"),
            session_id: SessionId::new("s"),
            world_id: WorldId::new("w"),
            manifest_hash: ManifestHash::new("m"),
            seq,
            payload: RecordPayload::Decision(DecisionRecord {
                action: ActionName::new(action),
                params: json!({}),
                provenance: Provenance::from_channel(
                    SourceChannel::UserPrompt,
                    SessionId::new("s"),
                    ContentHash::new("h"),
                ),
                context: ContextSnapshot {
                    taint: Taint::Clean,
                    mode: ExecutionMode::Interactive,
                    commands_run: 0,
                    tokens_used: 0,
                    file_writes: 0,
                    network_calls: 0,
                },
                outcome: OutcomeSummary {
                    kind: OutcomeKind::Evaluated,
                    decision: Decision::Allow,
                    rule: "default_allow".to_string(),
                    effect_mode: None,
                    descriptor_hash: None,
                },
            }),
        }
    }

    #[test]
    fn append_then_read_round_trips_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let store = TraceStore::open(dir.path().join("trace.jsonl"));
        store.append(&sample(0, "read_workspace")).unwrap();
        store.append(&sample(1, "run_command")).unwrap();

        let back = TraceStore::read(store.path()).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].seq, 0);
        assert_eq!(back[1].seq, 1);
        assert_eq!(
            back,
            vec![sample(0, "read_workspace"), sample(1, "run_command")]
        );
    }

    #[test]
    fn reading_a_missing_trace_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let back = TraceStore::read(dir.path().join("nope.jsonl")).unwrap();
        assert!(back.is_empty());
    }
}
