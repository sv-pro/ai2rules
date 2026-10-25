//! What a host hook does when its world cannot be read or compiled — the
//! **broken-world mode** (D87, AI2-56 / #128). Shared by `cc_hook` and `agy_hook`.
//!
//! Until D87 both hooks treated a broken world as a process failure and failed
//! open: the call went to the host's own flow, ungoverned, and nobody was told. A
//! broken world is not only an accident, though. An agent that can write the world
//! file can break it on purpose, so "fail open on a bad world" is a kill-switch the
//! agent can throw. The choice between keeping work moving and keeping protection
//! in place belongs to the person or administrator, so it is a flag that lives
//! **outside the world** (a broken world cannot say how it wants to be handled):
//!
//! - `closed`: every call is refused, with the reason and the way out.
//! - `open`: the call proceeds ungoverned, with a warning.
//! - `last-good` (default): decide with the last world that compiled here, with a
//!   warning that the new one did not apply. When none ever compiled, `closed`.
//!
//! Every mode warns: the hook attaches a user-facing message to its answer, the
//! decision log (where there is one) records the state, and `harness doctor`
//! reports the mode and the cache.
//!
//! The bootstrap shim's "no binary, fail open" (D37) is unchanged: without a
//! binary there is no hook to have a mode.

use crate::outcome::{now_ms, HostIds};
use compiler::sha256_hex;
use harness_types::CompiledWorld;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The broken-world modes, as the hooks' `--on-broken-world` flag spells them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum BrokenWorldMode {
    /// Refuse every call until the world compiles again.
    Closed,
    /// Let every call through ungoverned, with a warning.
    Open,
    /// Decide with the last world that compiled here; `closed` if none ever did.
    LastGood,
}

impl BrokenWorldMode {
    pub fn as_str(self) -> &'static str {
        match self {
            BrokenWorldMode::Closed => "closed",
            BrokenWorldMode::Open => "open",
            BrokenWorldMode::LastGood => "last-good",
        }
    }
}

/// The last-known-good cache, one file in the hook's state directory: the world
/// text that last compiled, its SHA-256, and the world path it was read from. One
/// file written atomically, so a reader never sees a text from one save and a hash
/// from another.
pub const LAST_GOOD_FILE: &str = "last-good-world.json";

/// The live world did not load. What the hook knows about it, for the warning.
#[derive(Debug, Clone)]
pub struct BrokenWorld {
    /// The mode the hook was configured with.
    pub mode: BrokenWorldMode,
    /// What this call actually got: `last-good` without a cached world falls back
    /// to `closed`, and the warning says so.
    pub applied: BrokenWorldMode,
    pub world_path: PathBuf,
    /// Why the world did not load, as the reader or the compiler said it (a YAML
    /// error carries its line and column).
    pub error: String,
    /// SHA-256 of the cached world text this call is decided with (`last-good`).
    pub last_good_sha256: Option<String>,
}

/// What the hook decides with.
pub enum WorldLoad {
    /// The live world compiled: business as usual, nothing to warn about.
    Live(CompiledWorld),
    /// The live world did not, and the mode says what happens instead. `world` is
    /// the cached last-good world when there is one to decide with; `None` means
    /// the hook answers without a kernel verdict (`closed` refuses, `open` passes).
    Broken {
        world: Option<CompiledWorld>,
        broken: BrokenWorld,
    },
}

/// Load the world the hook decides with. `compile` is the host's own compile (roots
/// resolved against its base and home), applied to the live text and, when that
/// fails under `last-good`, to the cached text.
///
/// Under `last-good` a live world that compiles is cached when its text changed.
/// A cache that cannot be written costs only the fallback, never the call, and the
/// other two modes keep no cache at all.
pub fn load(
    world_path: &Path,
    state_dir: &Path,
    mode: BrokenWorldMode,
    compile: impl Fn(&str) -> Result<CompiledWorld, String>,
) -> WorldLoad {
    let live = std::fs::read_to_string(world_path)
        .map_err(|e| format!("cannot read {}: {e}", world_path.display()))
        .and_then(|text| compile(&text).map(|w| (text, w)));
    let error = match live {
        Ok((text, world)) => {
            if mode == BrokenWorldMode::LastGood {
                remember(state_dir, world_path, &text);
            }
            return WorldLoad::Live(world);
        }
        Err(e) => e,
    };
    let mut broken = BrokenWorld {
        mode,
        applied: mode,
        world_path: world_path.to_path_buf(),
        error,
        last_good_sha256: None,
    };
    let world = match mode {
        BrokenWorldMode::Closed | BrokenWorldMode::Open => None,
        BrokenWorldMode::LastGood => match recall(state_dir, world_path)
            .and_then(|(text, sha)| compile(&text).ok().map(|w| (w, sha)))
        {
            Some((world, sha)) => {
                broken.last_good_sha256 = Some(sha);
                Some(world)
            }
            None => {
                broken.applied = BrokenWorldMode::Closed;
                None
            }
        },
    };
    WorldLoad::Broken { world, broken }
}

