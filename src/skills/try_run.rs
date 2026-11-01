use serde::Serialize;

use crate::llm::{ChatClient, LlmError};
use crate::subtitle::model::SubtitleSegment;
use crate::translate::{LlmTranslator, Translator};

use super::model::Skill;
use super::prompt::render_system_prompt;

pub const MAX_TRY_LINES: usize = 20;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TrySkillResult {
    pub lines: Vec<String>,
    pub system_prompt: String,
    pub error: Option<String>,
}

fn failed(system_prompt: String, error: String) -> TrySkillResult {
    TrySkillResult { lines: vec![], system_prompt, error: Some(error) }
}

pub async fn try_skill(
    client: Box<dyn ChatClient>,
    skill: Skill,
    others: &[Skill],
    sample: &str,
    source_lang: &str,
    target_lang: &str,
) -> TrySkillResult {
    let skill = skill.normalized();
    if let Err(e) = skill.validate(others) {
        return failed(String::new(), e);
    }
    let lines: Vec<&str> = sample.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if lines.is_empty() {
        return failed(String::new(), "Nhập ít nhất 1 dòng".into());
    }
    if lines.len() > MAX_TRY_LINES {
        return failed(String::new(), "Tối đa 20 dòng".into());
    }
    let segments: Vec<SubtitleSegment> = lines
        .iter()
        .enumerate()
        .map(|(i, l)| SubtitleSegment::new(i + 1, i as u64 * 1000, i as u64 * 1000 + 900, *l))
        .collect();
    let system_prompt = render_system_prompt(&skill, &segments, source_lang, target_lang);
    let translator = LlmTranslator::new(client, Some(skill));
    match translator.translate_batch(&segments, source_lang, target_lang).await {
        Ok(lines) => TrySkillResult { lines, system_prompt, error: None },
        Err(e) => {
            let msg = e
                .chain()
                .find_map(|c| c.downcast_ref::<LlmError>())
                .map(|l| l.user_message())
                .unwrap_or_else(|| format!("{e:#}"));
            failed(system_prompt, msg)
        }
    }
}
