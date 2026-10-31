use std::path::Path;
use std::sync::LazyLock;

use anyhow::{anyhow, Context};
use regex::Regex;

use crate::subtitle::model::{sanitize_segments, SubtitleSegment};

static ASS_OVERRIDE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{[^}]*\}").expect("valid ASS override regex"));

/// Parse a timestamp into milliseconds.
/// Supports formats: "HH:MM:SS,mmm", "HH:MM:SS.mmm", "MM:SS.mmm", "H:MM:SS.cc"
pub fn parse_timestamp(ts: &str) -> Option<u64> {
    let ts = ts.trim();
    if ts.is_empty() {
        return None;
    }

    let parts: Vec<&str> = ts.split(':').collect();
    let (hours, minutes, seconds_part) = match parts.len() {
        2 => (0u64, parts[0], parts[1]),
        3 => {
            if parts[0].is_empty() || !parts[0].chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let h = parts[0].parse::<u64>().ok()?;
            (h, parts[1], parts[2])
        }
        _ => return None,
    };

    if minutes.is_empty() || !minutes.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let minutes = minutes.parse::<u64>().ok()?;

    let (sec_str, frac_str) = if let Some(idx) = seconds_part.find(['.', ',']) {
        (&seconds_part[..idx], &seconds_part[idx + 1..])
    } else {
        (seconds_part, "")
    };

    if sec_str.is_empty() || !sec_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let seconds = sec_str.parse::<u64>().ok()?;

    let ms = if frac_str.is_empty() {
        0u64
    } else {
        if !frac_str.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        if frac_str.len() <= 3 {
            let padded = format!("{frac_str:0<3}");
            padded.parse::<u64>().ok()?
        } else {
            frac_str[..3].parse::<u64>().ok()?
        }
    };

    Some(hours * 3_600_000 + minutes * 60_000 + seconds * 1_000 + ms)
}

fn split_blocks(content: &str) -> Vec<Vec<&str>> {
    let mut blocks = Vec::new();
    let mut current_block = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            if !current_block.is_empty() {
                blocks.push(current_block);
                current_block = Vec::new();
            }
        } else {
            current_block.push(line);
        }
    }
    if !current_block.is_empty() {
        blocks.push(current_block);
    }

    blocks
}

fn parse_srt_or_vtt(content: &str) -> anyhow::Result<Vec<SubtitleSegment>> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    let blocks = split_blocks(&normalized);

    let mut segments = Vec::new();

    for (block_idx, block_lines) in blocks.into_iter().enumerate() {
        let block_num = block_idx + 1;

        let time_line_idx = match block_lines.iter().position(|l| l.contains("-->")) {
            Some(idx) => idx,
            None => continue,
        };

        let time_line = block_lines[time_line_idx];
        let (start_raw, end_raw) = time_line
            .split_once("-->")
            .ok_or_else(|| anyhow!("block {block_num}: invalid time range line '{time_line}'"))?;

        let start_str = start_raw.trim();
        let end_str = end_raw.split_whitespace().next().unwrap_or("");

        let start_ms = parse_timestamp(start_str)
            .ok_or_else(|| anyhow!("block {block_num}: malformed timestamp '{start_str}'"))?;
        let end_ms = parse_timestamp(end_str)
            .ok_or_else(|| anyhow!("block {block_num}: malformed timestamp '{end_str}'"))?;

        let text = block_lines[time_line_idx + 1..].join("\n");
        let id = segments.len() + 1;
        segments.push(SubtitleSegment::new(id, start_ms, end_ms, text));
    }

    Ok(segments)
}

pub fn parse_srt(content: &str) -> anyhow::Result<Vec<SubtitleSegment>> {
    parse_srt_or_vtt(content)
}

pub fn parse_vtt(content: &str) -> anyhow::Result<Vec<SubtitleSegment>> {
    parse_srt_or_vtt(content)
}

pub fn parse_ass(content: &str) -> anyhow::Result<Vec<SubtitleSegment>> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let normalized = content.replace("\r\n", "\n").replace('\r', "\n");

    let mut lines = normalized.lines();
    let mut in_events = false;

    for line in &mut lines {
        if line.trim().eq_ignore_ascii_case("[Events]") {
            in_events = true;
            break;
        }
    }

    if !in_events {
        anyhow::bail!("missing [Events] section in ASS content");
    }

    let mut field_indices: Option<(usize, usize, usize, usize)> = None;
    let mut segments = Vec::new();

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            break;
        }

        if field_indices.is_none() && trimmed.to_ascii_lowercase().starts_with("format:") {
            let format_str = trimmed["format:".len()..].trim();
            let fields: Vec<&str> = format_str.split(',').map(|s| s.trim()).collect();
            let field_count = fields.len();

            let start_idx = fields
                .iter()
                .position(|&f| f.eq_ignore_ascii_case("Start"))
                .ok_or_else(|| anyhow!("missing Start column in ASS Format line"))?;
            let end_idx = fields
                .iter()
                .position(|&f| f.eq_ignore_ascii_case("End"))
                .ok_or_else(|| anyhow!("missing End column in ASS Format line"))?;
            let text_idx = fields
                .iter()
                .position(|&f| f.eq_ignore_ascii_case("Text"))
                .ok_or_else(|| anyhow!("missing Text column in ASS Format line"))?;

            field_indices = Some((start_idx, end_idx, text_idx, field_count));
            continue;
        }

        if let Some((start_idx, end_idx, text_idx, field_count)) = field_indices {
            let rest = if trimmed.to_ascii_lowercase().starts_with("dialogue:") {
                trimmed["dialogue:".len()..].trim_start()
            } else {
                continue;
            };

            let parts: Vec<&str> = rest.splitn(field_count, ',').collect();
            if parts.len() < field_count {
                anyhow::bail!("ASS dialogue line has fewer fields than Format specifies");
            }

            let start_str = parts[start_idx].trim();
            let end_str = parts[end_idx].trim();
            let start_ms = parse_timestamp(start_str)
                .ok_or_else(|| anyhow!("malformed start timestamp '{start_str}'"))?;
            let end_ms = parse_timestamp(end_str)
                .ok_or_else(|| anyhow!("malformed end timestamp '{end_str}'"))?;

            let raw_text = parts[text_idx];
            let no_overrides = ASS_OVERRIDE_RE.replace_all(raw_text, "");
            let text = no_overrides
                .replace(r"\N", "\n")
                .replace(r"\n", "\n")
                .replace(r"\h", " ");

            let id = segments.len() + 1;
            segments.push(SubtitleSegment::new(id, start_ms, end_ms, text));
        }
    }

    if field_indices.is_none() {
        anyhow::bail!("missing Format line in ASS [Events] section");
    }

    Ok(segments)
}

/// Dispatch on extension (case-insensitive). Unknown extension => Err.
/// Result is passed through sanitize_segments. Empty result => Err("no subtitle cues found").
pub fn parse_subtitle_file(path: &Path) -> anyhow::Result<Vec<SubtitleSegment>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read subtitle file at {}", path.display()))?;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let raw_segs = match ext.as_str() {
        "srt" => parse_srt(&content)?,
        "vtt" => parse_vtt(&content)?,
        "ass" => parse_ass(&content)?,
        _ => anyhow::bail!(
            "unsupported subtitle extension: '{}' for file {}",
            ext,
            path.display()
        ),
    };

    let sanitized = sanitize_segments(raw_segs);
    if sanitized.is_empty() {
        anyhow::bail!("no subtitle cues found");
    }

    Ok(sanitized)
}
