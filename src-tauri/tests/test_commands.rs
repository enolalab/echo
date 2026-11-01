#[tokio::test]
async fn test_system_info_detection() {
    let info = auto_subtitle_desktop::commands::get_system_info().await;
    assert!(info.is_ok());
    let info = info.unwrap();
    assert!(info.whisper_models.len() >= 4);
    assert!(info.whisper_models.iter().any(|m| m.name == "base"));
}
