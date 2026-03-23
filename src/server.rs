// server.rs - MCP Server + Tool Definitions (CORRECTED for rmcp 0.1 API)
//
// Fixes verified via Perplexity documentation:
//   - REMOVED: Manual list_tools() / call_tool() implementation
//   - ADDED: #[tool_box] + #[tool] macro pattern (official rmcp 0.1 API)
//   - ADDED: schemars::JsonSchema derive on all input structs
//   - FIXED: get_info() returns ProtocolVersion + ServerCapabilities correctly
//

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use rmcp::{
    model::{
        Implementation, ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    schemars, tool, tool_box, ServerHandler,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::config_loader::{load_expert_combos, load_stage_profiles};
use crate::skills_index::SkillsIndexer;

// ===========================================================================
// Input Parameter Structs
// All must implement schemars::JsonSchema for rmcp macro introspection
// ===========================================================================

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
struct SuggestComboInput {
    /// Description of the task to find skills for
    task: String,
    /// Cognitive stage name (e.g. \"Synthesis\", \"Analysis\")
    stage: String,
    /// Number of skills to return (default: 5)
    #[serde(default = \"default_n\")]
    n: usize,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
struct GetExpertDnaInput {
    /// List of skill IDs to extract DNA from
    skills: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
struct LoadComboContentInput {
    /// List of skill IDs to load SKILL.md content for
    skills: Vec<String>,
}

fn default_n() -> usize {
    5
}

// ===========================================================================
// Server Struct
// ===========================================================================

#[derive(Clone)]
pub struct SequentialCombinationServer {
    indexer: Arc<RwLock<SkillsIndexer>>,
}

impl SequentialCombinationServer {
    /// Initialize server: load configs, build HNSW index, warm up ONNX model
    pub async fn new() -> Result<Self> {
        // Resolve base directory from env or executable path
        let base_dir = std::env::var(\"MCP_BASE_DIR\")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                    .unwrap_or_else(|| PathBuf::from(\".\"))
            });

        let config_dir = base_dir.join(\"config\");
        let skills_dir = base_dir.join(\"skills\");

        std::fs::create_dir_all(&skills_dir)
            .context(\"Cannot create skills directory\")?;

        info!(\"📂 Base: {:?} | Skills: {:?}\", base_dir, skills_dir);

        let stage_profiles_path = config_dir.join(\"stage_profiles.yaml\");
        let combos_seed_path = config_dir.join(\"combos_seed.json\");

        let stage_profiles = if stage_profiles_path.exists() {
            load_stage_profiles(&stage_profiles_path)?
        } else {
            warn!(\"⚠️  stage_profiles.yaml not found — stages will be empty.\");
            Default::default()
        };

        let expert_combos = if combos_seed_path.exists() {
            load_expert_combos(&combos_seed_path)?
        } else {
            warn!(\"⚠️  combos_seed.json not found — no expert combos.\");
            vec![]
        };

        let indexer = SkillsIndexer::new(skills_dir, stage_profiles, expert_combos).await?;

        Ok(Self {
            indexer: Arc::new(RwLock::new(indexer)),
        })
    }
}

// ===========================================================================
// Tool Definitions (rmcp 0.1 macro pattern)
// #[tool_box] on the impl block, #[tool] on each async method
// ===========================================================================

#[tool_box]
impl SequentialCombinationServer {
    /// Health check — returns server status, version and runtime info.
    #[tool(description = \"Health check — returns server version and runtime info.\")]
    async fn ping(&self) -> Result<String, rmcp::Error> {
        Ok(json!({
            \"status\": \"ok\",
            \"server\": \"sequential-combination-rs\",
            \"version\": \"1.0.0\",
            \"runtime\": \"Rust / Tokio / fastembed (ONNX) / instant-distance (HNSW)\"
        })
        .to_string())
    }

    /// List all available cognitive stages from stage_profiles.yaml.
    #[tool(description = \"List all available cognitive stages defined in stage_profiles.yaml.\")]
    async fn list_stages(&self) -> Result<String, rmcp::Error> {
        let indexer = self.indexer.read().await;
        let stages = indexer.list_stages();
        Ok(json!({ \"stages\": stages, \"count\": stages.len() }).to_string())
    }

    /// Suggest the best skill combination for a task and cognitive stage.
    #[tool(description = \"Suggest best skill combination for a task at a given cognitive stage.\")]
    async fn suggest_combo(
        &self,
        #[tool(aggr)] input: SuggestComboInput,
    ) -> Result<String, rmcp::Error> {
        let indexer = self.indexer.read().await;
        let combos = indexer.get_expert_combos();
        let expert_combo = combos
            .iter()
            .find(|c| c.name.to_lowercase() == input.stage.to_lowercase())
            .cloned();
        let skills = indexer.suggest_combo(&input.task, &input.stage, input.n);
        Ok(json!({
            \"message\": format!(\"Suggestions for '{}' at stage '{}':\", input.task, input.stage),
            \"expert_combo\": expert_combo,
            \"skills\": skills,
        })
        .to_string())
    }

    /// Get condensed 'Expert DNA' for a list of skill IDs.
    #[tool(description = \"Get condensed Expert DNA (key rules, sections, description) for a list of skill IDs.\")]
    async fn get_expert_dna(
        &self,
        #[tool(aggr)] input: GetExpertDnaInput,
    ) -> Result<String, rmcp::Error> {
        let indexer = self.indexer.read().await;
        let results = indexer.get_expert_dna(&input.skills);
        Ok(json!({ \"dna_count\": results.len(), \"results\": results }).to_string())
    }

    /// Load full SKILL.md content for a list of skill IDs.
    #[tool(description = \"Load full SKILL.md file content for a list of skill IDs.\")]
    async fn load_combo_content(
        &self,
        #[tool(aggr)] input: LoadComboContentInput,
    ) -> Result<String, rmcp::Error> {
        let indexer = self.indexer.read().await;
        let contents = indexer.load_combo_content(&input.skills);
        let formatted: Vec<serde_json::Value> = contents
            .iter()
            .map(|(id, content)| json!({ \"skill_id\": id, \"content\": content }))
            .collect();
        Ok(json!({ \"loaded\": formatted.len(), \"skills\": formatted }).to_string())
    }
}

// ===========================================================================
// ServerHandler Implementation
// #[tool_box] here links the tool routing to the impl block above
// ===========================================================================

#[tool_box]
impl ServerHandler for SequentialCombinationServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: \"sequential-combination-rs\".to_string(),
                version: \"1.0.0\".to_string(),
            },
            instructions: Some(
                \"Sequential Combination MCP: semantic skill search, DNA extraction, and expert combo loading.\"
                    .to_string(),
            ),
        }
    }

    async fn handle_request(
        &self,
        req: rmcp::Request,
        client: &mut rmcp::Client,
    ) -> Result<rmcp::Response, rmcp::Error> {
        match req {
            rmcp::Request::ListTools(_) => {
                Ok(rmcp::Response::ListTools(tool_box!(@list_tools)))
            }
            rmcp::Request::CallTool(call) => {
                tool_box!(@call_tool self, call)
            }
            _ => Err(rmcp::Error::method_not_found(\"Method not supported\")),
        }
    }
}