/// Cache `text` as the last world that compiled, unless it already is. "Already
/// is" checks the cached text, not only its recorded hash, so a torn or edited
/// cache is rewritten by the next good call.
fn remember(state_dir: &Path, world_path: &Path, text: &str) {
    let sha = sha256_hex(text.as_bytes());
    let file = state_dir.join(LAST_GOOD_FILE);
    if recall(state_dir, world_path).is_some_and(|(_, cached)| cached == sha) {
        return;
    }
    let saved_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let cache = json!({
        "world": path_key(world_path),
        "sha256": sha,
        "saved_ms": saved_ms,
        "text": text,
    });
    let _ = std::fs::create_dir_all(state_dir);
    if let Err(e) = crate::local_page::write_atomically(&file, &cache.to_string()) {
        eprintln!(
            "harness: last-good world cache {} not written: {e}",
            file.display()
        );
    }
}

/// The cached world text and its hash, if a cache exists for this world path and
/// its text still matches its hash. A cache for another world path is not this
/// world's last good state (two hooks may share a state directory).
fn recall(state_dir: &Path, world_path: &Path) -> Option<(String, String)> {
    let cache = read_cache(&state_dir.join(LAST_GOOD_FILE))?;
    if cache["world"].as_str() != Some(&path_key(world_path)) {
        return None;
    }
    let text = cache["text"].as_str()?;
    let sha = cache["sha256"].as_str()?;
    (sha256_hex(text.as_bytes()) == sha).then(|| (text.to_string(), sha.to_string()))
}

fn read_cache(file: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()
}

fn path_key(world_path: &Path) -> String {
    world_path.to_string_lossy().into_owned()
}

/// What `harness doctor` shows about a state directory's cache: its hash and when
/// it was saved, if its text still matches its hash. Not matched against a world
/// path: doctor reads the project from outside the shim, and the shim's spelling
/// of the path (`$PD/...` under Git Bash, say) is not doctor's.
pub fn cached_last_good(state_dir: &Path) -> Option<(String, u64)> {
    let cache = read_cache(&state_dir.join(LAST_GOOD_FILE))?;
    let sha = cache["sha256"].as_str()?;
    (sha256_hex(cache["text"].as_str()?.as_bytes()) == sha)
        .then(|| (sha.to_string(), cache["saved_ms"].as_u64().unwrap_or(0)))
}

/// The warning this hook process attaches to whatever it answers. A hook process
/// answers exactly one call, so this is set at most once, right after the world
/// loads, and every exit path (`emit`, the silent passthrough) reads it.
static WARNING: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn set_warning(message: String) {
    let _ = WARNING.set(message);
}

pub fn warning() -> Option<&'static str> {
    WARNING.get().map(String::as_str)
}

/// The decision-log line for a call answered without a kernel verdict (`closed`
/// refuses, `open` passes). Same fields as a kernel line, so the console's feed
/// reads it; `decision` is `BROKEN_WORLD` because no world decided it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn log_line(
    session: &str,
    tool: &str,
    call: &str,
    tainted: bool,
    broken: &BrokenWorld,
    answer: Option<&str>,
    reason: &str,
    ids: &HostIds,
) -> Value {
    let mut line = json!({
        "ts_ms": now_ms(),
        "kind": "decision",
        "session": session,
        "tool": tool,
        "action": null,
        "call": call,
        "decision": "BROKEN_WORLD",
        "rule": "broken_world",
        "reason": reason,
        "answer": answer,
        "taint": if tainted { "tainted" } else { "clean" },
        "manifest_hash": null,
        "broken_world": broken.log_fields(),
    });
    ids.stamp(&mut line);
    line
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(12)]
}

impl BrokenWorld {
    /// The user-facing warning. `way_out` is the host's own off switch, named in
    /// the `closed` message so a refused person knows how to get unstuck.
    pub fn message(&self, way_out: &str) -> String {
        let head = format!(
            "ai2rules: the world {} does not compile ({}).",
            self.world_path.display(),
            self.error
        );
        match (self.applied, self.mode) {
            (BrokenWorldMode::LastGood, _) => format!(
                "{head} Broken-world mode `last-good`: this call is decided by the last world that \
                 compiled here (sha256 {}), so your latest edit has NOT taken effect. Fix the file \
                 to apply it.",
                short(self.last_good_sha256.as_deref().unwrap_or("")),
            ),
            (BrokenWorldMode::Open, _) => format!(
                "{head} Broken-world mode `open`: this call proceeds UNGOVERNED. Fix the file to \
                 restore governance."
            ),
            (BrokenWorldMode::Closed, BrokenWorldMode::LastGood) => format!(
                "{head} Broken-world mode `last-good`, but no world has compiled here yet, so \
                 every tool call is refused. Fix the file, or {way_out}."
            ),
            (BrokenWorldMode::Closed, _) => format!(
                "{head} Broken-world mode `closed`: every tool call is refused until the file \
                 compiles. Fix the file, or {way_out}."
            ),
        }
    }

