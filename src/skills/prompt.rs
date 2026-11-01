use crate::subtitle::model::SubtitleSegment;

use super::model::{GlossaryEntry, Skill};

pub const MAX_TERMS_PER_BATCH: usize = 100;
pub const GLOSSARY_HEADER: &str =
    "Glossary — use these translations exactly; \"(keep)\" means keep the original term unchanged:";

pub fn substitute(template: &str, source_lang: &str, target_lang: &str) -> String {
    let src = if source_lang.trim().eq_ignore_ascii_case("auto") {
        "the source language"
    } else {
        source_lang
    };
    template.replace("{source_lang}", src).replace("{target_lang}", target_lang)
}

fn term_matches(haystack: &str, term: &str) -> bool {
    if !term.is_ascii() {
        return haystack.contains(term);
    }
    haystack.match_indices(term).any(|(i, m)| {
        let before_ok = haystack[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !c.is_ascii_alphanumeric());
        let after_ok = haystack[i + m.len()..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_ascii_alphanumeric());
        before_ok && after_ok
    })
}

pub fn glossary_for_batch<'a>(
    glossary: &'a [GlossaryEntry],
    batch: &[SubtitleSegment],
) -> Vec<&'a GlossaryEntry> {
    let haystack = batch
        .iter()
        .map(|s| s.original_text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    let mut found: Vec<&GlossaryEntry> = glossary
        .iter()
        .filter(|e| {
            let term = e.source.trim().to_lowercase();
            !term.is_empty() && term_matches(&haystack, &term)
        })
        .collect();
    found.sort_by(|a, b| {
        b.source
            .chars()
            .count()
            .cmp(&a.source.chars().count())
            .then_with(|| a.source.cmp(&b.source))
    });
    found.truncate(MAX_TERMS_PER_BATCH);
    found
}

pub fn glossary_applies(skill: &Skill, target_lang: &str) -> bool {
    !skill.is_builtin() || target_lang.trim().to_lowercase().starts_with("vi")
}

pub fn render_system_prompt(
    skill: &Skill,
    batch: &[SubtitleSegment],
    source_lang: &str,
    target_lang: &str,
) -> String {
    let mut out = substitute(&skill.system_prompt, source_lang, target_lang);
    if glossary_applies(skill, target_lang) {
        let terms = glossary_for_batch(&skill.glossary, batch);
        if !terms.is_empty() {
            out.push_str("\n\n");
            out.push_str(GLOSSARY_HEADER);
            for t in terms {
                let target = if t.target.trim().is_empty() { "(keep)" } else { t.target.as_str() };
                out.push_str(&format!("\n- {} → {}", t.source, target));
            }
        }
    }
    out
}
