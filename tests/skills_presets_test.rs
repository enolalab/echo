use auto_subtitle::skills::presets;

#[test]
fn six_valid_presets() {
    let all = presets();
    let ids: Vec<&str> = all.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "builtin:math",
            "builtin:ai-ml",
            "builtin:programming",
            "builtin:physics",
            "builtin:chemistry",
            "builtin:biology"
        ]
    );
    let names: Vec<&str> = all.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["Toán học", "AI / Machine Learning", "Lập trình / CS", "Vật lý", "Hóa học", "Sinh học"]
    );
    for s in all {
        assert!(s.is_builtin());
        assert!(s.system_prompt.contains("{source_lang}") && s.system_prompt.contains("{target_lang}"));
        assert!(!s.system_prompt.contains("{domain}"), "{} still has {{domain}}", s.id);
        assert!((20..=45).contains(&s.glossary.len()), "{} has {} terms", s.id, s.glossary.len());
        s.validate(all).unwrap_or_else(|e| panic!("{}: {e}", s.id));
        assert_eq!(s.clone().normalized(), *s, "{} not normalized", s.id);
    }
}
