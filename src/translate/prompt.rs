use std::sync::OnceLock;

use regex::Regex;

use crate::subtitle::model::SubtitleSegment;

static LINE_REGEX: OnceLock<Regex> = OnceLock::new();

/// Lines numbered LOCALLY 1..=n (never seg.id). Newlines inside a cue become " / ".
/// source_lang "auto" => "Detect the source language automatically."
pub fn build_translation_prompt(
    segments: &[SubtitleSegment],
    source_lang: &str,
    target_lang: &str,
) -> String {
    let numbered_lines = segments
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let single_line = s
                .original_text
                .replace("\r\n", "\n")
                .replace('\r', "\n")
                .replace('\n', " / ");
            format!("[{}] {}", i + 1, single_line)
        })
        .collect::<Vec<_>>()
        .join("\n");

    let src = if source_lang.trim().eq_ignore_ascii_case("auto") {
        "Detect the source language automatically."
    } else {
        source_lang
    };

    format!(
        "You are a professional subtitle translator.\n\
Translate each numbered subtitle line from {src} into {target_lang}.\n\
Keep it natural and consistent with the dialogue context (pronouns, tone).\n\
Return ONLY the translated lines, one per line, with the SAME numbers, e.g.:\n\
[1] ...\n\
[2] ...\n\
Do not merge, split, skip or add lines. Keep \" / \" where it appears.\n\n\
{numbered_lines}"
    )
}

/// Parses "[k] text" lines; Err if any k in 1..=count missing; ignores extras/out-of-range;
/// " / " converted back to "\n".
pub fn parse_translated_lines(response: &str, count: usize) -> anyhow::Result<Vec<String>> {
    let re = LINE_REGEX
        .get_or_init(|| Regex::new(r"^\s*(?:[-*]\s*)?\[(\d+)\]\s*(.*)$").expect("invalid regex"));

    let mut results: Vec<Option<String>> = vec![None; count];

    for line in response.lines() {
        let trimmed = line.trim();
        if let Some(caps) = re.captures(trimmed) {
            if let Ok(idx) = caps[1].parse::<usize>() {
                if (1..=count).contains(&idx) && results[idx - 1].is_none() {
                    let text = caps[2].trim_end();
                    results[idx - 1] = Some(text.replace(" / ", "\n"));
                }
            }
        }
    }

    let mut out = Vec::with_capacity(count);
    for (i, opt) in results.into_iter().enumerate() {
        match opt {
            Some(s) => out.push(s),
            None => anyhow::bail!(
                "Missing translated line [{}] in translation response",
                i + 1
            ),
        }
    }

    Ok(out)
}
