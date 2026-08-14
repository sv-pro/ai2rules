//! Durable approval store (E6.2–E6.4).
//!
//! Human-in-the-loop approval as durable state, not memory. The store is an
//! append-only JSONL log of lifecycle transitions (`Minted → Approved/Rejected →
//! Executed`), folded into the current token set on load — same shape as the
//! trace store, and replayable. An approval is bound to the exact call (action,
//! params, world, **compiled manifest**, descriptor hash, provenance, effect
//! mode); any drift in those voids reuse (E6.4).
//!
//! ## Why the log is signed (finding #15)
//!
//! This is the one file in the system whose contents *grant* something. As plain
//! append-only JSONL, anything that could write to it could append a `Minted` in
//! the `Approved` state and manufacture a human decision that never happened —
//! and the store would fold it in on load without a murmur.
//!
//! So every line carries an HMAC-SHA256 over its event, under a key kept beside
//! the log with `0600` and opened `O_NOFOLLOW`. Forging a grant now needs the key,
//! not merely write access. A line whose MAC does not verify makes `open` **fail**
//! rather than skip the line: a store that has been tampered with is not a store
//! to keep serving answers from, and the failure direction is safe — no approvals
//! means the human is asked again.
//!
//! The MAC is not a substitute for putting the store somewhere the governed
//! project cannot reach. It is what remains true when that assumption fails.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use compiler::sha256_hex;
use harness_types::{
    ActionName, ApprovalState, ApprovalToken, ApprovalTokenId, ApprovalTransitionError,
    ContentHash, DescriptorHash, EffectMode, ManifestHash, Provenance, WorldId,
};

use crate::integrity::{constant_time_eq, hmac_hex};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One durable transition in the approval log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ApprovalEvent {
    Minted(Box<ApprovalToken>),
    Approved(ApprovalTokenId),
    Rejected(ApprovalTokenId),
    Executed(ApprovalTokenId),
}

/// One line of the log: the event plus the MAC that makes it ours.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignedEvent {
    event: ApprovalEvent,
    mac: String,
}

/// The bytes a line's MAC covers — the event's canonical JSON.
fn event_bytes(event: &ApprovalEvent) -> io::Result<String> {
    serde_json::to_string(event).map_err(io::Error::other)
}

/// The canonical params hash used in an approval binding.
pub fn params_hash(params: &Value) -> ContentHash {
    let canonical = serde_json::to_string(params).unwrap_or_default();
    ContentHash::new(sha256_hex(canonical.as_bytes()))
}

/// Durable, append-only store of approval tokens and their lifecycle.
pub struct ApprovalStore {
    path: PathBuf,
    key: Vec<u8>,
    tokens: BTreeMap<ApprovalTokenId, ApprovalToken>,
}

impl ApprovalStore {
    /// Open (or start) a store, folding any existing log into current state.
    pub fn open(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        let key = load_or_create_key(&key_path(&path))?;
        let tokens = load(&path, &key)?;
        Ok(Self { path, key, tokens })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn token(&self, id: &ApprovalTokenId) -> Option<&ApprovalToken> {
        self.tokens.get(id)
    }

    /// Persist a freshly-minted pending token.
    pub fn mint(&mut self, token: ApprovalToken) -> io::Result<ApprovalTokenId> {
        let id = token.id.clone();
        self.append(&ApprovalEvent::Minted(Box::new(token.clone())))?;
        self.tokens.insert(id.clone(), token);
        Ok(id)
    }

    pub fn approve(&mut self, id: &ApprovalTokenId) -> io::Result<()> {
        self.transition(
            id,
            ApprovalToken::approve,
            ApprovalEvent::Approved(id.clone()),
        )
    }

    pub fn reject(&mut self, id: &ApprovalTokenId) -> io::Result<()> {
        self.transition(
            id,
            ApprovalToken::reject,
            ApprovalEvent::Rejected(id.clone()),
        )
    }

    pub fn mark_executed(&mut self, id: &ApprovalTokenId) -> io::Result<()> {
        self.transition(
            id,
            ApprovalToken::mark_executed,
            ApprovalEvent::Executed(id.clone()),
        )
    }

    /// True iff an **Approved** token binds to this exact call (E6.4). Any drift
    /// — different world, descriptor hash, params, provenance, or effect mode —
    /// means no match, so an approval is never reused after the world changes.
    #[allow(clippy::too_many_arguments)]
    pub fn is_granted(
        &self,
        action: &ActionName,
        params: &Value,
        world_id: &WorldId,
        manifest_hash: &ManifestHash,
        descriptor_hash: &DescriptorHash,
        provenance: &Provenance,
        effect_mode: EffectMode,
    ) -> bool {
        let ph = params_hash(params);
        self.tokens.values().any(|t| {
            t.state == ApprovalState::Approved
                && t.action == *action
                && t.params_hash == ph
                && t.world_id == *world_id
                && t.manifest_hash == *manifest_hash
                && t.descriptor_hash == *descriptor_hash
                && t.provenance == *provenance
                && t.effect_mode == effect_mode
        })
    }

    fn transition(
        &mut self,
        id: &ApprovalTokenId,
        apply: impl FnOnce(&mut ApprovalToken) -> Result<(), ApprovalTransitionError>,
        event: ApprovalEvent,
    ) -> io::Result<()> {
        {
            let token = self
                .tokens
                .get_mut(id)
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "unknown approval token"))?;
            apply(token).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        }
        self.append(&event)
    }

    fn append(&self, event: &ApprovalEvent) -> io::Result<()> {
        let payload = event_bytes(event)?;
        let signed = SignedEvent {
            event: event.clone(),
            mac: hmac_hex(&self.key, payload.as_bytes()),
        };
        let line = serde_json::to_string(&signed).map_err(io::Error::other)?;
        let mut file = private_append(&self.path)?;
        writeln!(file, "{line}")?;
        file.flush()
    }
}

