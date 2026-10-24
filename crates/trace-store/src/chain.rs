//! Hash-chained, head-anchored signed logs (D78).
//!
//! The approval log and the staged-commit log are the two files whose contents
//! *grant* or *settle* something. Since finding #15 every line carries an HMAC, so
//! a line cannot be forged or edited without the key. But a MAC per line says
//! nothing about the lines that are *not there*: dropping the signed `Consumed`
//! line handed a single-use grant back, and dropping `AttemptStarted` /
//! `AttemptFinished` released an idempotency key — every remaining line still
//! verified.
//!
//! Two additions close that:
//!
//! * **Chain.** Line `n` carries `seq = n` and `prev` = the MAC of line `n-1`
//!   (`GENESIS` for line 0), and its MAC covers `(seq, prev, event)`. Deleting,
//!   reordering, or splicing a line from another log breaks the chain.
//! * **Head.** A chain alone cannot see its own *tail* being cut: any prefix of a
//!   valid chain is a valid chain. So after every append the store rewrites
//!   `<log>.head` — the last `(seq, mac)`, itself MACed — atomically. On load the
//!   log must reach the head, and may run **at most one line** past it: the one a
//!   crash between the append and the head write can leave. `open` re-anchors such
//!   a log, so the slack never accumulates.
//! * **High-water mark.** Each open store remembers the furthest `(seq, mac)` it has
//!   loaded or written and refuses any later load that falls short of it. A log
//!   rolled back under a running process is caught even with a matching head.
//!
//! What this does **not** stop: the log is append-only, so every older log is a
//! prefix of the current one, and an attacker who saved a copy of the *head* alone
//! at the right moment (after `Approved`, before `Consumed`) can cut the log back
//! to it and restore that head — against a process that has not yet seen the later
//! lines. Rollback to a consistent earlier state needs a monotonic counter outside
//! the attacker's reach, which a file beside the log cannot be. As with the MAC
//! itself, the defence that matters is keeping the store where the governed agent
//! cannot write; this is what remains true when that fails, and it turns "delete a
//! line" into "have planned the snapshot in advance".

use std::fs::OpenOptions;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::integrity::{constant_time_eq, hmac_hex};

/// `prev` of the first line.
pub(crate) const GENESIS: &str = "genesis";
const HEAD_VERSION: u32 = 1;

/// Where the next line goes: its sequence number and the MAC it must chain to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Tail {
    pub(crate) next_seq: u64,
    pub(crate) last_mac: String,
}

impl Tail {
    pub(crate) fn genesis() -> Self {
        Self {
            next_seq: 0,
            last_mac: GENESIS.to_string(),
        }
    }
}

/// The MAC of one chained line. Domain-separated so a line MAC can never be
/// replayed as a head MAC, or as a pre-D78 unchained MAC.
pub(crate) fn line_mac(key: &[u8], seq: u64, prev: &str, payload: &str) -> String {
    hmac_hex(
        key,
        format!("ai2rules-log-line-v1\n{seq}\n{prev}\n{payload}").as_bytes(),
    )
}

/// Why a line failed the chain. Callers turn this into their own message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LineFault {
    /// No `seq`/`prev`: written before D78.
    Unchained,
    /// Right key, wrong place: a line was deleted, reordered, or spliced in.
    OutOfChain,
    /// The MAC does not verify at all.
    BadMac,
}

