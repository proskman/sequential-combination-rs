// skills_index.rs - Vector Search Engine (CORRECTED)
// Fixes:
//   - REMOVED: unsafe block (safety violation)
//   - FIXED: TextEmbedding wrapped in Arc<Mutex<>> for thread-safe sharing
//   - FIXED: fastembed InitOptions::new() builder pattern
//   - FIXED: embed() clone properly before spawn_blocking

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use instant_distance::{Builder, HnswMap, Search};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

use crate::config_loader::{ExpertCombo, StageProfiles};
use crate::dna_extractor::{extract_dna, format_dna_compact};

#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub content_preview: String,
}

#[derive(Clone)]
pub struct EmbeddingPoint(pub Vec<f32>);

impl instant_distance::Point for EmbeddingPoint {
    fn distance(&self, other: &Self) -> f32 {
        let dot: f32 = self.0.iter().zip(other.0.iter()).map(|(a, b)| a * b).sum();
        let na: f32 = self.0.iter().map(|x| x * x).sum::<f32>().sqrt();
        let nb: f32 = other.0.iter().map(|x| x * x).sum::<f32>().sqrt();
        if na == 0.0 || nb == 0.0 { return 1.0; }
        1.0 - (dot / (na * nb))
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub id: String,
    pub name: String,
    pub description: String,
    pub score: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SuggestedSkill {
    pub id: String,
    pub name: String,
    pub score: f32,
    pub description: String,
}

pub struct SkillsIndexer {
    embedding_model: Arc<Mutex<TextEmbedding>>,
    hnsw: Option<HnswMap<EmbeddingPoint, usize>>,
    skills: Vec<SkillEntry>,
    stage_profiles: StageProfiles,
    expert_combos: Vec<ExpertCombo>,
    skills_dir: PathBuf,
}

impl SkillsIndexer {
    pub async fn new(
        skills_dir: PathBuf,
        stage_profiles: StageProfiles,
        expert_combos: Vec<ExpertCombo>,
    ) -> Result<Self> {
        info!("🔧 Loading all-MiniLM-L6-v2 embedding model via ONNX...");
        let embedding_model = tokio::task::spawn_blocking(|| {
            TextEmbedding::try_new(
                InitOptions::new(EmbeddingModel::AllMiniLML6V2)
                    .with_show_download_progress(false),
            )
        })
        .await?
        .context("Failed to initialize fastembed ONNX model")?;
        info!("✅ Embedding model loaded.");
        let model_arc = Arc::new(Mutex::new(embedding_model));
        let mut indexer = Self {
            embedding_model: model_arc,
            hnsw: None,
            skills: Vec::new(),
            stage_profiles,
            expert_combos,
            skills_dir,
        };
        indexer.build_index().await?;
        Ok(indexer)
    }

    async fn build_index(&mut self) -> Result<()> {
        info!("📚 Scanning {:?} for SKILL.md files...", self.skills_dir);
        let skill_files = self.collect_skill_files();
        if skill_files.is_empty() {
            warn!("⚠️  No SKILL.md files found.");
            return Ok(());
        }
        info!("  Found {} skills.", skill_files.len());
        let mut documents: Vec<String> = Vec::new();
        let mut entries: Vec<SkillEntry> = Vec::new();
        for path in &skill_files {
            match std::fs::read_to_string(path) {
                Ok(content) => {
                    let id = self.extract_skill_id(path);
                    let dna = extract_dna(&id, &content);
                    let doc = format!("{} {}", dna.name, dna.description);
                    entries.push(SkillEntry {
                        id: id.clone(), name: id, description: dna.description.clone(),
                        path: path.clone(), content_preview: dna.sections.join(", "),
                    });
                    documents.push(doc);
                }
                Err(e) => debug!("Skipping {:?}: {}", path, e),
            }
        }
        info!("🧠 Generating embeddings for {} documents...", documents.len());
        let model_clone = Arc::clone(&self.embedding_model);
        let embeddings = tokio::task::spawn_blocking(move || {
            let model = model_clone.lock().unwrap();
            let refs: Vec<&str> = documents.iter().map(String::as_str).collect();
            model.embed(refs, None)
        })
        .await?
        .context("Embedding generation failed")?;
        info!("🏗️  Building HNSW index...");
        let points: Vec<EmbeddingPoint> = embeddings.into_iter().map(EmbeddingPoint).collect();
        let values: Vec<usize> = (0..entries.len()).collect();
        self.hnsw = Some(Builder::default().build(points, values));
        self.skills = entries;
        info!("✅ HNSW index ready ({} skills).", self.skills.len());
        Ok(())
    }

    fn collect_skill_files(&self) -> Vec<PathBuf> {
        WalkDir::new(&self.skills_dir).follow_links(true).into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_str() == Some("SKILL.md"))
            .map(|e| e.path().to_owned())
            .collect()
    }

    fn extract_skill_id(&self, path: &Path) -> String {
        path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str())
            .unwrap_or("unknown").to_lowercase().replace(' ', "-")
    }

