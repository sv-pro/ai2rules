//! What this version does with an approval log written by 0.4.1.
//!
//! `AuthorizationInstance` (D73) added five fields to the record `ApprovalToken`
//! used to be, and every one carries `#[serde(default)]` with a comment promising
//! the legacy case: an empty principal "never consumes", a zero expiry "marks a
//! legacy/non-consumable artifact". Those defaults are unreachable. The MAC is
//! computed over a *re-serialization* of the parsed event, not over the bytes on
//! disk, so an old record parses and then hashes to something its own version
//! never wrote — it is refused one step earlier, at authentication.
//!
//! Refusing is the right direction and this test does not argue with it. What it
//! pins is that the refusal says which of the two things happened, because
//! "written by an older version" and "modified by something without the key" ask
//! the operator for opposite responses.
//!
//! The shipped `harness` binary keeps its approval store in a per-run tempdir, so
//! no CLI user meets this. An embedder holding the store across an upgrade does.

use harness_types::{
    ActionName, ApprovalToken, ApprovalTokenId, ContentHash, DescriptorHash, EffectMode,
    ManifestHash, PrincipalId, Provenance, SessionId, SourceChannel, WorldId,
};
use serde_json::json;
use trace_store::integrity::hmac_hex;
use trace_store::{effect_binding, ApprovalStore, EffectBinding};

const NOW: u64 = 1_000;

/// The fields D73 added, in the order they are declared — which is the order
/// `serde_json` writes them, and so the order 0.4.1's line simply did not have.
const ADDED_AFTER_0_4_1: [&str; 5] = [
    "principal",
    "canonical_effect_hash",
    "resource",
    "valid_until_unix_ms",
    "remaining_uses",
];

fn binding() -> EffectBinding {
    effect_binding(
        PrincipalId::new("session:s"),
        ActionName::new("start_pty"),
        &json!({"shell": "bash"}),
        "resource:pty".to_string(),
        WorldId::new("w"),
        ManifestHash::new("m1"),
        DescriptorHash::new("desc-1"),
        Provenance::from_channel(
            SourceChannel::UserPrompt,
            SessionId::new("s"),
            ContentHash::new("c"),
        ),
        EffectMode::Simulate,
    )
}

fn token() -> ApprovalToken {
    let b = binding();
    ApprovalToken::pending(
        ApprovalTokenId::new("t1"),
        b.principal,
        b.action,
        b.params_hash,
        b.canonical_effect_hash,
        b.resource,
        b.world_id,
        b.manifest_hash,
        b.descriptor_hash,
        b.provenance,
        b.effect_mode,
        NOW + 1_000,
    )
}

/// Remove `"name":<scalar>` from a compact JSON object, leaving the order of
/// everything else alone — the shape 0.4.1 wrote, before the field existed.
fn without_field(object: &str, name: &str) -> String {
    let key = format!("\"{name}\":");
    let start = object
        .find(&key)
        .unwrap_or_else(|| panic!("{name} is not in {object}"));
    let bytes = object.as_bytes();
    let mut end = start + key.len();
    if bytes[end] == b'"' {
        end += 1;
        while bytes[end] != b'"' {
            end += if bytes[end] == b'\\' { 2 } else { 1 };
        }
        end += 1;
    } else {
        while end < bytes.len() && bytes[end] != b',' && bytes[end] != b'}' {
            end += 1;
        }
    }
    let mut out = String::with_capacity(object.len());
    out.push_str(&object[..start]);
    if bytes[end] == b',' {
        end += 1; // the field takes its separator with it…
    } else {
        out.pop(); // …unless it was last, in which case the one before it goes
    }
    out.push_str(&object[end..]);
    out
}

#[test]
fn a_0_4_1_approval_log_is_refused_as_old_rather_than_as_forged() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("approvals.jsonl");

    // One approved authorization, written the way this version writes it.
    let mut store = ApprovalStore::open(&path).unwrap();
    let id = store.mint(token()).unwrap();
    store.approve(&id).unwrap();
    drop(store);

    // Rewrite the log the way 0.4.1 would have: no D73 fields, and MAC'd over
    // those bytes with the store's own key, so it is a genuine record of its
    // time rather than a corrupted one.
    let key = std::fs::read(dir.path().join("approvals.jsonl.key")).unwrap();
    let downgraded: Vec<String> = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|line| {
            let (event, _) = line.split_once(",\"mac\":").unwrap();
            let event = event.strip_prefix("{\"event\":").unwrap();
            let legacy = ADDED_AFTER_0_4_1.iter().fold(event.to_string(), |acc, f| {
                if acc.contains(&format!("\"{f}\":")) {
                    without_field(&acc, f)
                } else {
                    acc // only the Minted event carries the token
                }
            });
            let mac = hmac_hex(&key, legacy.as_bytes());
            format!("{{\"event\":{legacy},\"mac\":\"{mac}\"}}")
        })
        .collect();
    std::fs::write(&path, downgraded.join("\n") + "\n").unwrap();
    eprintln!("0.4.1-shaped log:\n{}", downgraded.join("\n"));

    let message = match ApprovalStore::open(&path) {
        Err(e) => e.to_string(),
        Ok(_) => panic!("a 0.4.1 record cannot authenticate under this version's MAC"),
    };
    eprintln!("{message}");
    assert!(
        message.contains("before the authorization record carried a principal"),
        "the refusal must name the version change: {message}"
    );
    assert!(
        !message.contains("modified by something without the key"),
        "a genuine old log is not a forged one, and telling an operator to hunt for \
         an intruder is the wrong instruction: {message}"
    );
}
