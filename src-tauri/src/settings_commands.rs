use tauri::State;

use auto_subtitle::llm::{build_chat_client, test_connection, ConnectionTest, TEST_TIMEOUT};
use auto_subtitle::settings::{GeneralSettings, ProviderProfile, SettingsStore};

use crate::state::AppState;
use crate::types::{ProfileView, SettingsView};

pub(crate) fn lock(state: &AppState) -> Result<std::sync::MutexGuard<'_, SettingsStore>, String> {
    state
        .settings
        .lock()
        .map_err(|_| "Lỗi khóa cài đặt".to_string())
}

pub(crate) fn build_view(store: &mut SettingsStore) -> SettingsView {
    let s = store.settings().clone();
    SettingsView {
        profiles: s
            .profiles
            .into_iter()
            .map(|p| ProfileView {
                has_key: store.has_profile_key(&p.id),
                profile: p,
            })
            .collect(),
        default_profile_id: s.default_profile_id,
        general: s.general,
        presets: auto_subtitle::skills::presets().to_vec(),
        skills: s.skills,
        default_skill_id: s.default_skill_id,
        stt_has_key: matches!(store.stt_key(), Ok(Some(_))),
        secrets_backend: store.backend(),
        load_warning: store.take_load_warning(),
    }
}

pub(crate) fn resolve_key(
    store: &SettingsStore,
    id: &str,
    typed: Option<String>,
) -> Option<String> {
    typed
        .filter(|k| !k.trim().is_empty())
        .or_else(|| store.profile_key(id).ok().flatten())
}

fn err(e: anyhow::Error) -> String {
    format!("{e:#}")
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<SettingsView, String> {
    Ok(build_view(&mut *lock(&state)?))
}

#[tauri::command]
pub fn save_profile(
    state: State<'_, AppState>,
    profile: ProviderProfile,
    api_key: Option<String>,
) -> Result<SettingsView, String> {
    let mut store = lock(&state)?;
    store.upsert_profile(profile, api_key).map_err(err)?;
    Ok(build_view(&mut store))
}

#[tauri::command]
pub fn delete_profile(state: State<'_, AppState>, id: String) -> Result<SettingsView, String> {
    let mut store = lock(&state)?;
    store.delete_profile(&id).map_err(err)?;
    Ok(build_view(&mut store))
}

#[tauri::command]
pub fn set_default_profile(state: State<'_, AppState>, id: String) -> Result<SettingsView, String> {
    let mut store = lock(&state)?;
    store.set_default_profile(&id).map_err(err)?;
    Ok(build_view(&mut store))
}

#[tauri::command]
pub fn save_general(
    state: State<'_, AppState>,
    general: GeneralSettings,
) -> Result<SettingsView, String> {
    let mut store = lock(&state)?;
    store.set_general(general).map_err(err)?;
    Ok(build_view(&mut store))
}

#[tauri::command]
pub fn set_stt_key(state: State<'_, AppState>, key: String) -> Result<SettingsView, String> {
    let mut store = lock(&state)?;
    store.set_stt_key(&key).map_err(err)?;
    Ok(build_view(&mut store))
}

#[tauri::command]
pub async fn test_profile(
    state: State<'_, AppState>,
    profile: ProviderProfile,
    api_key: Option<String>,
) -> Result<ConnectionTest, String> {
    let key = resolve_key(&*lock(&state)?, &profile.id, api_key);
    let client = match build_chat_client(&profile, key) {
        Ok(c) => c,
        Err(e) => {
            return Ok(ConnectionTest {
                ok: false,
                latency_ms: 0,
                error: Some(e.user_message()),
            })
        }
    };
    Ok(test_connection(client.as_ref(), TEST_TIMEOUT).await)
}

#[tauri::command]
pub async fn list_profile_models(
    state: State<'_, AppState>,
    profile: ProviderProfile,
    api_key: Option<String>,
) -> Result<Vec<String>, String> {
    let key = resolve_key(&*lock(&state)?, &profile.id, api_key);
    let client = build_chat_client(&profile, key).map_err(|e| e.user_message())?;
    client.list_models().await.map_err(|e| e.user_message())
}

#[cfg(test)]
mod tests {
    use super::*;
    use auto_subtitle::settings::secrets::{MemoryStore, SecretsBackend};
    use auto_subtitle::settings::{ProviderKind, ProviderProfile, SettingsStore};

    #[test]
    fn view_exposes_has_key_but_never_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SettingsStore::open(
            dir.path(),
            Box::new(MemoryStore::default()),
            SecretsBackend::File,
        )
        .unwrap();
        let p = store
            .upsert_profile(
                ProviderProfile {
                    id: String::new(),
                    name: "DS".into(),
                    kind: ProviderKind::DeepSeek,
                    base_url: None,
                    model: "deepseek-chat".into(),
                },
                Some("sk-secret".into()),
            )
            .unwrap();
        let v = build_view(&mut store);
        let json = serde_json::to_string(&v).unwrap();
        assert!(!json.contains("sk-secret"));
        let pv = v.profiles.iter().find(|x| x.profile.id == p.id).unwrap();
        assert!(pv.has_key);
        assert!(json.contains("\"has_key\":true"));
        assert!(json.contains("\"secrets_backend\":\"file\""));
        assert!(json.contains("\"name\":\"DS\""));
        assert!(!v.stt_has_key);
    }

    #[test]
    fn resolve_key_prefers_form_value_then_stored() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SettingsStore::open(
            dir.path(),
            Box::new(MemoryStore::default()),
            SecretsBackend::Keyring,
        )
        .unwrap();
        let p = store
            .upsert_profile(
                ProviderProfile {
                    id: String::new(),
                    name: "DS".into(),
                    kind: ProviderKind::DeepSeek,
                    base_url: None,
                    model: "m".into(),
                },
                Some("stored".into()),
            )
            .unwrap();
        assert_eq!(
            resolve_key(&store, &p.id, Some("typed".into())).as_deref(),
            Some("typed")
        );
        assert_eq!(
            resolve_key(&store, &p.id, Some("  ".into())).as_deref(),
            Some("stored")
        );
        assert_eq!(resolve_key(&store, &p.id, None).as_deref(), Some("stored"));
        assert_eq!(resolve_key(&store, "unknown", None), None);
    }

    #[test]
    fn view_includes_presets_and_user_skills() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SettingsStore::open(
            dir.path(),
            Box::new(MemoryStore::default()),
            SecretsBackend::Keyring,
        )
        .unwrap();
        store.duplicate_skill("builtin:math").unwrap();
        store.set_default_skill(Some("builtin:math")).unwrap();
        let v = build_view(&mut store);
        assert_eq!(v.presets.len(), 6);
        assert_eq!(v.skills.len(), 1);
        assert_eq!(v.default_skill_id.as_deref(), Some("builtin:math"));
    }
}
