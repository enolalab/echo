use tauri::State;

use auto_subtitle::llm::build_chat_client;
use auto_subtitle::skills::io::MAX_IMPORT_BYTES;
use auto_subtitle::skills::try_run::{try_skill as run_try_skill, TrySkillResult};
use auto_subtitle::skills::Skill;

use crate::settings_commands::{build_view, lock};
use crate::state::AppState;
use crate::types::SettingsView;

pub(crate) fn skill_log_line(skill: Option<&Skill>) -> String {
    match skill {
        Some(s) => format!("Skill: {}", s.name),
        None => "Skill: không dùng".to_string(),
    }
}

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[tauri::command]
pub fn save_skill(state: State<'_, AppState>, skill: Skill) -> Result<SettingsView, String> {
    let mut store = lock(&state)?;
    store.upsert_skill(skill).map_err(err)?;
    Ok(build_view(&mut store))
}

#[tauri::command]
pub fn delete_skill(state: State<'_, AppState>, id: String) -> Result<SettingsView, String> {
    let mut store = lock(&state)?;
    store.delete_skill(&id).map_err(err)?;
    Ok(build_view(&mut store))
}

#[tauri::command]
pub fn duplicate_skill(state: State<'_, AppState>, id: String) -> Result<SettingsView, String> {
    let mut store = lock(&state)?;
    store.duplicate_skill(&id).map_err(err)?;
    Ok(build_view(&mut store))
}

#[tauri::command]
pub fn set_default_skill(state: State<'_, AppState>, id: Option<String>) -> Result<SettingsView, String> {
    let mut store = lock(&state)?;
    store.set_default_skill(id.as_deref()).map_err(err)?;
    Ok(build_view(&mut store))
}

#[tauri::command]
pub async fn import_skill(state: State<'_, AppState>) -> Result<SettingsView, String> {
    let file = rfd::AsyncFileDialog::new()
        .add_filter("Echo skill", &["json"])
        .set_title("Nhập skill")
        .pick_file()
        .await;
    let Some(file) = file else {
        return Ok(build_view(&mut *lock(&state)?));
    };
    let path = file.path().to_path_buf();
    let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
    if size as usize > MAX_IMPORT_BYTES {
        return Err("File skill quá lớn (tối đa 1 MB)".to_string());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("Không đọc được file: {e}"))?;
    let mut store = lock(&state)?;
    store.import_skill_json(&text).map_err(err)?;
    Ok(build_view(&mut store))
}

#[tauri::command]
pub async fn export_skill(state: State<'_, AppState>, id: String) -> Result<Option<String>, String> {
    let (file_name, json) = lock(&state)?.export_skill_json(&id).map_err(err)?;
    let file = rfd::AsyncFileDialog::new()
        .set_file_name(&file_name)
        .add_filter("Echo skill", &["json"])
        .set_title("Xuất skill")
        .save_file()
        .await;
    let Some(file) = file else { return Ok(None) };
    std::fs::write(file.path(), json).map_err(|e| format!("Không ghi được file: {e}"))?;
    Ok(Some(file.path().to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn try_skill(
    state: State<'_, AppState>,
    skill: Skill,
    profile_id: Option<String>,
    sample: String,
    source_lang: String,
    target_lang: String,
) -> Result<TrySkillResult, String> {
    let fail = |msg: String| TrySkillResult { lines: vec![], system_prompt: String::new(), error: Some(msg) };
    let (client, others) = {
        let store = lock(&state)?;
        let profile = match store.resolve_profile(profile_id.as_deref()) {
            Ok(Some(p)) => p,
            Ok(None) => return Ok(fail("Chưa cấu hình provider dịch".into())),
            Err(e) => return Ok(fail(format!("{e:#}"))),
        };
        let key = store.profile_key(&profile.id).map_err(err)?;
        let client = match build_chat_client(&profile, key) {
            Ok(c) => c,
            Err(e) => return Ok(fail(e.user_message())),
        };
        (client, store.other_skills(&skill.id))
    };
    Ok(run_try_skill(client, skill, &others, &sample, &source_lang, &target_lang).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use auto_subtitle::settings::secrets::{MemoryStore, SecretsBackend};
    use auto_subtitle::settings::SettingsStore;

    #[test]
    fn skill_label_for_logs() {
        let dir = tempfile::tempdir().unwrap();
        let store =
            SettingsStore::open(dir.path(), Box::new(MemoryStore::default()), SecretsBackend::Keyring).unwrap();
        let s = store.resolve_skill(Some("builtin:math")).unwrap();
        assert_eq!(skill_log_line(s.as_ref()), "Skill: Toán học");
        assert_eq!(skill_log_line(None), "Skill: không dùng");
    }

    #[test]
    fn pipeline_config_skill_id_defaults_to_none() {
        let cfg: crate::types::PipelineConfig = serde_json::from_str(
            r#"{"video_path":"v","subtitle_path":null,"whisper_model":"base","use_cloud_stt":false,"target_lang":"vi","skip_translate":false}"#,
        )
        .unwrap();
        assert_eq!(cfg.skill_id, None);
    }
}
