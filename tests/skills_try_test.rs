use auto_subtitle::llm::{ChatClient, LlmError};
use auto_subtitle::skills::try_run::*;
use auto_subtitle::skills::{GlossaryEntry, Skill};

struct Fixed(Result<String, LlmError>);
#[async_trait::async_trait]
impl ChatClient for Fixed {
    async fn chat(&self, _: Option<&str>, _: &str) -> Result<String, LlmError> {
        self.0.clone()
    }
    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        Ok(vec![])
    }
}

fn skill() -> Skill {
    Skill {
        id: String::new(),
        name: "T".into(),
        description: String::new(),
        system_prompt: "Sys {target_lang}".into(),
        glossary: vec![GlossaryEntry {
            source: "hello".into(),
            target: "chào".into(),
        }],
    }
}

#[tokio::test]
async fn translates_sample_and_returns_prompt() {
    let r = try_skill(
        Box::new(Fixed(Ok("[1] Chào\n[2] Tạm biệt".into()))),
        skill(),
        &[],
        "Hello\n\n  Bye  \n",
        "auto",
        "vi",
    )
    .await;
    assert_eq!(r.error, None);
    assert_eq!(r.lines, vec!["Chào", "Tạm biệt"]);
    assert!(r.system_prompt.starts_with("Sys vi") && r.system_prompt.contains("- hello → chào"));
}

#[tokio::test]
async fn sample_limits() {
    let r = try_skill(
        Box::new(Fixed(Ok(String::new()))),
        skill(),
        &[],
        "  \n ",
        "auto",
        "vi",
    )
    .await;
    assert_eq!(r.error.as_deref(), Some("Nhập ít nhất 1 dòng"));
    let many = (0..21)
        .map(|i| format!("l{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let r = try_skill(
        Box::new(Fixed(Ok(String::new()))),
        skill(),
        &[],
        &many,
        "auto",
        "vi",
    )
    .await;
    assert_eq!(r.error.as_deref(), Some("Tối đa 20 dòng"));
    assert_eq!(MAX_TRY_LINES, 20);
}

#[tokio::test]
async fn invalid_skill_reports_validation_error() {
    let mut s = skill();
    s.name = " ".into();
    let r = try_skill(
        Box::new(Fixed(Ok(String::new()))),
        s,
        &[],
        "Hello",
        "auto",
        "vi",
    )
    .await;
    assert_eq!(r.error.as_deref(), Some("Tên skill không được để trống"));
    assert!(r.lines.is_empty() && r.system_prompt.is_empty());
}

#[tokio::test]
async fn provider_error_keeps_prompt() {
    let r = try_skill(
        Box::new(Fixed(Err(LlmError::Auth))),
        skill(),
        &[],
        "Hello",
        "auto",
        "vi",
    )
    .await;
    assert_eq!(r.error.as_deref(), Some("API key không hợp lệ"));
    assert!(r.lines.is_empty());
    assert!(r.system_prompt.contains("hello → chào"));
}

#[tokio::test]
async fn parse_error_is_reported() {
    let r = try_skill(
        Box::new(Fixed(Ok("garbage".into()))),
        skill(),
        &[],
        "Hello",
        "auto",
        "vi",
    )
    .await;
    assert!(r.error.unwrap().contains("Missing translated line [1]"));
}
