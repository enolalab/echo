use auto_subtitle::llm::{ChatClient, LlmError};
use auto_subtitle::subtitle::model::SubtitleSegment as S;
use auto_subtitle::translate::{prompt::*, LlmTranslator, Translator};
use std::sync::{Arc, Mutex};

fn batch() -> Vec<S> {
    vec![
        S::new(21, 0, 1000, "Good morning."),
        S::new(22, 1000, 2000, "How are\nyou?"),
    ]
}

#[test]
fn prompt_uses_local_numbering() {
    let p = build_translation_prompt(&batch(), "en", "vi");
    assert!(p.contains("[1] Good morning.") && p.contains("[2] How are / you?"));
    assert!(!p.contains("[21]"));
}

#[test]
fn prompt_handles_auto_source_lang() {
    let p = build_translation_prompt(&batch(), "auto", "vi");
    assert!(p.contains("Detect the source language automatically."));
}

#[test]
fn parse_ok_and_missing_line_errors() {
    let ok =
        parse_translated_lines("Here:\n[1] Chào buổi sáng.\n[2] Bạn / khỏe không?\n", 2).unwrap();
    assert_eq!(ok, vec!["Chào buổi sáng.", "Bạn\nkhỏe không?"]);
    assert!(parse_translated_lines("[1] only one", 2).is_err());
}

#[test]
fn parse_ignores_extras_and_out_of_range() {
    let raw =
        "Intro commentary\n[0] Zero\n[1] First\n[2] Second\n[3] Third extra\n[99] Ninety-nine";
    let ok = parse_translated_lines(raw, 2).unwrap();
    assert_eq!(ok, vec!["First", "Second"]);
}

type SeenCalls = Arc<Mutex<Vec<(Option<String>, String)>>>;

struct Recorder {
    seen: SeenCalls,
    reply: Result<String, LlmError>,
}

#[async_trait::async_trait]
impl ChatClient for Recorder {
    async fn chat(&self, system: Option<&str>, user: &str) -> Result<String, LlmError> {
        self.seen.lock().unwrap().push((system.map(String::from), user.to_string()));
        self.reply.clone()
    }
    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        Ok(vec![])
    }
}

#[tokio::test]
async fn llm_translator_renders_skill_per_batch() {
    use auto_subtitle::skills::{GlossaryEntry, Skill};
    let seen = Arc::new(Mutex::new(vec![]));
    let skill = Skill {
        id: "u1".into(),
        name: "Toán".into(),
        description: String::new(),
        system_prompt: "Bạn là chuyên gia Toán ({target_lang})".into(),
        glossary: vec![
            GlossaryEntry { source: "morning".into(), target: "buổi sáng".into() },
            GlossaryEntry { source: "absent".into(), target: "x".into() },
        ],
    };
    let t = LlmTranslator::new(
        Box::new(Recorder { seen: seen.clone(), reply: Ok("[1] A\n[2] B / C".into()) }),
        Some(skill),
    );
    assert_eq!(t.translate_batch(&batch(), "en", "vi").await.unwrap(), vec!["A", "B\nC"]);
    let calls = seen.lock().unwrap();
    let system = calls[0].0.as_deref().unwrap();
    assert!(system.starts_with("Bạn là chuyên gia Toán (vi)"));
    assert!(system.contains("- morning → buổi sáng"));
    assert!(!system.contains("absent"));
    assert!(calls[0].1.contains("[2] How are / you?"));
}

#[tokio::test]
async fn llm_translator_without_skill_sends_no_system() {
    let seen = Arc::new(Mutex::new(vec![]));
    let t = LlmTranslator::new(Box::new(Recorder { seen: seen.clone(), reply: Ok("[1] A\n[2] B".into()) }), None);
    t.translate_batch(&batch(), "en", "vi").await.unwrap();
    assert_eq!(seen.lock().unwrap()[0].0, None);
}

#[tokio::test]
async fn llm_translator_wraps_llm_error_for_downcast() {
    let t = LlmTranslator::new(
        Box::new(Recorder { seen: Arc::new(Mutex::new(vec![])), reply: Err(LlmError::Auth) }),
        None,
    );
    let err = t.translate_batch(&batch(), "en", "vi").await.unwrap_err();
    assert_eq!(err.downcast_ref::<LlmError>(), Some(&LlmError::Auth));
}

#[tokio::test]
async fn llm_translator_empty_input_skips_call() {
    let seen = Arc::new(Mutex::new(vec![]));
    let t = LlmTranslator::new(Box::new(Recorder { seen: seen.clone(), reply: Ok(String::new()) }), None);
    assert!(t.translate_batch(&[], "en", "vi").await.unwrap().is_empty());
    assert!(seen.lock().unwrap().is_empty());
}