    /// The `broken_world` object a decision-log line carries while the world is
    /// broken, so the console and a later reader see the same state the person did.
    pub fn log_fields(&self) -> Value {
        json!({
            "mode": self.mode.as_str(),
            "applied": self.applied.as_str(),
            "world": self.world_path.display().to_string(),
            "error": self.error,
            "last_good_sha256": self.last_good_sha256,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "world_id: good\nbase_actions:\n  - { name: read_file, action_type: Read, side_effect: Read }\ncapabilities:\n  - { trust: Trusted, actions: [Read] }\n";

    fn compile_text(text: &str) -> Result<CompiledWorld, String> {
        compiler::load_yaml(text)
            .and_then(|m| compiler::compile(&m))
            .map_err(|e| e.to_string())
    }

    #[test]
    fn a_good_world_is_cached_only_under_last_good() {
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path().join("w.yaml");
        std::fs::write(&world, GOOD).unwrap();
        let state = dir.path().join("state");

        for mode in [BrokenWorldMode::Closed, BrokenWorldMode::Open] {
            assert!(matches!(
                load(&world, &state, mode, compile_text),
                WorldLoad::Live(_)
            ));
            assert!(
                !state.join(LAST_GOOD_FILE).exists(),
                "{mode:?} keeps no cache"
            );
        }
        assert!(matches!(
            load(&world, &state, BrokenWorldMode::LastGood, compile_text),
            WorldLoad::Live(_)
        ));
        let (sha, _) = cached_last_good(&state).expect("cached");
        assert_eq!(sha, sha256_hex(GOOD.as_bytes()));
    }

    #[test]
    fn last_good_decides_with_the_cache_and_names_its_hash() {
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path().join("w.yaml");
        let state = dir.path().join("state");
        std::fs::write(&world, GOOD).unwrap();
        load(&world, &state, BrokenWorldMode::LastGood, compile_text);
        std::fs::write(&world, "world_id: [unterminated").unwrap();

        let WorldLoad::Broken {
            world: Some(_),
            broken,
        } = load(&world, &state, BrokenWorldMode::LastGood, compile_text)
        else {
            panic!("a broken world under last-good must decide with the cache");
        };
        assert_eq!(broken.applied, BrokenWorldMode::LastGood);
        assert_eq!(
            broken.last_good_sha256.as_deref(),
            Some(sha256_hex(GOOD.as_bytes()).as_str())
        );
        let msg = broken.message("touch ~/.claude/gate-off");
        assert!(msg.contains("NOT taken effect"), "{msg}");
        assert!(
            msg.contains("line"),
            "the compile error carries its line: {msg}"
        );
    }

    #[test]
    fn last_good_without_a_cache_is_closed_and_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path().join("w.yaml");
        std::fs::write(&world, "world_id: [unterminated").unwrap();
        let WorldLoad::Broken {
            world: None,
            broken,
        } = load(&world, dir.path(), BrokenWorldMode::LastGood, compile_text)
        else {
            panic!("no cache: nothing to decide with");
        };
        assert_eq!(broken.applied, BrokenWorldMode::Closed);
        assert!(broken
            .message("X")
            .contains("no world has compiled here yet"));
    }

    #[test]
    fn a_tampered_or_foreign_cache_is_not_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let world = dir.path().join("w.yaml");
        std::fs::write(&world, GOOD).unwrap();
        load(&world, dir.path(), BrokenWorldMode::LastGood, compile_text);
        let file = dir.path().join(LAST_GOOD_FILE);

        // Text edited without its hash: a torn or hand-edited cache.
        let mut cache = read_cache(&file).unwrap();
        cache["text"] = json!(GOOD.replace("good", "edited"));
        std::fs::write(&file, cache.to_string()).unwrap();
        assert!(recall(dir.path(), &world).is_none());

        // A cache saved for a different world path.
        load(&world, dir.path(), BrokenWorldMode::LastGood, compile_text);
        assert!(recall(dir.path(), &dir.path().join("other.yaml")).is_none());
        assert!(recall(dir.path(), &world).is_some());
    }
}