/// The key lives beside the log: `approvals.jsonl` -> `approvals.jsonl.key`.
fn key_path(store: &Path) -> PathBuf {
    let mut name = store.as_os_str().to_os_string();
    name.push(".key");
    PathBuf::from(name)
}

/// Open a file for appending, creating it `0600`, refusing to follow a symlink.
///
/// `O_NOFOLLOW` matters because the store's directory may be more reachable than
/// the store: replacing `approvals.jsonl` with a link to somewhere else is a way
/// to make the harness write, or read, a file it did not choose.
fn private_append(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600).custom_flags(libc_o_nofollow());
    }
    opts.open(path)
}

#[cfg(unix)]
fn libc_o_nofollow() -> i32 {
    // O_NOFOLLOW is 0x20000 on Linux and 0x100 on macOS/BSD. Spelled out rather
    // than pulling `libc` into this crate for one constant.
    #[cfg(target_os = "linux")]
    {
        0x2_0000
    }
    #[cfg(not(target_os = "linux"))]
    {
        0x100
    }
}

/// Read the MAC key, creating a fresh random one on first use.
///
/// The key is `0600` and owned by us, and both are *checked* on every open: a key
/// another account can read is a key that can forge grants, and silently using it
/// would be worse than refusing.
fn load_or_create_key(path: &Path) -> io::Result<Vec<u8>> {
    match std::fs::read(path) {
        Ok(key) => {
            check_key_permissions(path)?;
            if key.len() < 32 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "approval key {} is too short to be a real key ({} bytes); \
                         delete it to have a new one generated",
                        path.display(),
                        key.len()
                    ),
                ));
            }
            Ok(key)
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            let key = random_key()?;
            let mut file = private_create_new(path)?;
            file.write_all(&key)?;
            file.flush()?;
            Ok(key)
        }
        Err(e) => Err(e),
    }
}

/// Create a file that must not already exist, `0600`, no symlink following.
fn private_create_new(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600).custom_flags(libc_o_nofollow());
    }
    opts.open(path)
}

#[cfg(unix)]
fn check_key_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::PermissionsExt;
    // `symlink_metadata` so a link is judged as a link, not as its target.
    let meta = std::fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("approval key {} is a symlink; refusing", path.display()),
        ));
    }
    let mode = meta.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "approval key {} is mode {:o}; it must not be readable or writable by \
                 group or others (chmod 600)",
                path.display(),
                mode
            ),
        ));
    }
    if meta.uid() != nix_getuid() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "approval key {} is owned by uid {}, not by this process",
                path.display(),
                meta.uid()
            ),
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_key_permissions(_path: &Path) -> io::Result<()> {
    // Windows ACLs are not modelled here; the MAC still binds the log.
    Ok(())
}

#[cfg(unix)]
fn nix_getuid() -> u32 {
    // `std` exposes no uid; read it from the one file we know we own.
    use std::os::unix::fs::MetadataExt;
    std::env::current_exe()
        .and_then(std::fs::metadata)
        .map(|m| m.uid())
        .unwrap_or_else(|_| u32::MAX)
}

