use serde::{Deserialize, Serialize};

use super::model::{GlossaryEntry, Skill};

pub const FORMAT: &str = "echo-skill";
pub const FORMAT_VERSION: u64 = 1;
pub const MAX_IMPORT_BYTES: usize = 1_048_576;

#[derive(Serialize, Deserialize)]
struct SkillBody {
    name: String,
    #[serde(default)]
    description: String,
    system_prompt: String,
    #[serde(default)]
    glossary: Vec<GlossaryEntry>,
}

#[derive(Serialize, Deserialize)]
struct SkillFile {
    format: String,
    version: u64,
    skill: SkillBody,
}

pub fn export_json(skill: &Skill) -> String {
    let file = SkillFile {
        format: FORMAT.into(),
        version: FORMAT_VERSION,
        skill: SkillBody {
            name: skill.name.clone(),
            description: skill.description.clone(),
            system_prompt: skill.system_prompt.clone(),
            glossary: skill.glossary.clone(),
        },
    };
    serde_json::to_string_pretty(&file).expect("skill serializes")
}

pub fn parse_import(text: &str) -> Result<Skill, String> {
    if text.len() > MAX_IMPORT_BYTES {
        return Err("File skill quá lớn (tối đa 1 MB)".into());
    }
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("File skill không hợp lệ: {e}"))?;
    if value.get("format").and_then(|v| v.as_str()) != Some(FORMAT) {
        return Err("File không phải skill Echo".into());
    }
    match value.get("version").and_then(|v| v.as_u64()) {
        Some(FORMAT_VERSION) => {}
        other => {
            let shown = other.map(|v| v.to_string()).unwrap_or_else(|| "?".into());
            return Err(format!("Phiên bản skill không hỗ trợ: {shown}"));
        }
    }
    let file: SkillFile =
        serde_json::from_value(value).map_err(|e| format!("File skill không hợp lệ: {e}"))?;
    Ok(Skill {
        id: String::new(),
        name: file.skill.name,
        description: file.skill.description,
        system_prompt: file.skill.system_prompt,
        glossary: file.skill.glossary,
    }
    .normalized())
}

pub fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    format!("{}.echo-skill.json", cleaned.trim())
}