    pub fn query_skills(&self, query: &str, n: usize) -> Result<Vec<SearchResult>> {
        let hnsw = match &self.hnsw { Some(h) => h, None => return Ok(vec![]) };
        let embedding = {
            let model = self.embedding_model.lock().unwrap();
            model.embed(vec![query], None)?
        };
        let point = EmbeddingPoint(embedding.into_iter().next().unwrap_or_default());
        let mut search = Search::default();
        Ok(hnsw.search(&point, &mut search).take(n).map(|item| {
            let skill = &self.skills[*item.value];
            SearchResult { id: skill.id.clone(), name: skill.name.clone(),
                description: skill.description.clone(), score: 1.0 - item.distance }
        }).collect())
    }

    pub fn list_stages(&self) -> Vec<String> {
        let mut v: Vec<String> = self.stage_profiles.keys().cloned().collect();
        v.sort(); v
    }

    pub fn suggest_combo(&self, task: &str, stage: &str, n: usize) -> Vec<SuggestedSkill> {
        let semantic = self.query_skills(task, n * 2).unwrap_or_default();
        let stage_kws = self.stage_profiles.get(stage).cloned().unwrap_or_default();
        let task_lower = task.to_lowercase();
        let mut scored: Vec<(usize, f32)> = semantic.iter().enumerate().map(|(i, r)| {
            let mut score = r.score;
            for kw in &stage_kws {
                if r.description.to_lowercase().contains(kw.as_str()) || r.id.contains(kw.as_str()) { score += 0.08; }
            }
            for word in task_lower.split_whitespace() {
                if r.id.contains(word) || r.description.to_lowercase().contains(word) { score += 0.05; }
            }
            (i, score)
        }).collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(n).map(|(i, score)| {
            let r = &semantic[i];
            SuggestedSkill { id: r.id.clone(), name: r.name.clone(), score, description: r.description.clone() }
        }).collect()
    }

    pub fn get_expert_dna(&self, skill_ids: &[String]) -> Vec<String> {
        skill_ids.iter().filter_map(|id| self.find_skill_path(id))
            .filter_map(|path| std::fs::read_to_string(&path).ok().map(|c| {
                let name = path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("unknown").to_string();
                format_dna_compact(&extract_dna(&name, &c))
            })).collect()
    }

    pub fn load_combo_content(&self, skill_ids: &[String]) -> Vec<(String, String)> {
        skill_ids.iter().filter_map(|id| {
            self.find_skill_path(id).and_then(|p| std::fs::read_to_string(&p).ok().map(|c| (id.clone(), c)))
        }).collect()
    }

    fn find_skill_path(&self, skill_id: &str) -> Option<PathBuf> {
        self.skills.iter()
            .find(|s| s.id == skill_id || s.name.to_lowercase() == skill_id.to_lowercase())
            .map(|s| s.path.clone())
    }

    pub fn get_expert_combos(&self) -> &[ExpertCombo] { &self.expert_combos }
}

pub type SharedIndexer = Arc<RwLock<SkillsIndexer>>;
