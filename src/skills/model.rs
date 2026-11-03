use serde::{Deserialize, Serialize};

pub const NO_SKILL: &str = "none";
pub const BUILTIN_PREFIX: &str = "builtin:";
pub const MAX_PROMPT_CHARS: usize = 8000;
pub const MAX_GLOSSARY: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlossaryEntry {
    pub source: String,
    #[serde(default)]
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Skill {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub system_prompt: String,
    #[serde(default)]
    pub glossary: Vec<GlossaryEntry>,
}

impl Skill {
    pub fn is_builtin(&self) -> bool {
        self.id.starts_with(BUILTIN_PREFIX)
    }

    pub fn normalized(mut self) -> Self {
        self.name = self.name.trim().to_string();
        self.description = self.description.trim().to_string();
        self.system_prompt = self.system_prompt.trim().to_string();
        for e in &mut self.glossary {
            e.source = e.source.trim().to_string();
            e.target = e.target.trim().to_string();
        }
        self
    }

    pub fn validate(&self, others: &[Skill]) -> Result<(), String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Tên skill không được để trống".into());
        }
        let lower = name.to_lowercase();
        if others
            .iter()
            .any(|o| o.id != self.id && o.name.trim().to_lowercase() == lower)
        {
            return Err("Tên skill đã tồn tại".into());
        }
        if self.system_prompt.trim().is_empty() {
            return Err("System prompt không được để trống".into());
        }
        if self.system_prompt.chars().count() > MAX_PROMPT_CHARS {
            return Err("System prompt tối đa 8000 ký tự".into());
        }
        if self.glossary.len() > MAX_GLOSSARY {
            return Err("Glossary tối đa 1000 thuật ngữ".into());
        }
        let mut seen = std::collections::HashSet::new();
        for e in &self.glossary {
            let src = e.source.trim();
            if src.is_empty() {
                return Err("Thuật ngữ gốc không được để trống".into());
            }
            if !seen.insert(src.to_lowercase()) {
                return Err(format!("Thuật ngữ bị trùng: {src}"));
            }
        }
        Ok(())
    }
}

pub fn unique_name(base: &str, taken: &[String]) -> String {
    let is_taken = |n: &str| {
        taken
            .iter()
            .any(|t| t.trim().to_lowercase() == n.to_lowercase())
    };
    if !is_taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|i| format!("{base} ({i})"))
        .find(|n| !is_taken(n))
        .expect("infinite iterator")
}
