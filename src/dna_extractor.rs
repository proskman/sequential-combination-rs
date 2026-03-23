// dna_extractor.rs - DNA Extraction Engine (SIMD-accelerated via Rust regex crate)
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

static SECTION_PATTERN: OnceLock<Regex> = OnceLock::new();
static FRONTMATTER_PATTERN: OnceLock<Regex> = OnceLock::new();
static RULE_PATTERN: OnceLock<Regex> = OnceLock::new();

fn section_pattern() -> &'static Regex {
    SECTION_PATTERN.get_or_init(|| Regex::new(r"(?m)^#{1,3}\s+(.+)$").unwrap())
}
fn frontmatter_pattern() -> &'static Regex {
    FRONTMATTER_PATTERN.get_or_init(|| Regex::new(r"(?s)^---\n(.+?)\n---").unwrap())
}
fn rule_pattern() -> &'static Regex {
    RULE_PATTERN.get_or_init(|| Regex::new(r"(?m)^[-*]\s+\*\*(.+?)\*\*.*$").unwrap())
}

#[derive(Debug, Clone)]
pub struct SkillDna {
    pub name: String,
    pub description: String,
    pub sections: Vec<String>,
    pub key_rules: Vec<String>,
    pub metadata: HashMap<String, String>,
}

pub fn extract_dna(skill_name: &str, content: &str) -> SkillDna {
    let metadata = extract_frontmatter(content);
    let sections = extract_sections(content);
    let key_rules = extract_key_rules(content);
    let description = metadata
        .get("description")
        .cloned()
        .unwrap_or_else(|| sections.first().cloned().unwrap_or_default());
    SkillDna { name: skill_name.to_string(), description, sections, key_rules, metadata }
}

fn extract_frontmatter(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if let Some(caps) = frontmatter_pattern().captures(content) {
        for line in caps.get(1).map_or("", |m| m.as_str()).lines() {
            if let Some((k, v)) = line.split_once(':') {
                map.insert(k.trim().to_string(), v.trim().trim_matches('"').to_string());
            }
        }
    }
    map
}

fn extract_sections(content: &str) -> Vec<String> {
    section_pattern().captures_iter(content).map(|c| c[1].trim().to_string()).take(10).collect()
}

fn extract_key_rules(content: &str) -> Vec<String> {
    rule_pattern().captures_iter(content).map(|c| c[1].trim().to_string()).take(5).collect()
}

pub fn format_dna_compact(dna: &SkillDna) -> String {
    let mut parts = vec![format!("## {}", dna.name)];
    parts.push(format!("**Description**: {}", dna.description));
    if !dna.sections.is_empty() { parts.push(format!("**Sections**: {}", dna.sections.join(", "))); }
    if !dna.key_rules.is_empty() { parts.push(format!("**Key Rules**: {}", dna.key_rules.join("; "))); }
    parts.join("\n")
}