/// Verify one line against the running tail and advance it.
pub(crate) fn verify_line(
    key: &[u8],
    tail: &mut Tail,
    seq: Option<u64>,
    prev: Option<&str>,
    mac: &str,
    payload: &str,
) -> Result<(), LineFault> {
    let (Some(seq), Some(prev)) = (seq, prev) else {
        return Err(LineFault::Unchained);
    };
    let expected = line_mac(key, seq, prev, payload);
    if !constant_time_eq(&expected, mac) {
        return Err(LineFault::BadMac);
    }
    if seq != tail.next_seq || !constant_time_eq(prev, &tail.last_mac) {
        return Err(LineFault::OutOfChain);
    }
    tail.next_seq += 1;
    tail.last_mac = mac.to_string();
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Head {
    v: u32,
    /// Sequence number of the last line anchored, `None` for an empty log.
    seq: Option<u64>,
    /// MAC of that line (`GENESIS` for an empty log).
    mac: String,
    head_mac: String,
}

fn head_mac(key: &[u8], seq: Option<u64>, mac: &str) -> String {
    let seq = seq.map_or_else(|| "none".to_string(), |s| s.to_string());
    hmac_hex(
        key,
        format!("ai2rules-log-head-v1\n{seq}\n{mac}").as_bytes(),
    )
}

/// `approvals.jsonl` -> `approvals.jsonl.head`.
pub(crate) fn head_path(log: &Path) -> PathBuf {
    let mut name = log.as_os_str().to_os_string();
    name.push(".head");
    PathBuf::from(name)
}

fn tmp_path(log: &Path) -> PathBuf {
    let mut name = log.as_os_str().to_os_string();
    name.push(".head.tmp");
    PathBuf::from(name)
}

/// Anchor the log at `tail` (the state *after* the last append). Written to a
/// temporary file, synced, then renamed over the head, so a reader sees the old
/// head or the new one, never a torn one.
pub(crate) fn write_head(log: &Path, key: &[u8], tail: &Tail) -> io::Result<()> {
    let seq = tail.next_seq.checked_sub(1);
    let head = Head {
        v: HEAD_VERSION,
        seq,
        mac: tail.last_mac.clone(),
        head_mac: head_mac(key, seq, &tail.last_mac),
    };
    let bytes = serde_json::to_vec(&head).map_err(io::Error::other)?;
    let tmp = tmp_path(log);
    {
        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600).custom_flags(o_nofollow());
        }
        let mut file = opts.open(&tmp)?;
        file.write_all(&bytes)?;
        file.flush()?;
        file.sync_data()?;
    }
    let head = head_path(log);
    std::fs::rename(&tmp, &head)?;
    sync_parent(&head);
    Ok(())
}

/// Best effort: make the rename itself durable. Without it a power loss can bring
/// the old head back — which the one-line slack in [`verify_head`] tolerates.
fn sync_parent(path: &Path) {
    #[cfg(unix)]
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        if let Ok(dir) = std::fs::File::open(dir) {
            let _ = dir.sync_all();
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Create the genesis head for a store that has never been written. A store
/// whose log already has lines but no head is refused, not re-anchored: deleting
/// the head is exactly what someone cutting the tail would do.
///
/// One exception, for the message's sake only: a log whose first line carries no
/// chain link predates D78. It is left for the line check to refuse, so the
/// operator hears "written by an older version", not "tampered with" (the D73
/// distinction). It is still refused.
pub(crate) fn ensure_head(log: &Path, key: &[u8], what: &str) -> io::Result<()> {
    if head_path(log).exists() {
        return Ok(());
    }
    let first_line = match std::fs::read_to_string(log) {
        Ok(text) => text
            .lines()
            .find(|l| !l.trim().is_empty())
            .map(str::to_string),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    if let Some(first) = first_line {
        let unchained = serde_json::from_str::<serde_json::Value>(&first)
            .map(|v| v.get("seq").is_none())
            .unwrap_or(false);
        if unchained {
            return Ok(());
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{what} {} has entries but no head anchor ({}). Either it was written \
                 before logs were chained (D78) or the anchor was removed; the store \
                 refuses to load a log whose end it cannot vouch for. Delete the log, its \
                 key and any head file to start over",
                log.display(),
                head_path(log).display()
            ),
        ));
    }
    write_head(log, key, &Tail::genesis())
}

/// After every line verified: the log must reach the head, and may run at most one
/// line past it. Returns whether it runs past (so `open` can re-anchor).
pub(crate) fn verify_head(log: &Path, key: &[u8], macs: &[String], what: &str) -> io::Result<bool> {
    let refuse = |why: String| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{what} {}: {why}", log.display()),
        )
    };
    let raw = match read_nofollow(&head_path(log)) {
        Ok(raw) => raw,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(refuse(
                "the head anchor is missing, so a truncated chain cannot be told apart \
                 from a complete one (D78). Refusing to load"
                    .to_string(),
            ))
        }
        Err(e) => return Err(e),
    };
    let head: Head = serde_json::from_slice(&raw)
        .map_err(|e| refuse(format!("the head anchor does not parse ({e})")))?;
    if head.v != HEAD_VERSION
        || !constant_time_eq(&head_mac(key, head.seq, &head.mac), &head.head_mac)
    {
        return Err(refuse(
            "the head anchor's MAC does not verify — it was written by something \
             without the key"
                .to_string(),
        ));
    }
    let anchored = match head.seq {
        None => 0,
        Some(seq) => {
            let reaches = usize::try_from(seq)
                .ok()
                .and_then(|i| macs.get(i))
                .is_some_and(|m| constant_time_eq(m, &head.mac));
            if !reaches {
                return Err(refuse(format!(
                    "the chain has {} line(s) but its head anchors line {} — entries were \
                     removed from the end of the log (D78). Refusing to load",
                    macs.len(),
                    seq + 1
                )));
            }
            usize::try_from(seq).unwrap_or(usize::MAX).saturating_add(1)
        }
    };
    // One line of slack covers a crash between an append and its head write; more
    // than that is an old head put back over a longer log (D78).
    if macs.len() > anchored + 1 {
        return Err(refuse(format!(
            "the chain runs {} line(s) past its head anchor — more than a crash can \
             leave; the head was replaced with an older one (D78). Refusing to load",
            macs.len() - anchored
        )));
    }
    Ok(macs.len() > anchored)
}

