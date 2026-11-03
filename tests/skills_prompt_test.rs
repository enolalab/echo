use auto_subtitle::skills::model::{GlossaryEntry, Skill};
use auto_subtitle::skills::prompt::*;
use auto_subtitle::subtitle::model::SubtitleSegment as S;

fn e(s: &str, t: &str) -> GlossaryEntry {
    GlossaryEntry {
        source: s.into(),
        target: t.into(),
    }
}

fn batch(lines: &[&str]) -> Vec<S> {
    lines
        .iter()
        .enumerate()
        .map(|(i, l)| S::new(i + 1, 0, 1, *l))
        .collect()
}

fn skill(id: &str, glossary: Vec<GlossaryEntry>) -> Skill {
    Skill {
        id: id.into(),
        name: "n".into(),
        description: String::new(),
        system_prompt: "From {source_lang} to {target_lang}.".into(),
        glossary,
    }
}

#[test]
fn placeholders() {
    assert_eq!(
        substitute("{source_lang}->{target_lang} {x}", "en", "vi"),
        "en->vi {x}"
    );
    assert_eq!(
        substitute("{source_lang}", "auto", "vi"),
        "the source language"
    );
    assert_eq!(
        substitute("{source_lang}", "AUTO", "vi"),
        "the source language"
    );
}

#[test]
fn matching_rules() {
    let g = vec![
        e("ion", ""),
        e("relu", "x"),
        e("gradient descent", "hạ gradient"),
        e("đạo hàm", "derivative"),
        e("微分", "vi phân"),
        e("absent", "y"),
    ];
    let b = batch(&[
        "The function uses ReLU, then Gradient Descent",
        "đạo hàm của f",
        "これは微分です",
    ]);
    let found: Vec<&str> = glossary_for_batch(&g, &b)
        .iter()
        .map(|x| x.source.as_str())
        .collect();
    assert_eq!(found, vec!["gradient descent", "đạo hàm", "relu", "微分"]);
}

#[test]
fn ascii_boundary_at_text_edges_and_unicode_neighbours() {
    let g = vec![e("ion", "")];
    assert_eq!(glossary_for_batch(&g, &batch(&["ion"])).len(), 1);
    assert_eq!(glossary_for_batch(&g, &batch(&["(ion)"])).len(), 1);
    assert_eq!(glossary_for_batch(&g, &batch(&["éion"])).len(), 1);
    assert_eq!(glossary_for_batch(&g, &batch(&["ions"])).len(), 0);
}

#[test]
fn ordering_and_cap() {
    let g: Vec<GlossaryEntry> = (0..150).map(|i| e(&format!("t{i:03}"), "")).collect();
    let text: String = (0..150).map(|i| format!("t{i:03} ")).collect();
    let found = glossary_for_batch(&g, &batch(&[&text]));
    assert_eq!(found.len(), MAX_TERMS_PER_BATCH);
    assert_eq!(found[0].source, "t000");
    let g = vec![e("ab", ""), e("abc d", ""), e("aa", "")];
    let found: Vec<&str> = glossary_for_batch(&g, &batch(&["aa ab abc d"]))
        .iter()
        .map(|x| x.source.as_str())
        .collect();
    assert_eq!(found, vec!["abc d", "aa", "ab"]);
}

#[test]
fn render_with_and_without_glossary() {
    let s = skill(
        "u1",
        vec![e("softmax", ""), e("logit", "logit thô"), e("absent", "z")],
    );
    let out = render_system_prompt(&s, &batch(&["softmax of the logit"]), "en", "fr");
    assert_eq!(
        out,
        format!("From en to fr.\n\n{GLOSSARY_HEADER}\n- softmax → (keep)\n- logit → logit thô")
    );
    let out = render_system_prompt(&s, &batch(&["nothing here"]), "auto", "vi");
    assert_eq!(out, "From the source language to vi.");
}

#[test]
fn preset_glossary_only_for_vietnamese() {
    let p = skill("builtin:x", vec![e("softmax", "")]);
    let b = batch(&["softmax"]);
    assert!(glossary_applies(&p, "vi") && glossary_applies(&p, "vi-VN"));
    assert!(!glossary_applies(&p, "en"));
    assert!(render_system_prompt(&p, &b, "en", "vi").contains("softmax → (keep)"));
    assert_eq!(render_system_prompt(&p, &b, "en", "en"), "From en to en.");
    let u = skill("u1", vec![e("softmax", "")]);
    assert!(glossary_applies(&u, "ja"));
}

#[test]
fn header_text_is_exact() {
    assert_eq!(
        GLOSSARY_HEADER,
        "Glossary — use these translations exactly; \"(keep)\" means keep the original term unchanged:"
    );
}
