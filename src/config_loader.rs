// config_loader.rs - Configuration Loader
use std::collections::HashMap;
use std::path::Path;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub type StageProfiles = HashMap<String, Vec<String>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpertCombo {
    pub name: String,
    pub skills: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CombosSeed {
    combos: Vec<ExpertCombo>,
}

pub fn load_stage_profiles(path: &Path) -> Result<StageProfiles> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Cannot read stage_profiles.yaml at {:?}", path))?;
    serde_yaml::from_str(&content).with_context(|| "Failed to parse stage_profiles.yaml")
}

pub fn load_expert_combos(path: &Path) -> Result<Vec<ExpertCombo>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Cannot read combos_seed.json at {:?}", path))?;
    let seed: CombosSeed = serde_json::from_str(&content)
        .with_context(|| "Failed to parse combos_seed.json")?;
    Ok(seed.combos)
}