/// Refuse a load that falls short of what this store instance has already seen:
/// the log was rolled back underneath a running process (D78).
pub(crate) fn check_high_water(
    log: &Path,
    macs: &[String],
    seen: &Tail,
    what: &str,
) -> io::Result<()> {
    let Some(last) = seen.next_seq.checked_sub(1) else {
        return Ok(());
    };
    let intact = usize::try_from(last)
        .ok()
        .and_then(|i| macs.get(i))
        .is_some_and(|m| constant_time_eq(m, &seen.last_mac));
    if intact {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{what} {}: shorter than this process has already seen ({} line(s), \
                 {} seen) — it was rolled back while in use (D78). Refusing to load",
                log.display(),
                macs.len(),
                seen.next_seq
            ),
        ))
    }
}

fn read_nofollow(path: &Path) -> io::Result<Vec<u8>> {
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.custom_flags(o_nofollow());
    }
    let mut buf = Vec::new();
    opts.open(path)?.read_to_end(&mut buf)?;
    Ok(buf)
}

#[cfg(unix)]
fn o_nofollow() -> i32 {
    // Same constant as `approval::libc_o_nofollow`; spelled out, no `libc` dep.
    #[cfg(target_os = "linux")]
    {
        0x2_0000
    }
    #[cfg(not(target_os = "linux"))]
    {
        0x100
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &[u8] = b"0123456789abcdef0123456789abcdef";

    fn chain(n: u64) -> (Vec<(u64, String, String, String)>, Tail) {
        let mut tail = Tail::genesis();
        let mut lines = Vec::new();
        for i in 0..n {
            let payload = format!("event-{i}");
            let mac = line_mac(KEY, tail.next_seq, &tail.last_mac, &payload);
            lines.push((tail.next_seq, tail.last_mac.clone(), mac.clone(), payload));
            tail.next_seq += 1;
            tail.last_mac = mac;
        }
        (lines, tail)
    }

    fn verify_all(lines: &[(u64, String, String, String)]) -> Result<Tail, LineFault> {
        let mut tail = Tail::genesis();
        for (seq, prev, mac, payload) in lines {
            verify_line(KEY, &mut tail, Some(*seq), Some(prev), mac, payload)?;
        }
        Ok(tail)
    }

    #[test]
    fn an_intact_chain_verifies() {
        let (lines, tail) = chain(4);
        assert_eq!(verify_all(&lines), Ok(tail));
    }

    #[test]
    fn a_deleted_middle_line_breaks_the_chain() {
        let (mut lines, _) = chain(4);
        lines.remove(1);
        assert_eq!(verify_all(&lines), Err(LineFault::OutOfChain));
    }

    #[test]
    fn reordered_lines_break_the_chain() {
        let (mut lines, _) = chain(3);
        lines.swap(1, 2);
        assert_eq!(verify_all(&lines), Err(LineFault::OutOfChain));
    }

    #[test]
    fn an_edited_payload_fails_its_mac() {
        let (mut lines, _) = chain(2);
        lines[1].3 = "event-X".to_string();
        assert_eq!(verify_all(&lines), Err(LineFault::BadMac));
    }

    #[test]
    fn a_line_without_a_link_is_unchained() {
        let mut tail = Tail::genesis();
        assert_eq!(
            verify_line(KEY, &mut tail, None, None, "mac", "event"),
            Err(LineFault::Unchained)
        );
    }

    #[test]
    fn the_head_catches_a_cut_tail_and_tolerates_a_crash_ahead() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("log.jsonl");
        let (lines, tail) = chain(3);
        let macs: Vec<String> = lines.iter().map(|l| l.2.clone()).collect();

        write_head(&log, KEY, &tail).unwrap();
        assert!(!verify_head(&log, KEY, &macs, "log").unwrap());

        // Tail cut: the first two lines are still a valid chain, but not the head's.
        let err = verify_head(&log, KEY, &macs[..2], "log").unwrap_err();
        assert!(format!("{err}").contains("removed from the end"), "{err}");

        // Crash between append and head write: the log runs one past the head.
        let (lines4, _) = chain(4);
        let macs4: Vec<String> = lines4.iter().map(|l| l.2.clone()).collect();
        assert!(verify_head(&log, KEY, &macs4, "log").unwrap());

        // Two past is not a crash: an older head was put back.
        let (lines5, _) = chain(5);
        let macs5: Vec<String> = lines5.iter().map(|l| l.2.clone()).collect();
        let err = verify_head(&log, KEY, &macs5, "log").unwrap_err();
        assert!(format!("{err}").contains("past its head"), "{err}");
    }

    #[test]
    fn a_genesis_head_anchors_at_most_one_line() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("log.jsonl");
        write_head(&log, KEY, &Tail::genesis()).unwrap();
        let (lines, _) = chain(2);
        let macs: Vec<String> = lines.iter().map(|l| l.2.clone()).collect();
        assert!(verify_head(&log, KEY, &macs[..1], "log").is_ok());
        assert!(verify_head(&log, KEY, &macs, "log").is_err());
    }

    #[test]
    fn the_high_water_mark_refuses_a_shorter_log() {
        let (lines, tail) = chain(3);
        let macs: Vec<String> = lines.iter().map(|l| l.2.clone()).collect();
        let log = Path::new("log.jsonl");
        assert!(check_high_water(log, &macs, &tail, "log").is_ok());
        assert!(check_high_water(log, &macs[..2], &tail, "log").is_err());
        assert!(check_high_water(log, &[], &Tail::genesis(), "log").is_ok());
    }

    #[test]
    fn a_forged_or_missing_head_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("log.jsonl");
        let (lines, _) = chain(2);
        let macs: Vec<String> = lines.iter().map(|l| l.2.clone()).collect();

        let err = verify_head(&log, KEY, &macs, "log").unwrap_err();
        assert!(format!("{err}").contains("missing"), "{err}");

        write_head(&log, b"another key, another key, another", &Tail::genesis()).unwrap();
        let err = verify_head(&log, KEY, &macs, "log").unwrap_err();
        assert!(format!("{err}").contains("does not verify"), "{err}");
    }

    #[test]
    fn ensure_head_refuses_to_re_anchor_a_populated_log() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("log.jsonl");
        ensure_head(&log, KEY, "log").unwrap(); // fresh store: genesis head
        assert!(head_path(&log).exists());

        std::fs::remove_file(head_path(&log)).unwrap();
        std::fs::write(&log, "{\"line\":1,\"seq\":0}\n").unwrap();
        let err = ensure_head(&log, KEY, "log").unwrap_err();
        assert!(format!("{err}").contains("no head anchor"), "{err}");

        // A pre-D78 first line is left to the line check (the old-version message),
        // and no head is written for it.
        std::fs::write(&log, "{\"line\":1}\n").unwrap();
        assert!(ensure_head(&log, KEY, "log").is_ok());
        assert!(!head_path(&log).exists());
    }
}