/// 32 bytes from the OS CSPRNG.
fn random_key() -> io::Result<Vec<u8>> {
    #[cfg(unix)]
    {
        use std::io::Read as _;
        let mut buf = vec![0u8; 32];
        File::open("/dev/urandom")?.read_exact(&mut buf)?;
        Ok(buf)
    }
    #[cfg(not(unix))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "no CSPRNG wired up for this platform; the approval store needs one",
        ))
    }
}

fn load(path: &Path, key: &[u8]) -> io::Result<BTreeMap<ApprovalTokenId, ApprovalToken>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(e),
    };
    let mut tokens = BTreeMap::new();
    for (n, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        // An unsigned line and a forged line are the same thing from here, so
        // there is no lenient path: both fail the load (finding #15).
        let signed: SignedEvent = serde_json::from_str(&line).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "approval log {} line {}: not a signed event ({e}). An unsigned \
                     entry cannot be told apart from a forged one, so the store \
                     refuses to load rather than honour it",
                    path.display(),
                    n + 1
                ),
            )
        })?;
        let expected = hmac_hex(key, event_bytes(&signed.event)?.as_bytes());
        if !constant_time_eq(&expected, &signed.mac) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "approval log {} line {}: MAC does not verify — the log has been \
                     modified by something without the key. Refusing to load it; \
                     pending approvals will simply be asked again",
                    path.display(),
                    n + 1
                ),
            ));
        }
        apply_event(&mut tokens, signed.event);
    }
    Ok(tokens)
}

