//! Loading the pack: one world, one mock upstream, one reference-gateway policy,
//! and the scenario files.
//!
//! Everything here is data on disk. Adding a scenario is adding a file; nothing
//! in the runner enumerates them.

use std::fs;
use std::path::{Path, PathBuf};

use compiler::{compile, loader::load_yaml};
use harness_types::CompiledWorld;

use crate::scenario::Scenario;
use crate::targets::WeakPolicy;
use crate::upstream::UpstreamTool;

pub struct Pack {
    pub root: PathBuf,
    pub world_path: PathBuf,
    pub world: CompiledWorld,
    pub tools: Vec<UpstreamTool>,
    pub weak_policy: WeakPolicy,
    pub scenarios: Vec<Scenario>,
}

impl Pack {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, String> {
        let root = root.as_ref().to_path_buf();
        let world_path = root.join("world.yaml");
        let manifest =
            load_yaml(&read(&world_path)?).map_err(|e| format!("{}: {e}", world_path.display()))?;
        let world = compile(&manifest).map_err(|e| format!("{}: {e}", world_path.display()))?;

        let upstream_path = root.join("upstream.yaml");
        let tools: Vec<UpstreamTool> = serde_yaml::from_str(&read(&upstream_path)?)
            .map_err(|e| format!("{}: {e}", upstream_path.display()))?;

        let policy_path = root.join("weak-gateway.yaml");
        let weak_policy: WeakPolicy = serde_yaml::from_str(&read(&policy_path)?)
            .map_err(|e| format!("{}: {e}", policy_path.display()))?;

        let dir = root.join("scenarios");
        let mut files: Vec<PathBuf> = fs::read_dir(&dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "yaml"))
            .collect();
        files.sort();
        let mut scenarios = Vec::new();
        for file in files {
            let scenario: Scenario = serde_yaml::from_str(&read(&file)?)
                .map_err(|e| format!("{}: {e}", file.display()))?;
            scenario
                .validate()
                .map_err(|e| format!("{}: {e}", file.display()))?;
            scenarios.push(scenario);
        }
        if scenarios.is_empty() {
            return Err(format!("{}: no scenarios", dir.display()));
        }
        Ok(Self {
            root,
            world_path,
            world,
            tools,
            weak_policy,
            scenarios,
        })
    }
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}
