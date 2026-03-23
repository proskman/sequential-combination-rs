// server.rs - MCP Server + Tool Definitions (rmcp 0.1)
// Uses #[tool_box] + #[tool] macro pattern (official rmcp 0.1 API)

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use rmcp::{
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    schemars, tool, tool_box, ServerHandler, ServiceExt,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::config_loader::{load_expert_combos, load_stage_profiles};
use crate::skills_index::SkillsIndexer;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SuggestComboInput {
    task: String,
    stage: String,
    #[serde(default = "default_n")]
    n: usize,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct GetExpertDnaInput {
    skills: Vec<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct LoadComboContentInput {
    skills: Vec<String>,
}

fn default_n() -> usize { 5 }

#[derive(Clone)]
pub struct SequentialCombinationServer {
    indexer: Arc<RwLock<SkillsIndexer>>,
}

impl SequentialCombinationServer {
    pub async fn new() -> Result<Self> {
        let base_dir = std::env::var("MCP_BASE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::current_exe().ok()
                    .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                    .unwrap_or_else(|| PathBuf::from("."))
            });
        let config_dir = base_dir.join("config");
        let skills_dir = base_dir.join("skills");
        std::fs::create_dir_all(&skills_dir).context("Cannot create skills dir")?;
        info!("📂 Base: {:?} | Skills: {:?}", base_dir, skills_dir);
        let stage_profiles = if config_dir.join("stage_profiles.yaml").exists() {
            load_stage_profiles(&config_dir.join("stage_profiles.yaml"))?
        } else {
            warn!("⚠️  stage_profiles.yaml not found.");
            Default::default()
        };
        let expert_combos = if config_dir.join("combos_seed.json").exists() {
            load_expert_combos(&config_dir.join("combos_seed.json"))?
        } else {
            warn!("⚠️  combos_seed.json not found.");
            vec![]
        };
        let indexer = SkillsIndexer::new(skills_dir, stage_profiles, expert_combos).await?;
        Ok(Self { indexer: Arc::new(RwLock::new(indexer)) })
    }
}

#[tool_box]
impl SequentialCombinationServer {
    #[tool(description = "Health check — returns server version and runtime info.")]
    async fn ping(&self) -> Result<String, rmcp::Error> {
        Ok(json!({
            "status": "ok",
            "server": "sequential-combination-rs",
            "version": "1.0.0",
            "runtime": "Rust / Tokio / fastembed (ONNX) / instant-distance (HNSW)"
        }).to_string())
    }

    #[tool(description = "List all available cognitive stages defined in stage_profiles.yaml.")]
    async fn list_stages(&self) -> Result<String, rmcp::Error> {
        let indexer = self.indexer.read().await;
        let stages = indexer.list_stages();
        Ok(json!({ "stages": stages, "count": stages.len() }).to_string())
    }

    #[tool(description = "Suggest best skill combination for a task at a given cognitive stage.")]
    async fn suggest_combo(
        &self,
        #[tool(aggr)] input: SuggestComboInput,
    ) -> Result<String, rmcp::Error> {
        let indexer = self.indexer.read().await;
        let expert_combo = indexer.get_expert_combos().iter()
            .find(|c| c.name.to_lowercase() == input.stage.to_lowercase()).cloned();
        let skills = indexer.suggest_combo(&input.task, &input.stage, input.n);
        Ok(json!({ "message": format!("Suggestions for '{}' at stage '{}':", input.task, input.stage),
            "expert_combo": expert_combo, "skills": skills }).to_string())
    }

    #[tool(description = "Get condensed Expert DNA for a list of skill IDs.")]
    async fn get_expert_dna(
        &self,
        #[tool(aggr)] input: GetExpertDnaInput,
    ) -> Result<String, rmcp::Error> {
        let indexer = self.indexer.read().await;
        let results = indexer.get_expert_dna(&input.skills);
        Ok(json!({ "dna_count": results.len(), "results": results }).to_string())
    }

    #[tool(description = "Load full SKILL.md content for a list of skill IDs.")]
    async fn load_combo_content(
        &self,
        #[tool(aggr)] input: LoadComboContentInput,
    ) -> Result<String, rmcp::Error> {
        let indexer = self.indexer.read().await;
        let contents = indexer.load_combo_content(&input.skills);
        let formatted: Vec<serde_json::Value> = contents.iter()
            .map(|(id, content)| json!({ "skill_id": id, "content": content })).collect();
        Ok(json!({ "loaded": formatted.len(), "skills": formatted }).to_string())
    }
}

#[tool_box]
impl ServerHandler for SequentialCombinationServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "sequential-combination-rs".to_string(),
                version: "1.0.0".to_string(),
            },
            instructions: Some(
                "Sequential Combination MCP: semantic skill search, DNA extraction, combo loading."
                    .to_string()
            ),
        }
    }
}