fn apply_event(tokens: &mut BTreeMap<ApprovalTokenId, ApprovalToken>, event: ApprovalEvent) {
    match event {
        ApprovalEvent::Minted(token) => {
            tokens.insert(token.id.clone(), *token);
        }
        ApprovalEvent::Approved(id) => {
            if let Some(t) = tokens.get_mut(&id) {
                let _ = t.approve();
            }
        }
        ApprovalEvent::Rejected(id) => {
            if let Some(t) = tokens.get_mut(&id) {
                let _ = t.reject();
            }
        }
        ApprovalEvent::Executed(id) => {
            if let Some(t) = tokens.get_mut(&id) {
                let _ = t.mark_executed();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use harness_types::{ContentHash, EffectMode, Provenance, SessionId, SourceChannel};
    use serde_json::json;

    fn token(id: &str, params: &Value, descriptor: &str) -> ApprovalToken {
        ApprovalToken::pending(
            ApprovalTokenId::new(id),
            ActionName::new("start_pty"),
            params_hash(params),
            WorldId::new("w"),
            ManifestHash::new("m1"),
            DescriptorHash::new(descriptor),
            prov(),
            EffectMode::Simulate,
        )
    }

    fn prov() -> Provenance {
        Provenance::from_channel(
            SourceChannel::UserPrompt,
            SessionId::new("s"),
            ContentHash::new("c"),
        )
    }

    fn granted(store: &ApprovalStore, params: &Value, descriptor: &str) -> bool {
        store.is_granted(
            &ActionName::new("start_pty"),
            params,
            &WorldId::new("w"),
            &ManifestHash::new("m1"),
            &DescriptorHash::new(descriptor),
            &prov(),
            EffectMode::Simulate,
        )
    }

    #[test]
    fn mint_approve_then_granted() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ApprovalStore::open(dir.path().join("approvals.jsonl")).unwrap();
        let params = json!({"shell": "bash"});
        let id = store.mint(token("t1", &params, "desc-1")).unwrap();
        assert!(!granted(&store, &params, "desc-1")); // pending, not yet approved
        store.approve(&id).unwrap();
        assert!(granted(&store, &params, "desc-1"));
    }

    #[test]
    fn drift_voids_the_approval() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ApprovalStore::open(dir.path().join("approvals.jsonl")).unwrap();
        let params = json!({"shell": "bash"});
        let id = store.mint(token("t1", &params, "desc-1")).unwrap();
        store.approve(&id).unwrap();
        // Descriptor drift → the approval no longer matches.
        assert!(!granted(&store, &params, "desc-2"));
        // Param change → no match either.
        assert!(!granted(&store, &json!({"shell": "zsh"}), "desc-1"));
    }

    #[test]
    fn reject_is_not_granted() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ApprovalStore::open(dir.path().join("approvals.jsonl")).unwrap();
        let params = json!({});
        let id = store.mint(token("t1", &params, "desc-1")).unwrap();
        store.reject(&id).unwrap();
        assert!(!granted(&store, &params, "desc-1"));
    }

    // ---- finding #15: the log grants things, so forging it must not work ----

    /// The attack the MAC exists for: something with write access to the log
    /// appends a token that is already `Approved`, manufacturing a human decision.
    #[test]
    fn an_appended_forged_approval_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("approvals.jsonl");
        let params = json!({"shell": "bash"});
        {
            let mut store = ApprovalStore::open(&path).unwrap();
            store.mint(token("t1", &params, "desc-1")).unwrap();
        }

        // The forger writes a well-formed event. They do not have the key, so they
        // either omit the MAC or guess one.
        let mut forged = token("evil", &params, "desc-1");
        forged.state = ApprovalState::Approved;
        let event = ApprovalEvent::Minted(Box::new(forged));
        for line in [
            serde_json::to_string(&event).unwrap(), // unsigned
            serde_json::to_string(&SignedEvent {
                event,
                mac: "00".repeat(32), // guessed
            })
            .unwrap(),
        ] {
            let mut f = OpenOptions::new().append(true).open(&path).unwrap();
            writeln!(f, "{line}").unwrap();
            drop(f);

            let err = match ApprovalStore::open(&path) {
                Ok(_) => panic!("a log with an unverifiable line must not load"),
                Err(e) => e,
            };
            assert_eq!(err.kind(), io::ErrorKind::InvalidData, "{err}");

            // Reset for the next shape.
            let good: Vec<String> = std::fs::read_to_string(&path)
                .unwrap()
                .lines()
                .take(1)
                .map(str::to_string)
                .collect();
            std::fs::write(&path, format!("{}\n", good.join("\n"))).unwrap();
        }

        // The untampered log still loads.
        assert!(ApprovalStore::open(&path).is_ok());
    }

    /// Editing an existing legitimate line — flipping a bound field — is caught too.
    #[test]
    fn a_modified_line_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("approvals.jsonl");
        let params = json!({"shell": "bash"});
        {
            let mut store = ApprovalStore::open(&path).unwrap();
            let id = store.mint(token("t1", &params, "desc-1")).unwrap();
            store.approve(&id).unwrap();
        }
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replace("desc-1", "desc-2")).unwrap();

        let err = match ApprovalStore::open(&path) {
            Ok(_) => panic!("an edited line must not load"),
            Err(e) => e,
        };
        assert!(format!("{err}").contains("MAC does not verify"), "{err}");
    }

    /// A rewritten policy voids an approval granted under the old one, even though
    /// the world keeps its id and the descriptor is unchanged.
    #[test]
    fn manifest_drift_voids_the_approval() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = ApprovalStore::open(dir.path().join("approvals.jsonl")).unwrap();
        let params = json!({"shell": "bash"});
        let id = store.mint(token("t1", &params, "desc-1")).unwrap();
        store.approve(&id).unwrap();
        assert!(granted(&store, &params, "desc-1"));

        let under_new_policy = store.is_granted(
            &ActionName::new("start_pty"),
            &params,
            &WorldId::new("w"),
            &ManifestHash::new("m2"), // same world, rewritten rules
            &DescriptorHash::new("desc-1"),
            &prov(),
            EffectMode::Simulate,
        );
        assert!(
            !under_new_policy,
            "an approval must not survive the policy it was granted under"
        );
    }

    /// A key any other account can read is a key that can forge grants.
    #[cfg(unix)]
    #[test]
    fn a_world_readable_key_is_refused() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("approvals.jsonl");
        ApprovalStore::open(&path).unwrap(); // creates the key
        let key = dir.path().join("approvals.jsonl.key");
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();

        let err = match ApprovalStore::open(&path) {
            Ok(_) => panic!("a readable key must be refused"),
            Err(e) => e,
        };
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied, "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn the_key_is_created_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("approvals.jsonl");
        ApprovalStore::open(&path).unwrap();
        let mode = std::fs::metadata(dir.path().join("approvals.jsonl.key"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "key mode is {mode:o}");
    }

    #[test]
    fn state_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("approvals.jsonl");
        let params = json!({"shell": "bash"});
        {
            let mut store = ApprovalStore::open(&path).unwrap();
            let id = store.mint(token("t1", &params, "desc-1")).unwrap();
            store.approve(&id).unwrap();
        }
        // Reopen from disk — the approval is still in force (durable).
        let store = ApprovalStore::open(&path).unwrap();
        assert!(granted(&store, &params, "desc-1"));
    }
}
