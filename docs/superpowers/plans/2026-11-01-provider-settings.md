# Provider Profiles & Persistent Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Multiple LLM provider profiles (OpenAI, OpenAI-compatible, Anthropic, Anthropic-compatible, DeepSeek, Ollama) with keychain-stored API keys, persistent general settings, shared by the desktop app and the CLI.

**Architecture:** A `settings` module in the core `auto-subtitle` crate owns `settings.json` + a `SecretStore` (OS keychain, file fallback). A new `llm` module provides one `ChatClient` per wire protocol; a single `LlmTranslator` replaces the two existing translators. Tauri exposes thin commands over `SettingsStore`; the React UI adds a `SettingsModal` and a profile select in the header.

**Tech Stack:** Rust 2021 (tokio, reqwest 0.12, serde, clap 4, keyring 3.6, uuid 1), wiremock 0.6 for HTTP tests; Tauri 2; React 18 + TypeScript + Vitest + Testing Library.

**Spec:** `docs/superpowers/specs/2026-11-01-provider-settings-design.md`

## Global Constraints

- Config dir: `dirs::config_dir()/echo/`; files `settings.json`, `settings.json.bak`, `secrets.json`.
- Settings `version` = `1`.
- Keychain service `com.echo.subtitles`; accounts `profile:<id>` and `stt:openai`.
- `keyring = "3.6"` with features `apple-native`, `windows-native`, `sync-secret-service`, `crypto-rust` (do NOT use 4.x).
- `ProviderKind` JSON values: `openai`, `openai_compatible`, `anthropic`, `anthropic_compatible`, `deepseek`, `ollama`.
- Default base URLs: OpenAi `https://api.openai.com/v1`, DeepSeek `https://api.deepseek.com/v1`, Anthropic `https://api.anthropic.com`, Ollama `http://localhost:11434`; `*_compatible` have none.
- Anthropic headers: `x-api-key`, `anthropic-version: 2023-06-01`; `max_tokens: 8192`.
- First-run default profile: name `Ollama local`, kind `ollama`, model `qwen2.5:7b`.
- API keys never written to `settings.json` and never sent over IPC during a pipeline run.
- Connection test prompt `Reply with: OK`, timeout 20s.
- Retry: existing `MAX_ATTEMPTS = 3`; non-retryable `LlmError` stops immediately; `RateLimit`/5xx sleep `2s × attempt`.
- User-facing strings are Vietnamese, exactly as written in this plan.
- No comments added to existing code beyond what this plan shows.

## Prerequisite (user, before Task 1)

The working tree has uncommitted user changes in `src-tauri/src/{commands,main,state,types}.rs`, `ui/src/App.tsx`, `ui/src/components/*`, `ui/src/lib/ipc.ts`, `ui/src/types/index.ts`. Ask the user to commit them first. Tasks 9–12 edit those files using **text anchors**, not line numbers; re-read each file before editing.

## File Structure

| File | Responsibility |
|---|---|
| `src/settings/mod.rs` | Re-exports |
| `src/settings/model.rs` | `Settings`, `ProviderProfile`, `ProviderKind`, `GeneralSettings`, defaults, validation, lookup |
| `src/settings/secrets.rs` | `SecretStore` trait, `KeyringStore`, `FileStore`, `MemoryStore`, backend selection |
| `src/settings/store.rs` | `SettingsStore`: load/atomic save/corrupt recovery, CRUD, key access, profile resolution |
| `src/llm/mod.rs` | `ChatClient` trait, `build_chat_client`, `test_connection`, shared HTTP helper |
| `src/llm/error.rs` | `LlmError` + mapping + Vietnamese messages |
| `src/llm/openai.rs` | OpenAI-protocol client (OpenAI, compatible, DeepSeek) |
| `src/llm/anthropic.rs` | Anthropic-protocol client |
| `src/llm/ollama.rs` | Ollama client + model-presence check |
| `src/translate/llm_translator.rs` | `LlmTranslator` |
| `src/cli.rs` | New flags + `choose_translator` |
| `src/main.rs` | CLI wiring |
| `src-tauri/src/settings_commands.rs` | Settings Tauri commands |
| `ui/src/lib/providers.ts` | Kind labels/defaults for UI |
| `ui/src/components/SettingsModal.tsx` | Settings UI |

---

### Task 1: Settings data model

**Files:**
- Modify: `Cargo.toml` (dependencies + dev-dependencies)
- Create: `src/settings/mod.rs`, `src/settings/model.rs`
- Modify: `src/lib.rs`
- Test: `tests/settings_model_test.rs`

**Interfaces:**
- Produces: `ProviderKind` (`Copy`, `as_str() -> &'static str`, `default_base_url() -> Option<&'static str>`, `requires_base_url() -> bool`, `requires_api_key() -> bool`); `ProviderProfile { id, name, kind, base_url: Option<String>, model }` with `effective_base_url() -> Option<String>`; `GeneralSettings` (`Default`); `Settings { version, profiles, default_profile_id, general }` with `with_defaults()`, `validate_profile(&ProviderProfile) -> Result<(), String>`, `find_profile(&str) -> Option<&ProviderProfile>`, `default_profile() -> Option<&ProviderProfile>`; const `SETTINGS_VERSION: u32 = 1`.

- [ ] **Step 1: Add dependencies**

```bash
cargo add keyring@3.6 --features apple-native,windows-native,sync-secret-service,crypto-rust
cargo add uuid@1 --features v4
cargo add --dev tokio@1 --features test-util,macros,rt-multi-thread
```

Expected: `Cargo.toml` `[dependencies]` gains `keyring` and `uuid`; `[dev-dependencies]` gains `tokio`.

- [ ] **Step 2: Write the failing test** — `tests/settings_model_test.rs`

```rust
use auto_subtitle::settings::model::*;

fn profile(id: &str, name: &str, kind: ProviderKind, base: Option<&str>) -> ProviderProfile {
    ProviderProfile {
        id: id.into(),
        name: name.into(),
        kind,
        base_url: base.map(String::from),
        model: "m".into(),
    }
}

#[test]
fn kind_serializes_to_spec_values() {
    let pairs = [
        (ProviderKind::OpenAi, "\"openai\""),
        (ProviderKind::OpenAiCompatible, "\"openai_compatible\""),
        (ProviderKind::Anthropic, "\"anthropic\""),
        (ProviderKind::AnthropicCompatible, "\"anthropic_compatible\""),
        (ProviderKind::DeepSeek, "\"deepseek\""),
        (ProviderKind::Ollama, "\"ollama\""),
    ];
    for (k, json) in pairs {
        assert_eq!(serde_json::to_string(&k).unwrap(), json);
        assert_eq!(serde_json::from_str::<ProviderKind>(json).unwrap(), k);
    }
}

#[test]
fn defaults_have_one_ollama_profile_as_default() {
    let s = Settings::with_defaults();
    assert_eq!(s.version, SETTINGS_VERSION);
    assert_eq!(s.profiles.len(), 1);
    let p = &s.profiles[0];
    assert_eq!(p.name, "Ollama local");
    assert_eq!(p.kind, ProviderKind::Ollama);
    assert_eq!(p.model, "qwen2.5:7b");
    assert_eq!(s.default_profile_id.as_deref(), Some(p.id.as_str()));
    assert_eq!(s.general, GeneralSettings::default());
    assert_eq!(s.general.whisper_model, "base");
    assert_eq!(s.general.target_lang, "vi");
    assert!(s.general.bilingual);
    assert_eq!(s.general.style_preset, "youtube_yellow");
    assert!(!s.general.use_cloud_stt);
}

#[test]
fn effective_base_url_uses_kind_default_and_trims() {
    assert_eq!(
        profile("1", "a", ProviderKind::DeepSeek, None).effective_base_url().as_deref(),
        Some("https://api.deepseek.com/v1")
    );
    assert_eq!(
        profile("1", "a", ProviderKind::OpenAi, Some(" https://x.io/v1/ ")).effective_base_url().as_deref(),
        Some("https://x.io/v1")
    );
    assert_eq!(profile("1", "a", ProviderKind::OpenAiCompatible, Some("  ")).effective_base_url(), None);
}

#[test]
fn validation_rules() {
    let mut s = Settings::with_defaults();
    s.profiles.push(profile("2", "DeepSeek", ProviderKind::DeepSeek, None));

    assert!(s.validate_profile(&profile("3", "Mới", ProviderKind::OpenAi, None)).is_ok());
    assert_eq!(
        s.validate_profile(&profile("3", "  ", ProviderKind::OpenAi, None)).unwrap_err(),
        "Tên profile không được để trống"
    );
    assert_eq!(
        s.validate_profile(&profile("3", "deepseek", ProviderKind::OpenAi, None)).unwrap_err(),
        "Tên profile đã tồn tại"
    );
    // same id may keep its own name
    assert!(s.validate_profile(&profile("2", "DeepSeek", ProviderKind::DeepSeek, None)).is_ok());
    assert_eq!(
        s.validate_profile(&profile("3", "X", ProviderKind::AnthropicCompatible, None)).unwrap_err(),
        "Base URL bắt buộc với provider tương thích"
    );
    let mut no_model = profile("3", "Y", ProviderKind::OpenAi, None);
    no_model.model = " ".into();
    assert_eq!(s.validate_profile(&no_model).unwrap_err(), "Model không được để trống");
}

#[test]
fn find_profile_by_id_or_case_insensitive_name() {
    let mut s = Settings::with_defaults();
    s.profiles.push(profile("abc", "DeepSeek Cá Nhân", ProviderKind::DeepSeek, None));
    assert_eq!(s.find_profile("abc").unwrap().name, "DeepSeek Cá Nhân");
    assert_eq!(s.find_profile("deepseek cá nhân").unwrap().id, "abc");
    assert!(s.find_profile("nope").is_none());
    assert_eq!(s.default_profile().unwrap().name, "Ollama local");
}

#[test]
fn unknown_fields_and_missing_sections_tolerated() {
    let s: Settings = serde_json::from_str(r#"{"version":1,"future":true}"#).unwrap();
    assert!(s.profiles.is_empty());
    assert_eq!(s.general, GeneralSettings::default());
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --test settings_model_test`
Expected: compile error `unresolved import auto_subtitle::settings`.

- [ ] **Step 4: Implement** — `src/settings/mod.rs` (Tasks 2 and 3 extend it):

```rust
pub mod model;

pub use model::{GeneralSettings, ProviderKind, ProviderProfile, Settings, SETTINGS_VERSION};
```

`src/settings/model.rs`:

```rust
use serde::{Deserialize, Serialize};

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderKind {
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "openai_compatible")]
    OpenAiCompatible,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "anthropic_compatible")]
    AnthropicCompatible,
    #[serde(rename = "deepseek")]
    DeepSeek,
    #[serde(rename = "ollama")]
    Ollama,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::OpenAiCompatible => "openai_compatible",
            Self::Anthropic => "anthropic",
            Self::AnthropicCompatible => "anthropic_compatible",
            Self::DeepSeek => "deepseek",
            Self::Ollama => "ollama",
        }
    }

    pub fn default_base_url(self) -> Option<&'static str> {
        match self {
            Self::OpenAi => Some("https://api.openai.com/v1"),
            Self::DeepSeek => Some("https://api.deepseek.com/v1"),
            Self::Anthropic => Some("https://api.anthropic.com"),
            Self::Ollama => Some("http://localhost:11434"),
            Self::OpenAiCompatible | Self::AnthropicCompatible => None,
        }
    }

    pub fn requires_base_url(self) -> bool {
        self.default_base_url().is_none()
    }

    pub fn requires_api_key(self) -> bool {
        !matches!(self, Self::Ollama)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderProfile {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    #[serde(default)]
    pub base_url: Option<String>,
    pub model: String,
}

impl ProviderProfile {
    pub fn effective_base_url(&self) -> Option<String> {
        self.base_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.trim_end_matches('/').to_string())
            .or_else(|| self.kind.default_base_url().map(String::from))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct GeneralSettings {
    pub whisper_model: String,
    pub target_lang: String,
    pub bilingual: bool,
    pub style_preset: String,
    pub use_cloud_stt: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            whisper_model: "base".into(),
            target_lang: "vi".into(),
            bilingual: true,
            style_preset: "youtube_yellow".into(),
            use_cloud_stt: false,
        }
    }
}

fn default_version() -> u32 {
    SETTINGS_VERSION
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub profiles: Vec<ProviderProfile>,
    #[serde(default)]
    pub default_profile_id: Option<String>,
    #[serde(default)]
    pub general: GeneralSettings,
}

impl Settings {
    pub fn with_defaults() -> Self {
        let ollama = ProviderProfile {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Ollama local".into(),
            kind: ProviderKind::Ollama,
            base_url: None,
            model: "qwen2.5:7b".into(),
        };
        Self {
            version: SETTINGS_VERSION,
            default_profile_id: Some(ollama.id.clone()),
            profiles: vec![ollama],
            general: GeneralSettings::default(),
        }
    }

    pub fn validate_profile(&self, p: &ProviderProfile) -> Result<(), String> {
        let name = p.name.trim();
        if name.is_empty() {
            return Err("Tên profile không được để trống".into());
        }
        if self
            .profiles
            .iter()
            .any(|o| o.id != p.id && o.name.trim().to_lowercase() == name.to_lowercase())
        {
            return Err("Tên profile đã tồn tại".into());
        }
        if p.kind.requires_base_url() && p.effective_base_url().is_none() {
            return Err("Base URL bắt buộc với provider tương thích".into());
        }
        if p.model.trim().is_empty() {
            return Err("Model không được để trống".into());
        }
        Ok(())
    }

    pub fn find_profile(&self, name_or_id: &str) -> Option<&ProviderProfile> {
        let q = name_or_id.trim();
        self.profiles.iter().find(|p| p.id == q).or_else(|| {
            let lower = q.to_lowercase();
            self.profiles.iter().find(|p| p.name.trim().to_lowercase() == lower)
        })
    }

    pub fn default_profile(&self) -> Option<&ProviderProfile> {
        let id = self.default_profile_id.as_deref()?;
        self.profiles.iter().find(|p| p.id == id)
    }
}
```

`src/lib.rs` — add `pub mod settings;` after `pub mod pipeline;`.

- [ ] **Step 5: Run tests**

Run: `cargo test --test settings_model_test`
Expected: 6 passed.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/lib.rs src/settings tests/settings_model_test.rs
git commit -m "feat(settings): add provider profile and general settings model"
```

---

### Task 2: Secret stores

**Files:**
- Create: `src/settings/secrets.rs`
- Modify: `src/settings/mod.rs`
- Test: `tests/settings_secrets_test.rs`

**Interfaces:**
- Produces: `trait SecretStore: Send + Sync { fn get(&self, account: &str) -> anyhow::Result<Option<String>>; fn set(&self, account: &str, value: &str) -> anyhow::Result<()>; fn delete(&self, account: &str) -> anyhow::Result<()>; }`; `enum SecretsBackend { Keyring, File }` (serde `"keyring"`/`"file"`); `KeyringStore`; `FileStore::new(path: PathBuf)`; `MemoryStore` (`Default`); `profile_account(id: &str) -> String`; consts `SERVICE`, `STT_ACCOUNT`; `select_secret_store(primary: Box<dyn SecretStore>, fallback_path: &Path) -> (Box<dyn SecretStore>, SecretsBackend)`; `open_secret_store(dir: &Path) -> (Box<dyn SecretStore>, SecretsBackend)`.

- [ ] **Step 1: Write the failing test** — `tests/settings_secrets_test.rs`

```rust
use auto_subtitle::settings::secrets::*;

struct Broken;
impl SecretStore for Broken {
    fn get(&self, _: &str) -> anyhow::Result<Option<String>> {
        anyhow::bail!("no keychain")
    }
    fn set(&self, _: &str, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("no keychain")
    }
    fn delete(&self, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("no keychain")
    }
}

#[test]
fn accounts() {
    assert_eq!(profile_account("abc"), "profile:abc");
    assert_eq!(STT_ACCOUNT, "stt:openai");
    assert_eq!(SERVICE, "com.echo.subtitles");
}

#[test]
fn file_store_roundtrip_and_delete() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secrets.json");
    let s = FileStore::new(path.clone());
    assert_eq!(s.get("a").unwrap(), None);
    s.set("a", "1").unwrap();
    s.set("b", "2").unwrap();
    assert_eq!(s.get("a").unwrap().as_deref(), Some("1"));
    s.delete("a").unwrap();
    s.delete("missing").unwrap();
    assert_eq!(s.get("a").unwrap(), None);
    // persisted across instances
    assert_eq!(FileStore::new(path).get("b").unwrap().as_deref(), Some("2"));
}

#[cfg(unix)]
#[test]
fn file_store_is_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("secrets.json");
    FileStore::new(path.clone()).set("a", "1").unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn memory_store_roundtrip() {
    let s = MemoryStore::default();
    s.set("x", "y").unwrap();
    assert_eq!(s.get("x").unwrap().as_deref(), Some("y"));
    s.delete("x").unwrap();
    assert_eq!(s.get("x").unwrap(), None);
}

#[test]
fn select_falls_back_to_file_when_primary_broken() {
    let dir = tempfile::tempdir().unwrap();
    let (store, backend) = select_secret_store(Box::new(Broken), &dir.path().join("secrets.json"));
    assert_eq!(backend, SecretsBackend::File);
    store.set("a", "1").unwrap();
    assert!(dir.path().join("secrets.json").exists());
}

#[test]
fn select_keeps_working_primary_and_cleans_probe() {
    let dir = tempfile::tempdir().unwrap();
    let (store, backend) =
        select_secret_store(Box::new(MemoryStore::default()), &dir.path().join("secrets.json"));
    assert_eq!(backend, SecretsBackend::Keyring);
    assert_eq!(store.get("probe:echo").unwrap(), None);
    assert!(!dir.path().join("secrets.json").exists());
}

#[test]
fn backend_serializes_lowercase() {
    assert_eq!(serde_json::to_string(&SecretsBackend::File).unwrap(), "\"file\"");
    assert_eq!(serde_json::to_string(&SecretsBackend::Keyring).unwrap(), "\"keyring\"");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test settings_secrets_test`
Expected: compile error `could not find secrets in settings`.

- [ ] **Step 3: Implement** — `src/settings/secrets.rs`

```rust
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Context;
use serde::{Deserialize, Serialize};

pub const SERVICE: &str = "com.echo.subtitles";
pub const STT_ACCOUNT: &str = "stt:openai";
const PROBE_ACCOUNT: &str = "probe:echo";

pub fn profile_account(id: &str) -> String {
    format!("profile:{id}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretsBackend {
    Keyring,
    File,
}

pub trait SecretStore: Send + Sync {
    fn get(&self, account: &str) -> anyhow::Result<Option<String>>;
    fn set(&self, account: &str, value: &str) -> anyhow::Result<()>;
    fn delete(&self, account: &str) -> anyhow::Result<()>;
}

pub struct KeyringStore;

impl SecretStore for KeyringStore {
    fn get(&self, account: &str) -> anyhow::Result<Option<String>> {
        let entry = keyring::Entry::new(SERVICE, account)?;
        match entry.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    fn set(&self, account: &str, value: &str) -> anyhow::Result<()> {
        keyring::Entry::new(SERVICE, account)?.set_password(value)?;
        Ok(())
    }

    fn delete(&self, account: &str) -> anyhow::Result<()> {
        match keyring::Entry::new(SERVICE, account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

pub struct FileStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl FileStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path, lock: Mutex::new(()) }
    }

    fn read_map(&self) -> anyhow::Result<BTreeMap<String, String>> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text)
                .with_context(|| format!("invalid secrets file {}", self.path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(e).with_context(|| format!("cannot read {}", self.path.display())),
        }
    }

    fn write_map(&self, map: &BTreeMap<String, String>) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(map)?)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

impl SecretStore for FileStore {
    fn get(&self, account: &str) -> anyhow::Result<Option<String>> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        Ok(self.read_map()?.get(account).cloned())
    }

    fn set(&self, account: &str, value: &str) -> anyhow::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut map = self.read_map()?;
        map.insert(account.to_string(), value.to_string());
        self.write_map(&map)
    }

    fn delete(&self, account: &str) -> anyhow::Result<()> {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut map = self.read_map()?;
        if map.remove(account).is_some() {
            self.write_map(&map)?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct MemoryStore(Mutex<BTreeMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, account: &str) -> anyhow::Result<Option<String>> {
        Ok(self.0.lock().unwrap_or_else(|e| e.into_inner()).get(account).cloned())
    }
    fn set(&self, account: &str, value: &str) -> anyhow::Result<()> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(account.to_string(), value.to_string());
        Ok(())
    }
    fn delete(&self, account: &str) -> anyhow::Result<()> {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).remove(account);
        Ok(())
    }
}

fn probe(store: &dyn SecretStore) -> bool {
    store.set(PROBE_ACCOUNT, "ok").is_ok()
        && matches!(store.get(PROBE_ACCOUNT), Ok(Some(v)) if v == "ok")
        && store.delete(PROBE_ACCOUNT).is_ok()
}

pub fn select_secret_store(
    primary: Box<dyn SecretStore>,
    fallback_path: &Path,
) -> (Box<dyn SecretStore>, SecretsBackend) {
    if probe(primary.as_ref()) {
        (primary, SecretsBackend::Keyring)
    } else {
        (Box::new(FileStore::new(fallback_path.to_path_buf())), SecretsBackend::File)
    }
}

pub fn open_secret_store(dir: &Path) -> (Box<dyn SecretStore>, SecretsBackend) {
    select_secret_store(Box::new(KeyringStore), &dir.join("secrets.json"))
}
```

`src/settings/mod.rs`:

```rust
pub mod model;
pub mod secrets;

pub use model::{GeneralSettings, ProviderKind, ProviderProfile, Settings, SETTINGS_VERSION};
pub use secrets::{SecretStore, SecretsBackend};
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test settings_secrets_test`
Expected: 7 passed (6 on Windows).

- [ ] **Step 5: Commit**

```bash
git add src/settings tests/settings_secrets_test.rs
git commit -m "feat(settings): add keychain secret store with file fallback"
```

---

### Task 3: SettingsStore

**Files:**
- Create: `src/settings/store.rs`
- Modify: `src/settings/mod.rs`
- Test: `tests/settings_store_test.rs`

**Interfaces:**
- Consumes: Task 1 model, Task 2 secrets.
- Produces: `SettingsStore` with:
  - `open(dir: &Path, secrets: Box<dyn SecretStore>, backend: SecretsBackend) -> anyhow::Result<Self>`
  - `open_default() -> anyhow::Result<Self>`; free fn `default_config_dir() -> anyhow::Result<PathBuf>`
  - `settings(&self) -> &Settings`, `backend(&self) -> SecretsBackend`, `take_load_warning(&mut self) -> Option<String>`
  - `upsert_profile(&mut self, profile: ProviderProfile, api_key: Option<String>) -> anyhow::Result<ProviderProfile>` (empty `id` ⇒ new uuid)
  - `delete_profile(&mut self, id: &str) -> anyhow::Result<()>`
  - `set_default_profile(&mut self, id: &str) -> anyhow::Result<()>`
  - `set_general(&mut self, general: GeneralSettings) -> anyhow::Result<()>`
  - `profile_key(&self, id: &str) -> anyhow::Result<Option<String>>`, `has_profile_key(&self, id: &str) -> bool`
  - `stt_key(&self) -> anyhow::Result<Option<String>>`, `set_stt_key(&self, key: &str) -> anyhow::Result<()>`
  - `resolve_profile(&self, name_or_id: Option<&str>) -> anyhow::Result<Option<ProviderProfile>>`

- [ ] **Step 1: Write the failing test** — `tests/settings_store_test.rs`

```rust
use auto_subtitle::settings::secrets::{MemoryStore, SecretsBackend};
use auto_subtitle::settings::*;

fn open(dir: &std::path::Path) -> SettingsStore {
    SettingsStore::open(dir, Box::new(MemoryStore::default()), SecretsBackend::Keyring).unwrap()
}

fn deepseek() -> ProviderProfile {
    ProviderProfile {
        id: String::new(),
        name: "DeepSeek".into(),
        kind: ProviderKind::DeepSeek,
        base_url: None,
        model: "deepseek-chat".into(),
    }
}

#[test]
fn missing_file_creates_defaults_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = open(dir.path());
    assert_eq!(s.settings().profiles[0].name, "Ollama local");
    assert!(dir.path().join("settings.json").exists());
    assert_eq!(s.take_load_warning(), None);
}

#[test]
fn corrupt_file_is_backed_up_with_warning() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("settings.json"), "{not json").unwrap();
    let mut s = open(dir.path());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("settings.json.bak")).unwrap(),
        "{not json"
    );
    let w = s.take_load_warning().unwrap();
    assert!(w.contains("settings.json.bak"));
    assert_eq!(s.take_load_warning(), None);
    assert_eq!(s.settings().profiles.len(), 1);
}

#[test]
fn upsert_assigns_id_stores_key_outside_json_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = open(dir.path());
    let saved = s.upsert_profile(deepseek(), Some(" sk-123 ".into())).unwrap();
    assert!(!saved.id.is_empty());
    assert_eq!(s.profile_key(&saved.id).unwrap().as_deref(), Some("sk-123"));
    assert!(s.has_profile_key(&saved.id));

    let json = std::fs::read_to_string(dir.path().join("settings.json")).unwrap();
    assert!(json.contains("DeepSeek") && !json.contains("sk-123"));
    assert!(!dir.path().join("settings.json.tmp").exists());

    // None keeps the key, Some("") deletes it
    let mut edited = saved.clone();
    edited.model = "deepseek-reasoner".into();
    s.upsert_profile(edited, None).unwrap();
    assert!(s.has_profile_key(&saved.id));
    s.upsert_profile(saved.clone(), Some(String::new())).unwrap();
    assert!(!s.has_profile_key(&saved.id));

    let reopened = open(dir.path());
    assert_eq!(reopened.settings().profiles.len(), 2);
}

#[test]
fn upsert_rejects_invalid_profile() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = open(dir.path());
    let mut dup = deepseek();
    dup.name = "ollama LOCAL".into();
    let err = s.upsert_profile(dup, None).unwrap_err();
    assert_eq!(err.to_string(), "Tên profile đã tồn tại");
}

#[test]
fn delete_moves_default_and_removes_key() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = open(dir.path());
    let ollama_id = s.settings().profiles[0].id.clone();
    let ds = s.upsert_profile(deepseek(), Some("k".into())).unwrap();
    s.set_default_profile(&ds.id).unwrap();
    s.delete_profile(&ds.id).unwrap();
    assert_eq!(s.settings().default_profile_id.as_deref(), Some(ollama_id.as_str()));
    assert_eq!(s.profile_key(&ds.id).unwrap(), None);
    s.delete_profile(&ollama_id).unwrap();
    assert_eq!(s.settings().default_profile_id, None);
    assert!(s.set_default_profile("nope").is_err());
}

#[test]
fn first_profile_becomes_default_when_none() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = open(dir.path());
    let id = s.settings().profiles[0].id.clone();
    s.delete_profile(&id).unwrap();
    let ds = s.upsert_profile(deepseek(), None).unwrap();
    assert_eq!(s.settings().default_profile_id.as_deref(), Some(ds.id.as_str()));
}

#[test]
fn general_and_stt_key() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = open(dir.path());
    let mut g = GeneralSettings::default();
    g.target_lang = "en".into();
    s.set_general(g.clone()).unwrap();
    assert_eq!(open(dir.path()).settings().general, g);

    s.set_stt_key("w-1").unwrap();
    assert_eq!(s.stt_key().unwrap().as_deref(), Some("w-1"));
    s.set_stt_key("").unwrap();
    assert_eq!(s.stt_key().unwrap(), None);
}

#[test]
fn resolve_profile() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = open(dir.path());
    let ds = s.upsert_profile(deepseek(), None).unwrap();
    assert_eq!(s.resolve_profile(None).unwrap().unwrap().name, "Ollama local");
    assert_eq!(s.resolve_profile(Some("deepseek")).unwrap().unwrap().id, ds.id);
    assert_eq!(s.resolve_profile(Some(&ds.id)).unwrap().unwrap().name, "DeepSeek");
    let err = s.resolve_profile(Some("claude")).unwrap_err().to_string();
    assert!(err.contains("Không tìm thấy profile 'claude'"));
    assert!(err.contains("Ollama local") && err.contains("DeepSeek"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test settings_store_test`
Expected: compile error `cannot find type SettingsStore`.

- [ ] **Step 3: Implement** — `src/settings/store.rs`

```rust
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};

use super::model::{GeneralSettings, ProviderProfile, Settings};
use super::secrets::{open_secret_store, profile_account, SecretStore, SecretsBackend, STT_ACCOUNT};

pub fn default_config_dir() -> anyhow::Result<PathBuf> {
    Ok(dirs::config_dir()
        .context("không xác định được thư mục cấu hình")?
        .join("echo"))
}

pub struct SettingsStore {
    path: PathBuf,
    settings: Settings,
    secrets: Box<dyn SecretStore>,
    backend: SecretsBackend,
    load_warning: Option<String>,
}

impl SettingsStore {
    pub fn open_default() -> anyhow::Result<Self> {
        let dir = default_config_dir()?;
        std::fs::create_dir_all(&dir)?;
        let (secrets, backend) = open_secret_store(&dir);
        Self::open(&dir, secrets, backend)
    }

    pub fn open(
        dir: &Path,
        secrets: Box<dyn SecretStore>,
        backend: SecretsBackend,
    ) -> anyhow::Result<Self> {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("không tạo được thư mục {}", dir.display()))?;
        let path = dir.join("settings.json");
        let (settings, load_warning, needs_save) = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Settings>(&text) {
                Ok(s) => (s, None, false),
                Err(e) => {
                    let bak = dir.join("settings.json.bak");
                    std::fs::rename(&path, &bak)?;
                    let warning = format!(
                        "File cài đặt bị hỏng ({e}); đã sao lưu sang {} và tạo cài đặt mặc định",
                        bak.display()
                    );
                    (Settings::with_defaults(), Some(warning), true)
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                (Settings::with_defaults(), None, true)
            }
            Err(e) => return Err(e).with_context(|| format!("không đọc được {}", path.display())),
        };
        let store = Self { path, settings, secrets, backend, load_warning };
        if needs_save {
            store.persist()?;
        }
        Ok(store)
    }

    fn persist(&self) -> anyhow::Result<()> {
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&self.settings)?)
            .with_context(|| format!("không ghi được {}", tmp.display()))?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn backend(&self) -> SecretsBackend {
        self.backend
    }

    pub fn take_load_warning(&mut self) -> Option<String> {
        self.load_warning.take()
    }

    pub fn upsert_profile(
        &mut self,
        mut profile: ProviderProfile,
        api_key: Option<String>,
    ) -> anyhow::Result<ProviderProfile> {
        profile.name = profile.name.trim().to_string();
        profile.model = profile.model.trim().to_string();
        if profile.id.trim().is_empty() {
            profile.id = uuid::Uuid::new_v4().to_string();
        }
        self.settings.validate_profile(&profile).map_err(anyhow::Error::msg)?;

        let account = profile_account(&profile.id);
        match api_key.as_deref().map(str::trim) {
            None => {}
            Some("") => self.secrets.delete(&account)?,
            Some(k) => self.secrets.set(&account, k)?,
        }

        match self.settings.profiles.iter_mut().find(|p| p.id == profile.id) {
            Some(existing) => *existing = profile.clone(),
            None => self.settings.profiles.push(profile.clone()),
        }
        if self.settings.default_profile().is_none() {
            self.settings.default_profile_id = Some(profile.id.clone());
        }
        self.persist()?;
        Ok(profile)
    }

    pub fn delete_profile(&mut self, id: &str) -> anyhow::Result<()> {
        self.settings.profiles.retain(|p| p.id != id);
        self.secrets.delete(&profile_account(id))?;
        if self.settings.default_profile_id.as_deref() == Some(id) {
            self.settings.default_profile_id = self.settings.profiles.first().map(|p| p.id.clone());
        }
        self.persist()
    }

    pub fn set_default_profile(&mut self, id: &str) -> anyhow::Result<()> {
        if !self.settings.profiles.iter().any(|p| p.id == id) {
            return Err(anyhow!("Không tìm thấy profile"));
        }
        self.settings.default_profile_id = Some(id.to_string());
        self.persist()
    }

    pub fn set_general(&mut self, general: GeneralSettings) -> anyhow::Result<()> {
        self.settings.general = general;
        self.persist()
    }

    pub fn profile_key(&self, id: &str) -> anyhow::Result<Option<String>> {
        self.secrets.get(&profile_account(id))
    }

    pub fn has_profile_key(&self, id: &str) -> bool {
        matches!(self.profile_key(id), Ok(Some(_)))
    }

    pub fn stt_key(&self) -> anyhow::Result<Option<String>> {
        self.secrets.get(STT_ACCOUNT)
    }

    pub fn set_stt_key(&self, key: &str) -> anyhow::Result<()> {
        match key.trim() {
            "" => self.secrets.delete(STT_ACCOUNT),
            k => self.secrets.set(STT_ACCOUNT, k),
        }
    }

    pub fn resolve_profile(&self, name_or_id: Option<&str>) -> anyhow::Result<Option<ProviderProfile>> {
        match name_or_id {
            None => Ok(self.settings.default_profile().cloned()),
            Some(q) => self.settings.find_profile(q).cloned().map(Some).ok_or_else(|| {
                let names: Vec<&str> = self.settings.profiles.iter().map(|p| p.name.as_str()).collect();
                anyhow!("Không tìm thấy profile '{q}'. Có: {}", names.join(", "))
            }),
        }
    }
}
```

`src/settings/mod.rs` — final version:

```rust
pub mod model;
pub mod secrets;
pub mod store;

pub use model::{GeneralSettings, ProviderKind, ProviderProfile, Settings, SETTINGS_VERSION};
pub use secrets::{SecretStore, SecretsBackend};
pub use store::SettingsStore;
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test settings_store_test --test settings_secrets_test --test settings_model_test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/settings tests/settings_store_test.rs
git commit -m "feat(settings): add SettingsStore with atomic persistence and recovery"
```

---

### Task 4: LlmError and OpenAI-protocol client

**Files:**
- Create: `src/llm/mod.rs`, `src/llm/error.rs`, `src/llm/openai.rs`
- Modify: `src/lib.rs`
- Test: `tests/llm_openai_test.rs`

**Interfaces:**
- Produces:
  - `#[async_trait] trait ChatClient: Send + Sync { async fn chat(&self, system: Option<&str>, user: &str) -> Result<String, LlmError>; async fn list_models(&self) -> Result<Vec<String>, LlmError>; }`
  - `enum LlmError { Auth, NotFound, RateLimit, Timeout(String), Network(String), Config(String), Other { status: Option<u16>, body: String } }` (`Debug, Clone, PartialEq, Eq`, `Display`, `std::error::Error`) with `from_status(u16, String)`, `from_reqwest(&reqwest::Error, &str)`, `is_retryable()`, `should_backoff()`, `user_message()`
  - `OpenAiChatClient::new(base_url: String, api_key: Option<String>, model: String)`
  - `pub(crate) fn http_client() -> reqwest::Client` (180s timeout); `pub(crate) async fn send_json<T: DeserializeOwned>(req: reqwest::RequestBuilder, url: &str) -> Result<T, LlmError>`

- [ ] **Step 1: Write the failing test** — `tests/llm_openai_test.rs`

```rust
use auto_subtitle::llm::{openai::OpenAiChatClient, ChatClient, LlmError};
use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn chat_sends_bearer_system_and_user() {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(header("authorization", "Bearer k"))
        .and(body_partial_json(json!({
            "model": "m",
            "messages": [
                {"role": "system", "content": "SYS"},
                {"role": "user", "content": "hi"}
            ]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"choices":[{"message":{"content":"hello"}}]}),
        ))
        .mount(&s)
        .await;
    let c = OpenAiChatClient::new(format!("{}/v1", s.uri()), Some("k".into()), "m".into());
    assert_eq!(c.chat(Some("SYS"), "hi").await.unwrap(), "hello");
}

#[tokio::test]
async fn chat_without_system_sends_only_user() {
    let s = MockServer::start().await;
    Mock::given(path("/chat/completions"))
        .and(body_partial_json(json!({"messages":[{"role":"user","content":"hi"}]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"choices":[{"message":{"content":"ok"}}]}),
        ))
        .mount(&s)
        .await;
    let c = OpenAiChatClient::new(s.uri(), None, "m".into());
    assert_eq!(c.chat(None, "hi").await.unwrap(), "ok");
}

#[tokio::test]
async fn list_models_sorted() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"data":[{"id":"gpt-4o"},{"id":"deepseek-chat"}]}),
        ))
        .mount(&s)
        .await;
    let c = OpenAiChatClient::new(s.uri(), Some("k".into()), "m".into());
    assert_eq!(c.list_models().await.unwrap(), vec!["deepseek-chat", "gpt-4o"]);
}

#[tokio::test]
async fn status_mapping() {
    for (code, expected) in [
        (401u16, LlmError::Auth),
        (403, LlmError::Auth),
        (404, LlmError::NotFound),
        (429, LlmError::RateLimit),
    ] {
        let s = MockServer::start().await;
        Mock::given(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(code))
            .mount(&s)
            .await;
        let c = OpenAiChatClient::new(s.uri(), Some("k".into()), "m".into());
        assert_eq!(c.chat(None, "x").await.unwrap_err(), expected);
    }

    let s = MockServer::start().await;
    Mock::given(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_string("x".repeat(300)))
        .mount(&s)
        .await;
    let c = OpenAiChatClient::new(s.uri(), Some("k".into()), "m".into());
    match c.chat(None, "x").await.unwrap_err() {
        LlmError::Other { status, body } => {
            assert_eq!(status, Some(500));
            assert_eq!(body.len(), 200);
        }
        e => panic!("unexpected {e:?}"),
    }
}

#[tokio::test]
async fn empty_choices_is_retryable_other() {
    let s = MockServer::start().await;
    Mock::given(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"choices":[]})))
        .mount(&s)
        .await;
    let c = OpenAiChatClient::new(s.uri(), None, "m".into());
    let e = c.chat(None, "x").await.unwrap_err();
    assert!(matches!(e, LlmError::Other { status: None, .. }));
    assert!(e.is_retryable());
}

#[tokio::test]
async fn unreachable_host_is_network_error() {
    let c = OpenAiChatClient::new("http://127.0.0.1:9".into(), None, "m".into());
    let e = c.chat(None, "x").await.unwrap_err();
    assert_eq!(e, LlmError::Network("127.0.0.1".into()));
    assert_eq!(e.user_message(), "Không kết nối được tới 127.0.0.1");
}

#[test]
fn retry_classification_and_messages() {
    assert!(!LlmError::Auth.is_retryable());
    assert!(!LlmError::NotFound.is_retryable());
    assert!(!LlmError::Config("x".into()).is_retryable());
    assert!(!LlmError::Other { status: Some(400), body: String::new() }.is_retryable());
    assert!(LlmError::RateLimit.is_retryable() && LlmError::RateLimit.should_backoff());
    let e500 = LlmError::Other { status: Some(503), body: String::new() };
    assert!(e500.is_retryable() && e500.should_backoff());
    assert!(LlmError::Timeout("h".into()).is_retryable());
    assert!(!LlmError::Timeout("h".into()).should_backoff());

    assert_eq!(LlmError::Auth.user_message(), "API key không hợp lệ");
    assert_eq!(LlmError::NotFound.user_message(), "Sai base URL hoặc tên model");
    assert_eq!(LlmError::RateLimit.user_message(), "Bị giới hạn tốc độ, thử lại sau");
    assert_eq!(LlmError::Timeout("h".into()).user_message(), "Không kết nối được tới h");
    assert_eq!(LlmError::Config("Thiếu".into()).user_message(), "Thiếu");
    assert_eq!(
        LlmError::Other { status: Some(500), body: "boom".into() }.user_message(),
        "Lỗi provider (500): boom"
    );
    assert_eq!(
        LlmError::Other { status: None, body: "bad".into() }.user_message(),
        "Lỗi provider: bad"
    );
    assert_eq!(LlmError::Auth.to_string(), "API key không hợp lệ");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test llm_openai_test`
Expected: compile error `could not find llm in auto_subtitle`.

- [ ] **Step 3: Implement** — `src/llm/error.rs`

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmError {
    Auth,
    NotFound,
    RateLimit,
    Timeout(String),
    Network(String),
    Config(String),
    Other { status: Option<u16>, body: String },
}

impl LlmError {
    pub fn from_status(status: u16, body: String) -> Self {
        match status {
            401 | 403 => Self::Auth,
            404 => Self::NotFound,
            429 => Self::RateLimit,
            _ => Self::Other { status: Some(status), body: body.chars().take(200).collect() },
        }
    }

    pub fn from_reqwest(err: &reqwest::Error, url: &str) -> Self {
        let host = reqwest::Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(String::from))
            .unwrap_or_else(|| url.to_string());
        if err.is_timeout() {
            Self::Timeout(host)
        } else if err.is_connect() || err.is_request() {
            Self::Network(host)
        } else {
            Self::Other { status: err.status().map(|s| s.as_u16()), body: err.to_string() }
        }
    }

    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Auth | Self::NotFound | Self::Config(_) => false,
            Self::Other { status: Some(s), .. } => *s >= 500,
            _ => true,
        }
    }

    pub fn should_backoff(&self) -> bool {
        matches!(self, Self::RateLimit) || matches!(self, Self::Other { status: Some(s), .. } if *s >= 500)
    }

    pub fn user_message(&self) -> String {
        match self {
            Self::Auth => "API key không hợp lệ".into(),
            Self::NotFound => "Sai base URL hoặc tên model".into(),
            Self::RateLimit => "Bị giới hạn tốc độ, thử lại sau".into(),
            Self::Timeout(h) | Self::Network(h) => format!("Không kết nối được tới {h}"),
            Self::Config(m) => m.clone(),
            Self::Other { status: Some(s), body } => format!("Lỗi provider ({s}): {body}"),
            Self::Other { status: None, body } => format!("Lỗi provider: {body}"),
        }
    }
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.user_message())
    }
}

impl std::error::Error for LlmError {}
```

`src/llm/mod.rs` (Task 6 extends it):

```rust
pub mod error;
pub mod openai;

pub use error::LlmError;

use serde::de::DeserializeOwned;

#[async_trait::async_trait]
pub trait ChatClient: Send + Sync {
    async fn chat(&self, system: Option<&str>, user: &str) -> Result<String, LlmError>;
    async fn list_models(&self) -> Result<Vec<String>, LlmError>;
}

pub(crate) fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

pub(crate) async fn send_json<T: DeserializeOwned>(
    req: reqwest::RequestBuilder,
    url: &str,
) -> Result<T, LlmError> {
    let resp = req.send().await.map_err(|e| LlmError::from_reqwest(&e, url))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(LlmError::from_status(status.as_u16(), body));
    }
    resp.json::<T>().await.map_err(|e| LlmError::Other {
        status: None,
        body: format!("Phản hồi không hợp lệ: {e}"),
    })
}
```

`src/llm/openai.rs`:

```rust
use serde::Deserialize;
use serde_json::json;

use super::{http_client, send_json, ChatClient, LlmError};

pub struct OpenAiChatClient {
    base_url: String,
    api_key: Option<String>,
    model: String,
    client: reqwest::Client,
}

impl OpenAiChatClient {
    pub fn new(base_url: String, api_key: Option<String>, model: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            model,
            client: http_client(),
        }
    }

    fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(k) => req.bearer_auth(k),
            None => req,
        }
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelItem>,
}

#[derive(Deserialize)]
struct ModelItem {
    id: String,
}

#[async_trait::async_trait]
impl ChatClient for OpenAiChatClient {
    async fn chat(&self, system: Option<&str>, user: &str) -> Result<String, LlmError> {
        let url = format!("{}/chat/completions", self.base_url);
        let mut messages = Vec::new();
        if let Some(sys) = system {
            messages.push(json!({"role": "system", "content": sys}));
        }
        messages.push(json!({"role": "user", "content": user}));
        let body = json!({"model": self.model, "temperature": 0.2, "messages": messages});
        let resp: ChatResponse = send_json(self.auth(self.client.post(&url)).json(&body), &url).await?;
        resp.choices
            .into_iter()
            .next()
            .map(|c| c.message.content.unwrap_or_default())
            .ok_or_else(|| LlmError::Other { status: None, body: "Phản hồi không có choices".into() })
    }

    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        let url = format!("{}/models", self.base_url);
        let resp: ModelsResponse = send_json(self.auth(self.client.get(&url)), &url).await?;
        let mut ids: Vec<String> = resp.data.into_iter().map(|m| m.id).collect();
        ids.sort();
        Ok(ids)
    }
}
```

`src/lib.rs` — add `pub mod llm;` after `pub mod downloader;`.

- [ ] **Step 4: Run tests**

Run: `cargo test --test llm_openai_test`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/llm tests/llm_openai_test.rs
git commit -m "feat(llm): add ChatClient trait, LlmError and OpenAI-protocol client"
```

---

### Task 5: Anthropic client

**Files:**
- Create: `src/llm/anthropic.rs`
- Modify: `src/llm/mod.rs` (add `pub mod anthropic;`)
- Test: `tests/llm_anthropic_test.rs`

**Interfaces:**
- Consumes: `ChatClient`, `LlmError`, `http_client`, `send_json` (Task 4).
- Produces: `AnthropicChatClient::new(base_url: String, api_key: Option<String>, model: String)`.

- [ ] **Step 1: Write the failing test** — `tests/llm_anthropic_test.rs`

```rust
use auto_subtitle::llm::{anthropic::AnthropicChatClient, ChatClient, LlmError};
use serde_json::json;
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn chat_uses_messages_api_headers_and_top_level_system() {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", "k"))
        .and(header("anthropic-version", "2023-06-01"))
        .and(body_partial_json(json!({
            "model": "claude-x",
            "max_tokens": 8192,
            "system": "SYS",
            "messages": [{"role": "user", "content": "hi"}]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": [
                {"type": "text", "text": "a"},
                {"type": "tool_use", "id": "t"},
                {"type": "text", "text": "b"}
            ]
        })))
        .mount(&s)
        .await;
    let c = AnthropicChatClient::new(s.uri(), Some("k".into()), "claude-x".into());
    assert_eq!(c.chat(Some("SYS"), "hi").await.unwrap(), "ab");
}

#[tokio::test]
async fn chat_without_system_omits_field() {
    let s = MockServer::start().await;
    Mock::given(path("/v1/messages"))
        .respond_with(move |req: &wiremock::Request| {
            let v: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
            assert!(v.get("system").is_none());
            ResponseTemplate::new(200).set_body_json(json!({"content":[{"type":"text","text":"ok"}]}))
        })
        .mount(&s)
        .await;
    let c = AnthropicChatClient::new(format!("{}/", s.uri()), Some("k".into()), "m".into());
    assert_eq!(c.chat(None, "hi").await.unwrap(), "ok");
}

#[tokio::test]
async fn list_models_and_auth_error() {
    let s = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .and(header("x-api-key", "k"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"data":[{"id":"claude-b"},{"id":"claude-a"}]}),
        ))
        .mount(&s)
        .await;
    Mock::given(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&s)
        .await;
    let c = AnthropicChatClient::new(s.uri(), Some("k".into()), "m".into());
    assert_eq!(c.list_models().await.unwrap(), vec!["claude-a", "claude-b"]);
    assert_eq!(c.chat(None, "x").await.unwrap_err(), LlmError::Auth);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test llm_anthropic_test`
Expected: compile error `could not find anthropic in llm`.

- [ ] **Step 3: Implement** — `src/llm/anthropic.rs`

```rust
use serde::Deserialize;
use serde_json::json;

use super::{http_client, send_json, ChatClient, LlmError};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 8192;

pub struct AnthropicChatClient {
    base_url: String,
    api_key: Option<String>,
    model: String,
    client: reqwest::Client,
}

impl AnthropicChatClient {
    pub fn new(base_url: String, api_key: Option<String>, model: String) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            model,
            client: http_client(),
        }
    }

    fn headers(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        let req = req.header("anthropic-version", ANTHROPIC_VERSION);
        match &self.api_key {
            Some(k) => req.header("x-api-key", k),
            None => req,
        }
    }
}

#[derive(Deserialize)]
struct MessagesResponse {
    #[serde(default)]
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelItem>,
}

#[derive(Deserialize)]
struct ModelItem {
    id: String,
}

#[async_trait::async_trait]
impl ChatClient for AnthropicChatClient {
    async fn chat(&self, system: Option<&str>, user: &str) -> Result<String, LlmError> {
        let url = format!("{}/v1/messages", self.base_url);
        let mut body = json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "temperature": 0.2,
            "messages": [{"role": "user", "content": user}]
        });
        if let Some(sys) = system {
            body["system"] = json!(sys);
        }
        let resp: MessagesResponse =
            send_json(self.headers(self.client.post(&url)).json(&body), &url).await?;
        Ok(resp
            .content
            .into_iter()
            .filter(|b| b.kind == "text")
            .filter_map(|b| b.text)
            .collect())
    }

    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        let url = format!("{}/v1/models", self.base_url);
        let resp: ModelsResponse = send_json(self.headers(self.client.get(&url)), &url).await?;
        let mut ids: Vec<String> = resp.data.into_iter().map(|m| m.id).collect();
        ids.sort();
        Ok(ids)
    }
}
```

`src/llm/mod.rs` — add `pub mod anthropic;` above `pub mod error;`.

- [ ] **Step 4: Run tests**

Run: `cargo test --test llm_anthropic_test`
Expected: 3 passed.

- [ ] **Step 5: Commit**

```bash
git add src/llm tests/llm_anthropic_test.rs
git commit -m "feat(llm): add Anthropic Messages API client"
```

---

### Task 6: Ollama client, client factory, connection test

**Files:**
- Create: `src/llm/ollama.rs`
- Modify: `src/llm/mod.rs`
- Test: `tests/llm_factory_test.rs`

**Interfaces:**
- Consumes: Tasks 1, 4, 5.
- Produces:
  - `OllamaChatClient::new(base_url: String, model: String)`
  - `ollama::ollama_model_present(models: &[String], model: &str) -> bool`
  - `ollama::ensure_ollama_model(client: &dyn ChatClient, base_url: &str, model: &str) -> anyhow::Result<()>`
  - `build_chat_client(profile: &ProviderProfile, api_key: Option<String>) -> Result<Box<dyn ChatClient>, LlmError>`
  - `#[derive(Debug, Clone, Serialize, PartialEq, Eq)] struct ConnectionTest { ok: bool, latency_ms: u64, error: Option<String> }`
  - `const TEST_TIMEOUT: Duration` (20s); `async fn test_connection(client: &dyn ChatClient, timeout: Duration) -> ConnectionTest`

- [ ] **Step 1: Write the failing test** — `tests/llm_factory_test.rs`

```rust
use auto_subtitle::llm::ollama::{ensure_ollama_model, ollama_model_present, OllamaChatClient};
use auto_subtitle::llm::{build_chat_client, test_connection, ChatClient, LlmError};
use auto_subtitle::settings::{ProviderKind, ProviderProfile};
use serde_json::json;
use std::time::Duration;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn profile(kind: ProviderKind, base: Option<String>) -> ProviderProfile {
    ProviderProfile { id: "1".into(), name: "p".into(), kind, base_url: base, model: "m".into() }
}

#[tokio::test]
async fn ollama_chat_and_tags() {
    let s = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .and(body_partial_json(json!({
            "model": "m",
            "stream": false,
            "messages": [{"role":"system","content":"S"},{"role":"user","content":"u"}]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"message":{"content":"r"}})))
        .mount(&s)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"models":[{"name":"qwen2.5:7b"},{"name":"llama3:latest"}]}),
        ))
        .mount(&s)
        .await;
    let c = OllamaChatClient::new(s.uri(), "m".into());
    assert_eq!(c.chat(Some("S"), "u").await.unwrap(), "r");
    assert_eq!(c.list_models().await.unwrap(), vec!["llama3:latest", "qwen2.5:7b"]);
    ensure_ollama_model(&c, &s.uri(), "llama3").await.unwrap();
    let err = ensure_ollama_model(&c, &s.uri(), "mistral").await.unwrap_err();
    assert!(format!("{err:#}").contains("ollama pull mistral"));
}

#[tokio::test]
async fn ollama_unreachable_is_actionable() {
    let c = OllamaChatClient::new("http://127.0.0.1:9".into(), "m".into());
    let err = ensure_ollama_model(&c, "http://127.0.0.1:9", "m").await.unwrap_err();
    assert!(format!("{err:#}").contains("ollama serve"));
}

#[test]
fn model_presence_rules() {
    let tags = vec!["qwen2.5:latest".to_string(), "llama3:8b".to_string()];
    assert!(ollama_model_present(&tags, "qwen2.5"));
    assert!(ollama_model_present(&tags, "qwen2.5:latest"));
    assert!(ollama_model_present(&tags, "llama3:8b"));
    assert!(!ollama_model_present(&tags, "llama3"));
    assert!(ollama_model_present(&["mistral".to_string()], "mistral:latest"));
}

#[tokio::test]
async fn factory_routes_kinds_to_protocols() {
    let s = MockServer::start().await;
    Mock::given(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"choices":[{"message":{"content":"openai"}}]})))
        .mount(&s)
        .await;
    Mock::given(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"content":[{"type":"text","text":"anthropic"}]})))
        .mount(&s)
        .await;
    Mock::given(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"message":{"content":"ollama"}})))
        .mount(&s)
        .await;

    for (kind, expected) in [
        (ProviderKind::OpenAi, "openai"),
        (ProviderKind::OpenAiCompatible, "openai"),
        (ProviderKind::DeepSeek, "openai"),
        (ProviderKind::Anthropic, "anthropic"),
        (ProviderKind::AnthropicCompatible, "anthropic"),
        (ProviderKind::Ollama, "ollama"),
    ] {
        let c = build_chat_client(&profile(kind, Some(s.uri())), Some("k".into())).unwrap();
        assert_eq!(c.chat(None, "x").await.unwrap(), expected, "{kind:?}");
    }
}

#[test]
fn factory_rejects_compatible_without_base_url() {
    let err = build_chat_client(&profile(ProviderKind::OpenAiCompatible, None), None).err().unwrap();
    assert_eq!(err, LlmError::Config("Base URL bắt buộc với provider tương thích".into()));
}

#[tokio::test]
async fn connection_test_ok_error_and_timeout() {
    let s = MockServer::start().await;
    Mock::given(path("/ok/chat/completions"))
        .and(body_partial_json(json!({"messages":[{"role":"user","content":"Reply with: OK"}]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"choices":[{"message":{"content":"OK"}}]})))
        .mount(&s)
        .await;
    Mock::given(path("/bad/chat/completions"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&s)
        .await;
    Mock::given(path("/slow/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(500))
                .set_body_json(json!({"choices":[{"message":{"content":"OK"}}]})),
        )
        .mount(&s)
        .await;

    let ok = build_chat_client(&profile(ProviderKind::OpenAi, Some(format!("{}/ok", s.uri()))), None).unwrap();
    let r = test_connection(ok.as_ref(), Duration::from_secs(5)).await;
    assert!(r.ok && r.error.is_none());

    let bad = build_chat_client(&profile(ProviderKind::OpenAi, Some(format!("{}/bad", s.uri()))), None).unwrap();
    let r = test_connection(bad.as_ref(), Duration::from_secs(5)).await;
    assert!(!r.ok);
    assert_eq!(r.error.as_deref(), Some("API key không hợp lệ"));

    let slow = build_chat_client(&profile(ProviderKind::OpenAi, Some(format!("{}/slow", s.uri()))), None).unwrap();
    let r = test_connection(slow.as_ref(), Duration::from_millis(50)).await;
    assert!(!r.ok);
    assert_eq!(r.error.as_deref(), Some("Hết thời gian chờ"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test llm_factory_test`
Expected: compile error `could not find ollama in llm`.

- [ ] **Step 3: Implement** — `src/llm/ollama.rs`

```rust
use serde::Deserialize;
use serde_json::json;

use super::{http_client, send_json, ChatClient, LlmError};

pub struct OllamaChatClient {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl OllamaChatClient {
    pub fn new(base_url: String, model: String) -> Self {
        Self { base_url: base_url.trim_end_matches('/').to_string(), model, client: http_client() }
    }
}

#[derive(Deserialize)]
struct ChatResponse {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    #[serde(default)]
    content: String,
}

#[derive(Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagItem>,
}

#[derive(Deserialize)]
struct TagItem {
    name: String,
}

#[async_trait::async_trait]
impl ChatClient for OllamaChatClient {
    async fn chat(&self, system: Option<&str>, user: &str) -> Result<String, LlmError> {
        let url = format!("{}/api/chat", self.base_url);
        let mut messages = Vec::new();
        if let Some(sys) = system {
            messages.push(json!({"role": "system", "content": sys}));
        }
        messages.push(json!({"role": "user", "content": user}));
        let body = json!({
            "model": self.model,
            "stream": false,
            "options": {"temperature": 0.2},
            "messages": messages
        });
        let resp: ChatResponse = send_json(self.client.post(&url).json(&body), &url).await?;
        Ok(resp.message.content)
    }

    async fn list_models(&self) -> Result<Vec<String>, LlmError> {
        let url = format!("{}/api/tags", self.base_url);
        let resp: TagsResponse = send_json(self.client.get(&url), &url).await?;
        let mut names: Vec<String> = resp.models.into_iter().map(|m| m.name).collect();
        names.sort();
        Ok(names)
    }
}

pub fn ollama_model_present(models: &[String], model: &str) -> bool {
    models.iter().any(|m| {
        m == model
            || (!model.contains(':') && *m == format!("{model}:latest"))
            || (model.ends_with(":latest") && m == model.trim_end_matches(":latest"))
    })
}

pub async fn ensure_ollama_model(
    client: &dyn ChatClient,
    base_url: &str,
    model: &str,
) -> anyhow::Result<()> {
    let models = client.list_models().await.map_err(|e| {
        anyhow::anyhow!(
            "Không kết nối được với Ollama tại {base_url}. Hãy kiểm tra xem Ollama đã chạy chưa (chạy 'ollama serve') hoặc dùng --translate-engine openai-compat. ({e})"
        )
    })?;
    if !ollama_model_present(&models, model) {
        anyhow::bail!("Model '{model}' không tìm thấy trong Ollama. Hãy chạy 'ollama pull {model}' trước.");
    }
    Ok(())
}
```

`src/llm/mod.rs` — full final version:

```rust
pub mod anthropic;
pub mod error;
pub mod ollama;
pub mod openai;

pub use error::LlmError;

use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::settings::{ProviderKind, ProviderProfile};

pub const TEST_TIMEOUT: Duration = Duration::from_secs(20);

#[async_trait::async_trait]
pub trait ChatClient: Send + Sync {
    async fn chat(&self, system: Option<&str>, user: &str) -> Result<String, LlmError>;
    async fn list_models(&self) -> Result<Vec<String>, LlmError>;
}

pub fn build_chat_client(
    profile: &ProviderProfile,
    api_key: Option<String>,
) -> Result<Box<dyn ChatClient>, LlmError> {
    let base = profile
        .effective_base_url()
        .ok_or_else(|| LlmError::Config("Base URL bắt buộc với provider tương thích".into()))?;
    let model = profile.model.clone();
    let key = api_key.filter(|k| !k.trim().is_empty());
    Ok(match profile.kind {
        ProviderKind::OpenAi | ProviderKind::OpenAiCompatible | ProviderKind::DeepSeek => {
            Box::new(openai::OpenAiChatClient::new(base, key, model))
        }
        ProviderKind::Anthropic | ProviderKind::AnthropicCompatible => {
            Box::new(anthropic::AnthropicChatClient::new(base, key, model))
        }
        ProviderKind::Ollama => Box::new(ollama::OllamaChatClient::new(base, model)),
    })
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ConnectionTest {
    pub ok: bool,
    pub latency_ms: u64,
    pub error: Option<String>,
}

pub async fn test_connection(client: &dyn ChatClient, timeout: Duration) -> ConnectionTest {
    let start = Instant::now();
    let res = tokio::time::timeout(timeout, client.chat(None, "Reply with: OK")).await;
    let latency_ms = start.elapsed().as_millis() as u64;
    let error = match res {
        Ok(Ok(_)) => None,
        Ok(Err(e)) => Some(e.user_message()),
        Err(_) => Some("Hết thời gian chờ".to_string()),
    };
    ConnectionTest { ok: error.is_none(), latency_ms, error }
}

pub(crate) fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

pub(crate) async fn send_json<T: DeserializeOwned>(
    req: reqwest::RequestBuilder,
    url: &str,
) -> Result<T, LlmError> {
    let resp = req.send().await.map_err(|e| LlmError::from_reqwest(&e, url))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(LlmError::from_status(status.as_u16(), body));
    }
    resp.json::<T>().await.map_err(|e| LlmError::Other {
        status: None,
        body: format!("Phản hồi không hợp lệ: {e}"),
    })
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test llm_factory_test --test llm_openai_test --test llm_anthropic_test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/llm tests/llm_factory_test.rs
git commit -m "feat(llm): add Ollama client, profile-based client factory and connection test"
```

---

### Task 7: LlmTranslator, remove old translators, pipeline retry rules

**Files:**
- Create: `src/translate/llm_translator.rs`
- Modify: `src/translate/mod.rs`, `src/pipeline.rs` (retry loop in `translate_all_with_progress`)
- Delete: `src/translate/ollama.rs`, `src/translate/openai_compat.rs`
- Modify: `tests/translator_test.rs`, `tests/pipeline_test.rs`

**Interfaces:**
- Consumes: `ChatClient`, `LlmError` (Task 4).
- Produces: `LlmTranslator::new(client: Box<dyn ChatClient>, system_prompt: Option<String>) -> Self` implementing `Translator`.

- [ ] **Step 1: Write the failing tests**

Replace everything in `tests/translator_test.rs` **after** the `parse_ignores_extras_and_out_of_range` test (i.e. delete the four Ollama/OpenAI tests) and change the imports at the top to:

```rust
use auto_subtitle::llm::{ChatClient, LlmError};
use auto_subtitle::subtitle::model::SubtitleSegment as S;
use auto_subtitle::translate::{prompt::*, LlmTranslator, Translator};
use std::sync::{Arc, Mutex};
```

Append:

```rust
struct Recorder {
    seen: Arc<Mutex<Vec<(Option<String>, String)>>>,
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
async fn llm_translator_forwards_prompt_and_system() {
    let seen = Arc::new(Mutex::new(vec![]));
    let t = LlmTranslator::new(
        Box::new(Recorder { seen: seen.clone(), reply: Ok("[1] A\n[2] B / C".into()) }),
        Some("Bạn là chuyên gia Toán".into()),
    );
    assert_eq!(t.translate_batch(&batch(), "en", "vi").await.unwrap(), vec!["A", "B\nC"]);
    let calls = seen.lock().unwrap();
    assert_eq!(calls[0].0.as_deref(), Some("Bạn là chuyên gia Toán"));
    assert!(calls[0].1.contains("[2] How are / you?"));
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
```

Append to `tests/pipeline_test.rs`:

```rust
struct LlmFails(AtomicUsize, auto_subtitle::llm::LlmError, usize);
#[async_trait]
impl Translator for LlmFails {
    async fn translate_batch(&self, s: &[S], a: &str, b: &str) -> anyhow::Result<Vec<String>> {
        let n = self.0.fetch_add(1, Ordering::SeqCst);
        if n < self.2 {
            return Err(anyhow::Error::new(self.1.clone()));
        }
        Prefix.translate_batch(s, a, b).await
    }
}

#[tokio::test]
async fn auth_error_stops_without_retry() {
    let t = LlmFails(AtomicUsize::new(0), auto_subtitle::llm::LlmError::Auth, usize::MAX);
    let err = translate_all(&t, &mut many(3), "en", "vi").await.unwrap_err();
    assert_eq!(t.0.load(Ordering::SeqCst), 1);
    assert!(format!("{err:#}").contains("API key không hợp lệ"));
}

#[tokio::test(start_paused = true)]
async fn rate_limit_backs_off_then_succeeds() {
    let t = LlmFails(AtomicUsize::new(0), auto_subtitle::llm::LlmError::RateLimit, 2);
    let start = tokio::time::Instant::now();
    translate_all(&t, &mut many(3), "en", "vi").await.unwrap();
    assert_eq!(t.0.load(Ordering::SeqCst), 3);
    assert!(start.elapsed() >= std::time::Duration::from_secs(6));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test translator_test --test pipeline_test`
Expected: compile error `no LlmTranslator in translate`.

- [ ] **Step 3: Implement** — `src/translate/llm_translator.rs`

```rust
use crate::llm::ChatClient;
use crate::subtitle::model::SubtitleSegment;
use crate::translate::prompt::{build_translation_prompt, parse_translated_lines};
use crate::translate::Translator;

pub struct LlmTranslator {
    client: Box<dyn ChatClient>,
    system_prompt: Option<String>,
}

impl LlmTranslator {
    pub fn new(client: Box<dyn ChatClient>, system_prompt: Option<String>) -> Self {
        Self { client, system_prompt }
    }
}

#[async_trait::async_trait]
impl Translator for LlmTranslator {
    async fn translate_batch(
        &self,
        segments: &[SubtitleSegment],
        source_lang: &str,
        target_lang: &str,
    ) -> anyhow::Result<Vec<String>> {
        if segments.is_empty() {
            return Ok(Vec::new());
        }
        let prompt = build_translation_prompt(segments, source_lang, target_lang);
        let raw = self
            .client
            .chat(self.system_prompt.as_deref(), &prompt)
            .await
            .map_err(anyhow::Error::new)?;
        parse_translated_lines(&raw, segments.len())
    }
}
```

`src/translate/mod.rs` — replace the first five lines (`pub mod ollama;` … `pub use openai_compat::OpenAiCompatTranslator;`) with:

```rust
pub mod llm_translator;
pub mod prompt;

pub use llm_translator::LlmTranslator;
```

Delete files:

```bash
git rm src/translate/ollama.rs src/translate/openai_compat.rs
```

`src/pipeline.rs` — in `translate_all_with_progress`, replace:

```rust
        for _attempt in 1..=MAX_ATTEMPTS {
```

with:

```rust
        for attempt in 1..=MAX_ATTEMPTS {
```

and replace the `Err` arm:

```rust
                Err(err) => {
                    last_err = Some(err);
                }
```

with:

```rust
                Err(err) => {
                    let llm_err = err
                        .chain()
                        .find_map(|e| e.downcast_ref::<crate::llm::LlmError>())
                        .cloned();
                    last_err = Some(err);
                    if let Some(llm_err) = llm_err {
                        if !llm_err.is_retryable() {
                            break;
                        }
                        if llm_err.should_backoff() && attempt < MAX_ATTEMPTS {
                            tokio::time::sleep(std::time::Duration::from_secs(2 * attempt as u64))
                                .await;
                        }
                    }
                }
```

Integration tests also build the `auto-subtitle` binary, so `src/main.rs` must compile now (Task 8 rewrites it fully). Interim fix in `src/main.rs`:

Replace the three imports

```rust
use auto_subtitle::translate::ollama::OllamaTranslator;
use auto_subtitle::translate::openai_compat::OpenAiCompatTranslator;
use auto_subtitle::translate::Translator;
```

with

```rust
use auto_subtitle::llm::ollama::{ensure_ollama_model, OllamaChatClient};
use auto_subtitle::llm::openai::OpenAiChatClient;
use auto_subtitle::translate::{LlmTranslator, Translator};
```

and replace the body of the `match cli.translate_engine { ... }` with:

```rust
            TranslateEngine::Ollama => {
                let client = OllamaChatClient::new(cli.ollama_url.clone(), cli.ollama_model.clone());
                ensure_ollama_model(&client, &cli.ollama_url, &cli.ollama_model).await?;
                Some(Box::new(LlmTranslator::new(Box::new(client), None)))
            }
            TranslateEngine::OpenaiCompat => {
                let api_key = cli
                    .api_key
                    .clone()
                    .context("--api-key or OPENAI_API_KEY required for translation")?;
                let client = OpenAiChatClient::new(
                    cli.api_base_url.clone(),
                    Some(api_key),
                    cli.api_model.clone(),
                );
                Some(Box::new(LlmTranslator::new(Box::new(client), None)))
            }
```

`src-tauri` will not compile until Task 9; that is expected (root-package commands below don't build it).

- [ ] **Step 4: Run tests**

Run: `cargo test --lib && cargo test --test translator_test --test pipeline_test && cargo build --bin auto-subtitle`
Expected: all pass (the `hardsub_and_softsub_e2e_with_synthetic_video` test needs ffmpeg as before).

- [ ] **Step 5: Commit**

```bash
git add src/translate src/pipeline.rs tests/translator_test.rs tests/pipeline_test.rs
git commit -m "refactor(translate): single LlmTranslator over ChatClient; stop retrying non-retryable errors"
```

---

### Task 8: CLI profile support

**Files:**
- Modify: `src/cli.rs`, `src/main.rs`
- Test: `tests/cli_test.rs`

**Interfaces:**
- Consumes: `Settings`, `SettingsStore`, `build_chat_client`, `OllamaChatClient`, `OpenAiChatClient`, `ensure_ollama_model`, `LlmTranslator`.
- Produces: `Cli` fields `translate_engine: Option<TranslateEngine>`, `ollama_model: Option<String>`, `ollama_url: Option<String>`, `api_base_url: Option<String>`, `api_model: Option<String>`, `profile: Option<String>`, `list_profiles: bool`; consts `DEFAULT_OLLAMA_MODEL`, `DEFAULT_OLLAMA_URL`, `DEFAULT_API_BASE_URL`, `DEFAULT_API_MODEL`; methods `Cli::api_base_url_or_default(&self) -> String`; `enum TranslatorChoice { Legacy(TranslateEngine), Profile(ProviderProfile) }`; `fn choose_translator(cli: &Cli, settings: Option<&Settings>) -> anyhow::Result<TranslatorChoice>`.

- [ ] **Step 1: Write the failing test** — replace `defaults_match_spec` in `tests/cli_test.rs` and append new tests:

```rust
#[test]
fn defaults_match_spec() {
    let c = Cli::try_parse_from(["auto-subtitle", "-i", "v.mp4"]).unwrap();
    assert_eq!(c.target_lang, "vi");
    assert_eq!(c.source_lang, "auto");
    assert_eq!(c.mode, OutputMode::Hardsub);
    assert_eq!(c.stt_engine, SttEngine::Local);
    assert_eq!(c.whisper_model, "small");
    assert_eq!(c.translate_engine, None);
    assert_eq!(c.ollama_model, None);
    assert_eq!(c.profile, None);
    assert!(!c.list_profiles);
    assert_eq!(c.api_base_url_or_default(), "https://api.openai.com/v1");
    assert!(!c.bilingual && !c.skip_translate && !c.keep_temp);
}
```

Append:

```rust
use auto_subtitle::settings::{ProviderKind, ProviderProfile, Settings};

fn settings_with_deepseek() -> Settings {
    let mut s = Settings::with_defaults();
    s.profiles.push(ProviderProfile {
        id: "ds-1".into(),
        name: "DeepSeek".into(),
        kind: ProviderKind::DeepSeek,
        base_url: None,
        model: "deepseek-chat".into(),
    });
    s
}

fn parse(args: &[&str]) -> Cli {
    let mut full = vec!["auto-subtitle", "-i", "v.mp4"];
    full.extend_from_slice(args);
    Cli::try_parse_from(full).unwrap()
}

#[test]
fn explicit_engine_takes_legacy_path_even_with_profiles() {
    let s = settings_with_deepseek();
    let c = parse(&["--translate-engine", "openai-compat", "--profile", "DeepSeek"]);
    assert!(matches!(
        choose_translator(&c, Some(&s)).unwrap(),
        TranslatorChoice::Legacy(TranslateEngine::OpenaiCompat)
    ));
}

#[test]
fn profile_by_name_or_id_and_default() {
    let s = settings_with_deepseek();
    for q in ["deepseek", "ds-1"] {
        match choose_translator(&parse(&["--profile", q]), Some(&s)).unwrap() {
            TranslatorChoice::Profile(p) => assert_eq!(p.id, "ds-1"),
            _ => panic!("expected profile"),
        }
    }
    match choose_translator(&parse(&[]), Some(&s)).unwrap() {
        TranslatorChoice::Profile(p) => assert_eq!(p.name, "Ollama local"),
        _ => panic!("expected default profile"),
    }
}

#[test]
fn unknown_profile_and_missing_settings() {
    let s = settings_with_deepseek();
    let err = choose_translator(&parse(&["--profile", "claude"]), Some(&s)).unwrap_err();
    assert!(err.to_string().contains("Không tìm thấy profile 'claude'"));
    assert!(choose_translator(&parse(&["--profile", "x"]), None).is_err());
    assert!(matches!(
        choose_translator(&parse(&[]), None).unwrap(),
        TranslatorChoice::Legacy(TranslateEngine::Ollama)
    ));
    let empty = Settings { profiles: vec![], default_profile_id: None, ..Settings::with_defaults() };
    assert!(matches!(
        choose_translator(&parse(&[]), Some(&empty)).unwrap(),
        TranslatorChoice::Legacy(TranslateEngine::Ollama)
    ));
}

#[test]
fn list_profiles_flag_parses() {
    assert!(parse(&["--list-profiles"]).list_profiles);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test cli_test`
Expected: compile errors (`no field profile`, `choose_translator` not found).

- [ ] **Step 3: Implement** — `src/cli.rs`

Replace the five fields `translate_engine`, `ollama_model`, `ollama_url`, `api_base_url`, `api_model` with:

```rust
    #[arg(long, value_enum)]
    pub translate_engine: Option<TranslateEngine>,

    #[arg(long)]
    pub ollama_model: Option<String>,

    #[arg(long)]
    pub ollama_url: Option<String>,

    #[arg(long, env = "OPENAI_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,

    #[arg(long)]
    pub api_base_url: Option<String>,

    #[arg(long)]
    pub api_model: Option<String>,

    /// Provider profile name or id from settings.json
    #[arg(long)]
    pub profile: Option<String>,

    /// Print configured provider profiles and exit
    #[arg(long)]
    pub list_profiles: bool,
```

(The existing `api_key` field moves into this block; delete its old declaration.)

Append to `src/cli.rs`:

```rust
use crate::settings::{ProviderProfile, Settings};

pub const DEFAULT_OLLAMA_MODEL: &str = "qwen2.5:7b";
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";
pub const DEFAULT_API_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_API_MODEL: &str = "gpt-4o-mini";

impl Cli {
    pub fn api_base_url_or_default(&self) -> String {
        self.api_base_url.clone().unwrap_or_else(|| DEFAULT_API_BASE_URL.to_string())
    }
}

#[derive(Debug, Clone)]
pub enum TranslatorChoice {
    Legacy(TranslateEngine),
    Profile(ProviderProfile),
}

pub fn choose_translator(cli: &Cli, settings: Option<&Settings>) -> anyhow::Result<TranslatorChoice> {
    if let Some(engine) = cli.translate_engine {
        return Ok(TranslatorChoice::Legacy(engine));
    }
    match (cli.profile.as_deref(), settings) {
        (Some(q), Some(s)) => s.find_profile(q).cloned().map(TranslatorChoice::Profile).ok_or_else(|| {
            let names: Vec<&str> = s.profiles.iter().map(|p| p.name.as_str()).collect();
            anyhow::anyhow!("Không tìm thấy profile '{q}'. Có: {}", names.join(", "))
        }),
        (Some(_), None) => anyhow::bail!("Không đọc được settings.json nên không dùng được --profile"),
        (None, Some(s)) => Ok(s
            .default_profile()
            .cloned()
            .map(TranslatorChoice::Profile)
            .unwrap_or(TranslatorChoice::Legacy(TranslateEngine::Ollama))),
        (None, None) => Ok(TranslatorChoice::Legacy(TranslateEngine::Ollama)),
    }
}
```

`src/main.rs` — full replacement:

```rust
use anyhow::Context;
use clap::Parser;

use auto_subtitle::cli::{
    choose_translator, Cli, OutputMode, SttEngine, TranslateEngine, TranslatorChoice,
    DEFAULT_API_MODEL, DEFAULT_OLLAMA_MODEL, DEFAULT_OLLAMA_URL,
};
use auto_subtitle::config::AppConfig;
use auto_subtitle::downloader::DependencyManager;
use auto_subtitle::llm::ollama::{ensure_ollama_model, OllamaChatClient};
use auto_subtitle::llm::openai::OpenAiChatClient;
use auto_subtitle::llm::{build_chat_client, ChatClient};
use auto_subtitle::pipeline::{self, Deps};
use auto_subtitle::settings::{ProviderKind, SettingsStore};
use auto_subtitle::stt::whisper_api::ApiTranscriber;
use auto_subtitle::stt::whisper_local::LocalWhisperTranscriber;
use auto_subtitle::stt::Transcriber;
use auto_subtitle::translate::{LlmTranslator, Translator};

fn print_profiles() -> anyhow::Result<()> {
    let store = SettingsStore::open_default()?;
    let s = store.settings();
    if s.profiles.is_empty() {
        println!("(chưa có profile nào)");
    }
    for p in &s.profiles {
        let default = if s.default_profile_id.as_deref() == Some(p.id.as_str()) { "default" } else { "" };
        println!("{} | {} | {} | {}", p.name, p.kind.as_str(), p.model, default);
    }
    Ok(())
}

async fn build_translator(
    cli: &Cli,
    store: Option<&SettingsStore>,
) -> anyhow::Result<Box<dyn Translator>> {
    match choose_translator(cli, store.map(|s| s.settings()))? {
        TranslatorChoice::Legacy(TranslateEngine::Ollama) => {
            let url = cli.ollama_url.clone().unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string());
            let model = cli.ollama_model.clone().unwrap_or_else(|| DEFAULT_OLLAMA_MODEL.to_string());
            let client = OllamaChatClient::new(url.clone(), model.clone());
            ensure_ollama_model(&client, &url, &model).await?;
            Ok(Box::new(LlmTranslator::new(Box::new(client), None)))
        }
        TranslatorChoice::Legacy(TranslateEngine::OpenaiCompat) => {
            let api_key = cli
                .api_key
                .clone()
                .context("--api-key or OPENAI_API_KEY required for translation")?;
            let model = cli.api_model.clone().unwrap_or_else(|| DEFAULT_API_MODEL.to_string());
            let client = OpenAiChatClient::new(cli.api_base_url_or_default(), Some(api_key), model);
            Ok(Box::new(LlmTranslator::new(Box::new(client), None)))
        }
        TranslatorChoice::Profile(profile) => {
            let key = match store {
                Some(s) => s.profile_key(&profile.id)?,
                None => None,
            };
            let client: Box<dyn ChatClient> =
                build_chat_client(&profile, key).map_err(anyhow::Error::new)?;
            if profile.kind == ProviderKind::Ollama {
                let base = profile.effective_base_url().unwrap_or_default();
                ensure_ollama_model(client.as_ref(), &base, &profile.model).await?;
            }
            Ok(Box::new(LlmTranslator::new(client, None)))
        }
    }
}

async fn run_app() -> anyhow::Result<()> {
    if std::env::args().skip(1).any(|a| a == "--list-profiles") {
        return print_profiles();
    }
    let cli = Cli::parse();
    let config = AppConfig::load().context("failed to load application configuration")?;
    let dep_mgr = DependencyManager::new(config);
    let store = match SettingsStore::open_default() {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("Cảnh báo: không đọc được cài đặt ({e:#}); dùng giá trị mặc định.");
            None
        }
    };

    let ffmpeg = if cli.mode == OutputMode::SubsOnly && cli.subtitle.is_some() {
        None
    } else {
        Some(
            dep_mgr
                .find_or_download_ffmpeg()
                .await
                .context("failed to find or download ffmpeg")?,
        )
    };

    let transcriber_box: Option<Box<dyn Transcriber>> = if cli.subtitle.is_some() {
        None
    } else {
        let lang_opt = if cli.source_lang == "auto" {
            None
        } else {
            Some(cli.source_lang.clone())
        };

        match cli.stt_engine {
            SttEngine::Local => {
                let model_path = dep_mgr
                    .ensure_whisper_model(&cli.whisper_model)
                    .await
                    .with_context(|| {
                        format!("failed to ensure whisper model '{}'", cli.whisper_model)
                    })?;
                Some(Box::new(LocalWhisperTranscriber::new(
                    model_path, lang_opt, true,
                )))
            }
            SttEngine::Api => {
                let stored = store.as_ref().and_then(|s| s.stt_key().ok().flatten());
                let api_key = cli
                    .api_key
                    .clone()
                    .or(stored)
                    .context("--api-key or OPENAI_API_KEY required for STT API")?;
                Some(Box::new(ApiTranscriber::new(
                    api_key,
                    cli.api_base_url_or_default(),
                    cli.stt_api_model.clone(),
                    lang_opt,
                )))
            }
        }
    };

    let translator_box: Option<Box<dyn Translator>> = if cli.skip_translate {
        None
    } else {
        Some(build_translator(&cli, store.as_ref()).await?)
    };

    let deps = Deps {
        ffmpeg,
        transcriber: transcriber_box.as_deref(),
        translator: translator_box.as_deref(),
    };

    let report = pipeline::run(&cli, deps).await?;

    println!("Processed {} subtitle segments.", report.segments);
    for output in report.outputs {
        println!("Output: {}", output.display());
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run_app().await {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}
```

If `pipeline.rs` or other tests reference `cli.api_base_url` / `cli.ollama_model` as `String`, fix with `cli.api_base_url_or_default()` / `.unwrap_or(...)`. Check with:

Run: `cargo build --bin auto-subtitle 2>&1 | grep -E "^error" -A5`
Expected: no output.

- [ ] **Step 4: Run tests**

Run: `cargo test -p auto-subtitle`
Expected: all pass (except tests needing ffmpeg/network that were already environment-dependent).

Manual: `cargo run --bin auto-subtitle -- --list-profiles`
Expected: `Ollama local | ollama | qwen2.5:7b | default`

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/main.rs tests/cli_test.rs
git commit -m "feat(cli): --profile and --list-profiles using shared settings"
```

---

### Task 9: Tauri settings commands and pipeline wiring

**Files:**
- Create: `src-tauri/src/settings_commands.rs`
- Modify: `src-tauri/src/state.rs`, `src-tauri/src/main.rs`, `src-tauri/src/types.rs`, `src-tauri/src/commands.rs`
- Modify: `.github/workflows/release.yml` (add `libdbus-1-dev`)
- Test: `src-tauri/src/settings_commands.rs` (`#[cfg(test)]` module)

**Interfaces:**
- Consumes: `SettingsStore`, `build_chat_client`, `test_connection`, `TEST_TIMEOUT`, `ConnectionTest`, `LlmTranslator`, `ApiTranscriber`.
- Produces (IPC contract used by Task 10):
  - `get_settings() -> SettingsView`
  - `save_profile(profile: ProviderProfile, apiKey: Option<String>) -> SettingsView`
  - `delete_profile(id: String) -> SettingsView`
  - `set_default_profile(id: String) -> SettingsView`
  - `save_general(general: GeneralSettings) -> SettingsView`
  - `set_stt_key(key: String) -> SettingsView`
  - `test_profile(profile: ProviderProfile, apiKey: Option<String>) -> ConnectionTest`
  - `list_profile_models(profile: ProviderProfile, apiKey: Option<String>) -> Vec<String>`
  - `SettingsView { profiles: Vec<ProfileView>, default_profile_id, general, stt_has_key, secrets_backend, load_warning }`, `ProfileView { ...ProviderProfile (flattened), has_key }`
  - `PipelineConfig { job_id, video_path, subtitle_path, whisper_model, use_cloud_stt, target_lang, skip_translate, profile_id: Option<String> }`

- [ ] **Step 1: Write the failing test** — create `src-tauri/src/settings_commands.rs` with only the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use auto_subtitle::settings::secrets::{MemoryStore, SecretsBackend};
    use auto_subtitle::settings::{ProviderKind, ProviderProfile, SettingsStore};

    #[test]
    fn view_exposes_has_key_but_never_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let mut store =
            SettingsStore::open(dir.path(), Box::new(MemoryStore::default()), SecretsBackend::File).unwrap();
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
        let mut store =
            SettingsStore::open(dir.path(), Box::new(MemoryStore::default()), SecretsBackend::Keyring).unwrap();
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
        assert_eq!(resolve_key(&store, &p.id, Some("typed".into())).as_deref(), Some("typed"));
        assert_eq!(resolve_key(&store, &p.id, Some("  ".into())).as_deref(), Some("stored"));
        assert_eq!(resolve_key(&store, &p.id, None).as_deref(), Some("stored"));
        assert_eq!(resolve_key(&store, "unknown", None), None);
    }
}
```

Add `tempfile = "3.20"` already exists in `src-tauri/Cargo.toml` `[dependencies]` — no change needed.

Add to `src-tauri/src/main.rs` module list: `pub mod settings_commands;`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p auto-subtitle-desktop settings_commands`
Expected: compile errors (`build_view`, `resolve_key` not found; plus `commands.rs` errors from removed translators — fixed in Step 3).

- [ ] **Step 3: Implement**

`src-tauri/src/types.rs` — add imports at top:

```rust
use auto_subtitle::settings::{GeneralSettings, ProviderProfile, SecretsBackend};
```

Replace the `PipelineConfig` struct with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineConfig {
    #[serde(default)]
    pub job_id: Option<String>,
    pub video_path: String,
    pub subtitle_path: Option<String>,
    pub whisper_model: String,
    pub use_cloud_stt: bool,
    pub target_lang: String,
    pub skip_translate: bool,
    #[serde(default)]
    pub profile_id: Option<String>,
}
```

Append (before `#[cfg(test)]`):

```rust
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProfileView {
    #[serde(flatten)]
    pub profile: ProviderProfile,
    pub has_key: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SettingsView {
    pub profiles: Vec<ProfileView>,
    pub default_profile_id: Option<String>,
    pub general: GeneralSettings,
    pub stt_has_key: bool,
    pub secrets_backend: SecretsBackend,
    pub load_warning: Option<String>,
}
```

`src-tauri/src/state.rs` — full replacement:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

use auto_subtitle::settings::SettingsStore;

pub struct AppState {
    pub cancel_senders: Arc<Mutex<HashMap<String, broadcast::Sender<()>>>>,
    pub settings: Arc<std::sync::Mutex<SettingsStore>>,
}

impl AppState {
    pub fn new(settings: SettingsStore) -> Self {
        Self {
            cancel_senders: Arc::new(Mutex::new(HashMap::new())),
            settings: Arc::new(std::sync::Mutex::new(settings)),
        }
    }
}
```

If the user's committed `state.rs` has additional fields, keep them and only add `settings` + the constructor parameter.

`src-tauri/src/settings_commands.rs` — put this above the test module:

```rust
use tauri::State;

use auto_subtitle::llm::{build_chat_client, test_connection, ConnectionTest, TEST_TIMEOUT};
use auto_subtitle::settings::{GeneralSettings, ProviderProfile, SettingsStore};

use crate::state::AppState;
use crate::types::{ProfileView, SettingsView};

fn lock(state: &AppState) -> Result<std::sync::MutexGuard<'_, SettingsStore>, String> {
    state.settings.lock().map_err(|_| "Lỗi khóa cài đặt".to_string())
}

pub(crate) fn build_view(store: &mut SettingsStore) -> SettingsView {
    let s = store.settings().clone();
    SettingsView {
        profiles: s
            .profiles
            .into_iter()
            .map(|p| ProfileView { has_key: store.has_profile_key(&p.id), profile: p })
            .collect(),
        default_profile_id: s.default_profile_id,
        general: s.general,
        stt_has_key: matches!(store.stt_key(), Ok(Some(_))),
        secrets_backend: store.backend(),
        load_warning: store.take_load_warning(),
    }
}

pub(crate) fn resolve_key(store: &SettingsStore, id: &str, typed: Option<String>) -> Option<String> {
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
        Err(e) => return Ok(ConnectionTest { ok: false, latency_ms: 0, error: Some(e.user_message()) }),
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
```

`src-tauri/src/main.rs` — change the `AppState` construction and register commands:

```rust
pub mod commands;
pub mod settings_commands;
pub mod state;
pub mod types;

use commands::*;
use settings_commands::*;
use state::AppState;

fn main() {
    let settings = auto_subtitle::settings::SettingsStore::open_default()
        .expect("Không mở được thư mục cài đặt");
    tauri::Builder::default()
        .manage(AppState::new(settings))
        .invoke_handler(tauri::generate_handler![
            get_system_info,
            download_whisper_model,
            start_pipeline,
            cancel_pipeline,
            export_subtitles,
            open_in_file_manager,
            pick_video_file,
            pick_video_files,
            get_output_dir,
            pick_save_file,
            get_settings,
            save_profile,
            delete_profile,
            set_default_profile,
            save_general,
            set_stt_key,
            test_profile,
            list_profile_models
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Keep any other builder calls (plugins, setup) the user's committed `main.rs` has; only change `.manage(...)` and append the eight command names.

`src-tauri/src/commands.rs`:

1. Replace imports

```rust
use auto_subtitle::translate::ollama::OllamaTranslator;
use auto_subtitle::translate::openai_compat::OpenAiCompatTranslator;
use auto_subtitle::translate::Translator;
```

with

```rust
use auto_subtitle::llm::build_chat_client;
use auto_subtitle::settings::SettingsStore;
use auto_subtitle::stt::whisper_api::ApiTranscriber;
use auto_subtitle::translate::{LlmTranslator, Translator};
```

2. In `start_pipeline`, change the call

```rust
    let result = run_pipeline(app, config, job_id.clone(), cancel_rx).await;
```

to

```rust
    let result = run_pipeline(app, state.settings.clone(), config, job_id.clone(), cancel_rx).await;
```

3. Change the `run_pipeline` signature to:

```rust
async fn run_pipeline(
    app: AppHandle,
    settings: std::sync::Arc<std::sync::Mutex<SettingsStore>>,
    config: PipelineConfig,
    job_id: String,
    mut cancel_rx: broadcast::Receiver<()>,
) -> Result<Vec<SubtitleSegment>, String> {
```

4. Immediately after the `if !input_video.is_file() { ... }` block, insert:

```rust
    let (translator, stt_key): (Option<Box<dyn Translator>>, Option<String>) = {
        let store = settings.lock().map_err(|_| "Lỗi khóa cài đặt".to_string())?;
        let translator = if config.skip_translate {
            None
        } else {
            let profile = store
                .resolve_profile(config.profile_id.as_deref())
                .map_err(|e| format!("{e:#}"))?
                .ok_or_else(|| "Chưa cấu hình provider dịch".to_string())?;
            let key = store.profile_key(&profile.id).map_err(|e| format!("{e:#}"))?;
            let client = build_chat_client(&profile, key).map_err(|e| e.user_message())?;
            Some(Box::new(LlmTranslator::new(client, None)) as Box<dyn Translator>)
        };
        let stt_key = if config.use_cloud_stt {
            Some(
                store
                    .stt_key()
                    .map_err(|e| format!("{e:#}"))?
                    .ok_or_else(|| "Chưa nhập API key Whisper".to_string())?,
            )
        } else {
            None
        };
        (translator, stt_key)
    };
```

5. Replace the audio extraction lines

```rust
    let wav_path = temp_dir.path().join("audio_16k.wav");

    let extract_cmd = media::build_extract_wav_command(&ffmpeg, &input_video, &wav_path);
```

with

```rust
    let audio_path = if stt_key.is_some() {
        temp_dir.path().join("audio.mp3")
    } else {
        temp_dir.path().join("audio_16k.wav")
    };

    let extract_cmd = if stt_key.is_some() {
        media::build_extract_mp3_command(&ffmpeg, &input_video, &audio_path)
    } else {
        media::build_extract_wav_command(&ffmpeg, &input_video, &audio_path)
    };
```

6. Replace the block from `let model_path = dm` (the `ensure_whisper_model` call) through `let transcriber = LocalWhisperTranscriber::new(model_path, None, true);` with:

```rust
    let stt_label = if stt_key.is_some() {
        "Whisper API".to_string()
    } else {
        format!("Whisper ({})", config.whisper_model)
    };

    let transcriber: Box<dyn Transcriber> = match stt_key {
        Some(key) => Box::new(ApiTranscriber::new(
            key,
            "https://api.openai.com/v1".to_string(),
            "whisper-1".to_string(),
            None,
        )),
        None => {
            let model_path = dm
                .ensure_whisper_model(&config.whisper_model)
                .await
                .map_err(|e| format!("Lỗi tải model Whisper: {e:#}"))?;
            Box::new(LocalWhisperTranscriber::new(model_path, None, true))
        }
    };

    emit_progress(
        &app,
        &job_id,
        start_time,
        "stt",
        &format!("Đang nhận dạng âm thanh với {stt_label}..."),
        18.0,
        None,
        None,
    );
```

(This replaces the previous `emit_progress(... 18.0 ...)` call that sat between `ensure_whisper_model` and `LocalWhisperTranscriber::new` — delete that old call.)

7. In the ticker, replace `let model_name = config.whisper_model.clone();` with `let model_name = stt_label.clone();` and its message `&format!("Đang nhận dạng âm thanh với Whisper ({})...", model_name)` with `&format!("Đang nhận dạng âm thanh với {}...", model_name)`.

8. Replace `res = transcriber.transcribe(&wav_path) => Some(res),` with `res = transcriber.transcribe(&audio_path) => Some(res),`.

9. In Phase 3, replace the whole `let translator: Box<dyn Translator> = if config.use_cloud_llm { ... } else { ... };` block with:

```rust
        let translator = translator.expect("translator resolved when translation enabled");
```

`.github/workflows/release.yml` — in the "Install Linux dependencies" step add `libdbus-1-dev \` after `libssl-dev \` … i.e. the list becomes:

```yaml
          sudo apt-get install -y \
            libwebkit2gtk-4.1-dev \
            libappindicator3-dev \
            librsvg2-dev \
            patchelf \
            cmake \
            libasound2-dev \
            libssl-dev \
            libdbus-1-dev
```

- [ ] **Step 4: Run tests and build**

Run: `cargo test -p auto-subtitle-desktop`
Expected: all pass, including the 2 new `settings_commands` tests and the existing `types.rs` tests.

Run: `cargo build -p auto-subtitle-desktop`
Expected: success, no warnings about unused `OllamaTranslator`.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src .github/workflows/release.yml
git commit -m "feat(desktop): settings commands; pipeline uses profiles and cloud STT key"
```

---

### Task 10: Frontend types, provider helpers, IPC wrappers

**Files:**
- Modify: `ui/src/types/index.ts`, `ui/src/lib/ipc.ts`, `ui/src/lib/ipc.test.ts`
- Create: `ui/src/lib/providers.ts`

**Interfaces:**
- Consumes: IPC contract from Task 9.
- Produces (TS):
  - Types `ProviderKind`, `ProviderProfile`, `ProfileView`, `GeneralSettings`, `SettingsView`, `ConnectionTest`; updated `PipelineConfig`.
  - `providers.ts`: `PROVIDER_KINDS: ProviderKind[]`, `KIND_LABELS: Record<ProviderKind, string>`, `KIND_ICONS: Record<ProviderKind, string>`, `defaultBaseUrl(kind): string | null`, `requiresBaseUrl(kind): boolean`, `requiresApiKey(kind): boolean`, `emptyProfile(): ProviderProfile`.
  - `ipc.ts`: `getSettings()`, `saveProfile(profile, apiKey: string | null)`, `deleteProfile(id)`, `setDefaultProfile(id)`, `saveGeneral(general)`, `setSttKey(key)` → `Promise<SettingsView>`; `testProfile(profile, apiKey: string | null): Promise<ConnectionTest>`; `listProfileModels(profile, apiKey: string | null): Promise<string[]>`.

- [ ] **Step 1: Write the failing test** — append to `ui/src/lib/ipc.test.ts`:

```ts
import { invoke } from '@tauri-apps/api/core';
import { getSettings, saveProfile, testProfile, listProfileModels, saveGeneral } from './ipc';
import { defaultBaseUrl, requiresBaseUrl, requiresApiKey, emptyProfile, KIND_LABELS } from './providers';

describe('settings IPC', () => {
  it('passes camelCase apiKey and null to keep the stored key', async () => {
    const profile = { id: 'p1', name: 'DS', kind: 'deepseek' as const, base_url: null, model: 'deepseek-chat' };
    await saveProfile(profile, null);
    expect(invoke).toHaveBeenCalledWith('save_profile', { profile, apiKey: null });
    await testProfile(profile, 'sk');
    expect(invoke).toHaveBeenCalledWith('test_profile', { profile, apiKey: 'sk' });
    await listProfileModels(profile, null);
    expect(invoke).toHaveBeenCalledWith('list_profile_models', { profile, apiKey: null });
  });

  it('calls get_settings and save_general', async () => {
    await getSettings();
    expect(invoke).toHaveBeenCalledWith('get_settings');
    const general = { whisper_model: 'base', target_lang: 'vi', bilingual: true, style_preset: 'youtube_yellow' as const, use_cloud_stt: false };
    await saveGeneral(general);
    expect(invoke).toHaveBeenCalledWith('save_general', { general });
  });
});

describe('provider helpers', () => {
  it('knows defaults and requirements', () => {
    expect(defaultBaseUrl('deepseek')).toBe('https://api.deepseek.com/v1');
    expect(defaultBaseUrl('openai_compatible')).toBeNull();
    expect(requiresBaseUrl('anthropic_compatible')).toBe(true);
    expect(requiresApiKey('ollama')).toBe(false);
    expect(KIND_LABELS.anthropic).toBe('Anthropic');
    expect(emptyProfile()).toEqual({ id: '', name: '', kind: 'openai', base_url: null, model: '' });
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix ui test -- src/lib/ipc.test.ts`
Expected: FAIL — `getSettings` / `./providers` not found.

- [ ] **Step 3: Implement**

`ui/src/types/index.ts` — replace the `PipelineConfig` interface with:

```ts
export interface PipelineConfig {
  job_id?: string;
  video_path: string;
  subtitle_path?: string | null;
  whisper_model: string;
  use_cloud_stt: boolean;
  target_lang: string;
  skip_translate: boolean;
  profile_id?: string | null;
}
```

Append:

```ts
export type ProviderKind =
  | 'openai'
  | 'openai_compatible'
  | 'anthropic'
  | 'anthropic_compatible'
  | 'deepseek'
  | 'ollama';

export interface ProviderProfile {
  id: string;
  name: string;
  kind: ProviderKind;
  base_url: string | null;
  model: string;
}

export interface ProfileView extends ProviderProfile {
  has_key: boolean;
}

export interface GeneralSettings {
  whisper_model: string;
  target_lang: string;
  bilingual: boolean;
  style_preset: StylePreset;
  use_cloud_stt: boolean;
}

export interface SettingsView {
  profiles: ProfileView[];
  default_profile_id: string | null;
  general: GeneralSettings;
  stt_has_key: boolean;
  secrets_backend: 'keyring' | 'file';
  load_warning: string | null;
}

export interface ConnectionTest {
  ok: boolean;
  latency_ms: number;
  error: string | null;
}
```

`ui/src/lib/providers.ts`:

```ts
import type { ProviderKind, ProviderProfile } from '../types';

export const PROVIDER_KINDS: ProviderKind[] = [
  'openai',
  'openai_compatible',
  'anthropic',
  'anthropic_compatible',
  'deepseek',
  'ollama',
];

export const KIND_LABELS: Record<ProviderKind, string> = {
  openai: 'OpenAI',
  openai_compatible: 'OpenAI Compatible',
  anthropic: 'Anthropic',
  anthropic_compatible: 'Anthropic Compatible',
  deepseek: 'DeepSeek',
  ollama: 'Ollama (local)',
};

export const KIND_ICONS: Record<ProviderKind, string> = {
  openai: '◎',
  openai_compatible: '◎',
  anthropic: '✳',
  anthropic_compatible: '✳',
  deepseek: '◆',
  ollama: '🦙',
};

const DEFAULT_BASE_URLS: Record<ProviderKind, string | null> = {
  openai: 'https://api.openai.com/v1',
  openai_compatible: null,
  anthropic: 'https://api.anthropic.com',
  anthropic_compatible: null,
  deepseek: 'https://api.deepseek.com/v1',
  ollama: 'http://localhost:11434',
};

export const defaultBaseUrl = (kind: ProviderKind): string | null => DEFAULT_BASE_URLS[kind];
export const requiresBaseUrl = (kind: ProviderKind): boolean => DEFAULT_BASE_URLS[kind] === null;
export const requiresApiKey = (kind: ProviderKind): boolean => kind !== 'ollama';

export const emptyProfile = (): ProviderProfile => ({
  id: '',
  name: '',
  kind: 'openai',
  base_url: null,
  model: '',
});
```

`ui/src/lib/ipc.ts` — extend the type import with `SettingsView, ProviderProfile, GeneralSettings, ConnectionTest`, then append:

```ts
let mockSettings: SettingsView = {
  profiles: [
    { id: 'mock-ollama', name: 'Ollama local', kind: 'ollama', base_url: null, model: 'qwen2.5:7b', has_key: false },
  ],
  default_profile_id: 'mock-ollama',
  general: {
    whisper_model: 'base',
    target_lang: 'vi',
    bilingual: true,
    style_preset: 'youtube_yellow',
    use_cloud_stt: false,
  },
  stt_has_key: false,
  secrets_backend: 'keyring',
  load_warning: null,
};

const cloneMock = (): SettingsView => JSON.parse(JSON.stringify(mockSettings));

export async function getSettings(): Promise<SettingsView> {
  if (!isTauri()) return cloneMock();
  return await invoke<SettingsView>('get_settings');
}

export async function saveProfile(profile: ProviderProfile, apiKey: string | null): Promise<SettingsView> {
  if (!isTauri()) {
    const id = profile.id || `mock-${Date.now()}`;
    const existing = mockSettings.profiles.find((p) => p.id === id);
    const hasKey = apiKey === null ? existing?.has_key ?? false : apiKey.trim() !== '';
    const next = { ...profile, id, has_key: hasKey };
    mockSettings.profiles = existing
      ? mockSettings.profiles.map((p) => (p.id === id ? next : p))
      : [...mockSettings.profiles, next];
    mockSettings.default_profile_id ??= id;
    return cloneMock();
  }
  return await invoke<SettingsView>('save_profile', { profile, apiKey });
}

export async function deleteProfile(id: string): Promise<SettingsView> {
  if (!isTauri()) {
    mockSettings.profiles = mockSettings.profiles.filter((p) => p.id !== id);
    if (mockSettings.default_profile_id === id) {
      mockSettings.default_profile_id = mockSettings.profiles[0]?.id ?? null;
    }
    return cloneMock();
  }
  return await invoke<SettingsView>('delete_profile', { id });
}

export async function setDefaultProfile(id: string): Promise<SettingsView> {
  if (!isTauri()) {
    mockSettings.default_profile_id = id;
    return cloneMock();
  }
  return await invoke<SettingsView>('set_default_profile', { id });
}

export async function saveGeneral(general: GeneralSettings): Promise<SettingsView> {
  if (!isTauri()) {
    mockSettings.general = { ...general };
    return cloneMock();
  }
  return await invoke<SettingsView>('save_general', { general });
}

export async function setSttKey(key: string): Promise<SettingsView> {
  if (!isTauri()) {
    mockSettings.stt_has_key = key.trim() !== '';
    return cloneMock();
  }
  return await invoke<SettingsView>('set_stt_key', { key });
}

export async function testProfile(profile: ProviderProfile, apiKey: string | null): Promise<ConnectionTest> {
  if (!isTauri()) {
    await new Promise((r) => setTimeout(r, 300));
    return { ok: true, latency_ms: 300, error: null };
  }
  return await invoke<ConnectionTest>('test_profile', { profile, apiKey });
}

export async function listProfileModels(profile: ProviderProfile, apiKey: string | null): Promise<string[]> {
  if (!isTauri()) return ['qwen2.5:7b', 'llama3:8b'];
  return await invoke<string[]>('list_profile_models', { profile, apiKey });
}
```

- [ ] **Step 4: Run tests and typecheck**

Run: `npm --prefix ui test -- src/lib/ipc.test.ts`
Expected: PASS.

Run: `(cd ui && npx tsc --noEmit)`
Expected: errors **only** in `App.tsx` about `use_cloud_llm`/`llm_model` (fixed in Task 12).

- [ ] **Step 5: Commit**

```bash
git add ui/src/types/index.ts ui/src/lib/ipc.ts ui/src/lib/ipc.test.ts ui/src/lib/providers.ts
git commit -m "feat(ui): settings types, provider helpers and IPC wrappers"
```

---

### Task 11: SettingsModal

**Files:**
- Create: `ui/src/components/SettingsModal.tsx`, `ui/src/components/SettingsModal.test.tsx`

**Interfaces:**
- Consumes: Task 10 types, `providers.ts`, IPC functions.
- Produces: `SettingsModal` component and `SettingsApi` type:

```ts
export interface SettingsApi {
  saveProfile: (p: ProviderProfile, apiKey: string | null) => Promise<SettingsView>;
  deleteProfile: (id: string) => Promise<SettingsView>;
  setDefaultProfile: (id: string) => Promise<SettingsView>;
  testProfile: (p: ProviderProfile, apiKey: string | null) => Promise<ConnectionTest>;
  listProfileModels: (p: ProviderProfile, apiKey: string | null) => Promise<string[]>;
  setSttKey: (key: string) => Promise<SettingsView>;
}
interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  settings: SettingsView;
  onSettingsChange: (s: SettingsView) => void;
  general: GeneralSettings;
  onGeneralChange: (g: GeneralSettings) => void;
  whisperModels: WhisperModelInfo[];
  api?: SettingsApi; // defaults to the real ipc functions
}
```

- [ ] **Step 1: Write the failing test** — `ui/src/components/SettingsModal.test.tsx`

```tsx
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
import { SettingsModal, type SettingsApi } from './SettingsModal';
import type { SettingsView, GeneralSettings } from '../types';

const general: GeneralSettings = {
  whisper_model: 'base',
  target_lang: 'vi',
  bilingual: true,
  style_preset: 'youtube_yellow',
  use_cloud_stt: false,
};

const view = (over: Partial<SettingsView> = {}): SettingsView => ({
  profiles: [
    { id: 'o', name: 'Ollama local', kind: 'ollama', base_url: null, model: 'qwen2.5:7b', has_key: false },
    { id: 'd', name: 'DeepSeek', kind: 'deepseek', base_url: null, model: 'deepseek-chat', has_key: true },
  ],
  default_profile_id: 'o',
  general,
  stt_has_key: false,
  secrets_backend: 'keyring',
  load_warning: null,
  ...over,
});

let api: SettingsApi;
beforeEach(() => {
  api = {
    saveProfile: vi.fn().mockResolvedValue(view()),
    deleteProfile: vi.fn().mockResolvedValue(view()),
    setDefaultProfile: vi.fn().mockResolvedValue(view()),
    testProfile: vi.fn().mockResolvedValue({ ok: true, latency_ms: 123, error: null }),
    listProfileModels: vi.fn().mockResolvedValue(['deepseek-chat', 'deepseek-reasoner']),
    setSttKey: vi.fn().mockResolvedValue(view()),
  };
});

const renderModal = (settings = view(), onSettingsChange = vi.fn()) =>
  render(
    <SettingsModal
      isOpen
      onClose={vi.fn()}
      settings={settings}
      onSettingsChange={onSettingsChange}
      general={general}
      onGeneralChange={vi.fn()}
      whisperModels={[{ name: 'base', downloaded: true, size_mb: 142 }]}
      api={api}
    />
  );

// Profile names also appear as <option> labels in the Kind select, so target the list buttons.
const pickProfile = (name: string) =>
  fireEvent.click(screen.getByRole('button', { name: new RegExp(name) }));

describe('SettingsModal', () => {
  it('lists profiles with default star and no keychain banner', () => {
    renderModal();
    expect(screen.getByRole('button', { name: /★ Ollama local/ })).toBeDefined();
    expect(screen.getByRole('button', { name: /DeepSeek/ })).toBeDefined();
    expect(screen.queryByText(/Không tìm thấy keychain/)).toBeNull();
  });

  it('shows keychain banner for file backend', () => {
    renderModal(view({ secrets_backend: 'file' }));
    expect(screen.getByText(/Không tìm thấy keychain hệ thống/)).toBeDefined();
  });

  it('saving with empty key keeps stored key (null)', async () => {
    const onChange = vi.fn();
    renderModal(view(), onChange);
    pickProfile('DeepSeek');
    expect(screen.getByPlaceholderText('••• đã lưu')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Lưu' }));
    await waitFor(() =>
      expect(api.saveProfile).toHaveBeenCalledWith(
        { id: 'd', name: 'DeepSeek', kind: 'deepseek', base_url: null, model: 'deepseek-chat' },
        null
      )
    );
    expect(onChange).toHaveBeenCalled();
  });

  it('typed key is sent; Xóa key sends empty string', async () => {
    renderModal();
    pickProfile('DeepSeek');
    fireEvent.change(screen.getByLabelText('API key'), { target: { value: 'sk-new' } });
    fireEvent.click(screen.getByRole('button', { name: 'Lưu' }));
    await waitFor(() => expect(api.saveProfile).toHaveBeenLastCalledWith(expect.objectContaining({ id: 'd' }), 'sk-new'));
    fireEvent.click(screen.getByRole('button', { name: 'Xóa key' }));
    await waitFor(() => expect(api.saveProfile).toHaveBeenLastCalledWith(expect.objectContaining({ id: 'd' }), ''));
  });

  it('test connection shows success and failure', async () => {
    renderModal();
    pickProfile('DeepSeek');
    fireEvent.click(screen.getByRole('button', { name: 'Test kết nối' }));
    expect(await screen.findByText('✅ Kết nối OK (123 ms)')).toBeDefined();
    (api.testProfile as any).mockResolvedValueOnce({ ok: false, latency_ms: 5, error: 'API key không hợp lệ' });
    fireEvent.click(screen.getByRole('button', { name: 'Test kết nối' }));
    expect(await screen.findByText('❌ API key không hợp lệ')).toBeDefined();
  });

  it('new profile is saved with empty id; delete asks confirmation', async () => {
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    renderModal();
    fireEvent.click(screen.getByRole('button', { name: '+ Thêm' }));
    fireEvent.change(screen.getByLabelText('Tên'), { target: { value: 'Claude' } });
    fireEvent.change(screen.getByLabelText('Loại'), { target: { value: 'anthropic' } });
    fireEvent.change(screen.getByLabelText('Model'), { target: { value: 'claude-x' } });
    fireEvent.click(screen.getByRole('button', { name: 'Lưu' }));
    await waitFor(() =>
      expect(api.saveProfile).toHaveBeenCalledWith(
        { id: '', name: 'Claude', kind: 'anthropic', base_url: null, model: 'claude-x' },
        null
      )
    );

    pickProfile('DeepSeek');
    fireEvent.click(screen.getByRole('button', { name: 'Xóa' }));
    await waitFor(() => expect(api.deleteProfile).toHaveBeenCalledWith('d'));
  });

  it('model suggestions are fetched for the selected profile', async () => {
    renderModal();
    pickProfile('DeepSeek');
    await waitFor(() => expect(api.listProfileModels).toHaveBeenCalled());
    await waitFor(() =>
      expect(document.querySelector('datalist#model-suggestions option[value="deepseek-reasoner"]')).not.toBeNull()
    );
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm --prefix ui test -- src/components/SettingsModal.test.tsx`
Expected: FAIL — cannot resolve `./SettingsModal`.

- [ ] **Step 3: Implement** — `ui/src/components/SettingsModal.tsx`

```tsx
import React, { useEffect, useState } from 'react';
import { X, RefreshCw, AlertTriangle } from 'lucide-react';
import type {
  ConnectionTest,
  GeneralSettings,
  ProfileView,
  ProviderProfile,
  SettingsView,
  StylePreset,
  WhisperModelInfo,
} from '../types';
import * as ipc from '../lib/ipc';
import {
  KIND_ICONS,
  KIND_LABELS,
  PROVIDER_KINDS,
  defaultBaseUrl,
  emptyProfile,
  requiresApiKey,
} from '../lib/providers';

export interface SettingsApi {
  saveProfile: (p: ProviderProfile, apiKey: string | null) => Promise<SettingsView>;
  deleteProfile: (id: string) => Promise<SettingsView>;
  setDefaultProfile: (id: string) => Promise<SettingsView>;
  testProfile: (p: ProviderProfile, apiKey: string | null) => Promise<ConnectionTest>;
  listProfileModels: (p: ProviderProfile, apiKey: string | null) => Promise<string[]>;
  setSttKey: (key: string) => Promise<SettingsView>;
}

const defaultApi: SettingsApi = {
  saveProfile: ipc.saveProfile,
  deleteProfile: ipc.deleteProfile,
  setDefaultProfile: ipc.setDefaultProfile,
  testProfile: ipc.testProfile,
  listProfileModels: ipc.listProfileModels,
  setSttKey: ipc.setSttKey,
};

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  settings: SettingsView;
  onSettingsChange: (s: SettingsView) => void;
  general: GeneralSettings;
  onGeneralChange: (g: GeneralSettings) => void;
  whisperModels: WhisperModelInfo[];
  api?: SettingsApi;
}

const toProfile = ({ has_key: _hasKey, ...p }: ProfileView): ProviderProfile => p;

const inputCls =
  'w-full bg-slate-900 border border-slate-700 rounded px-2 py-1.5 text-xs text-slate-100 focus:outline-none focus:border-blue-500';

export const SettingsModal: React.FC<SettingsModalProps> = ({
  isOpen,
  onClose,
  settings,
  onSettingsChange,
  general,
  onGeneralChange,
  whisperModels,
  api = defaultApi,
}) => {
  const [tab, setTab] = useState<'providers' | 'general'>('providers');
  const [selectedId, setSelectedId] = useState<string | null>(settings.default_profile_id);
  const [form, setForm] = useState<ProviderProfile>(emptyProfile());
  const [apiKey, setApiKey] = useState('');
  const [models, setModels] = useState<string[]>([]);
  const [testResult, setTestResult] = useState<ConnectionTest | null>(null);
  const [testing, setTesting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sttKey, setSttKeyInput] = useState('');

  const selected = settings.profiles.find((p) => p.id === selectedId) ?? null;
  const isNew = selectedId === null;

  useEffect(() => {
    setForm(selected ? toProfile(selected) : emptyProfile());
    setApiKey('');
    setTestResult(null);
    setError(null);
    setModels([]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId]);

  const refreshModels = async (profile: ProviderProfile = form) => {
    try {
      setModels(await api.listProfileModels(profile, apiKey.trim() ? apiKey : null));
    } catch {
      setModels([]);
    }
  };

  useEffect(() => {
    if (isOpen && selected) refreshModels(toProfile(selected));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, selectedId]);

  if (!isOpen) return null;

  const run = async (fn: () => Promise<SettingsView>) => {
    setError(null);
    try {
      onSettingsChange(await fn());
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    }
  };

  const handleSave = async () => {
    const payload: ProviderProfile = {
      ...form,
      base_url: form.base_url && form.base_url.trim() ? form.base_url.trim() : null,
    };
    const ok = await run(() => api.saveProfile(payload, apiKey.trim() ? apiKey : null));
    if (ok) setApiKey('');
  };

  const handleDelete = async () => {
    if (!selected || !window.confirm(`Xóa profile "${selected.name}"?`)) return;
    if (await run(() => api.deleteProfile(selected.id))) setSelectedId(null);
  };

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      setTestResult(await api.testProfile(form, apiKey.trim() ? apiKey : null));
    } catch (e) {
      setTestResult({ ok: false, latency_ms: 0, error: String(e) });
    } finally {
      setTesting(false);
    }
  };

  const patchGeneral = (patch: Partial<GeneralSettings>) => onGeneralChange({ ...general, ...patch });

  return (
    <div className="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-4">
      <div className="w-full max-w-3xl max-h-[85vh] bg-slate-800 border border-slate-700 rounded-lg shadow-xl flex flex-col text-xs text-slate-200">
        <div className="flex items-center justify-between px-4 py-3 border-b border-slate-700">
          <div className="flex gap-2">
            <button
              onClick={() => setTab('providers')}
              className={`px-3 py-1 rounded ${tab === 'providers' ? 'bg-blue-600 text-white' : 'bg-slate-700'}`}
            >
              Providers
            </button>
            <button
              onClick={() => setTab('general')}
              className={`px-3 py-1 rounded ${tab === 'general' ? 'bg-blue-600 text-white' : 'bg-slate-700'}`}
            >
              Chung
            </button>
          </div>
          <button onClick={onClose} aria-label="Đóng" className="text-slate-400 hover:text-white">
            <X className="w-4 h-4" />
          </button>
        </div>

        {settings.secrets_backend === 'file' && (
          <div className="mx-4 mt-3 flex items-center gap-2 bg-amber-900/40 border border-amber-700 text-amber-200 rounded px-3 py-2">
            <AlertTriangle className="w-4 h-4 shrink-0" />
            <span>Không tìm thấy keychain hệ thống — API key đang lưu dạng file</span>
          </div>
        )}

        {tab === 'providers' ? (
          <div className="flex-1 min-h-0 flex gap-3 p-4">
            <div className="w-48 shrink-0 flex flex-col gap-1 overflow-y-auto">
              {settings.profiles.map((p) => (
                <button
                  key={p.id}
                  onClick={() => setSelectedId(p.id)}
                  className={`text-left px-2 py-1.5 rounded truncate ${
                    p.id === selectedId ? 'bg-slate-700 text-white' : 'hover:bg-slate-700/50'
                  }`}
                  title={KIND_LABELS[p.kind]}
                >
                  <span className="mr-1">{KIND_ICONS[p.kind]}</span>
                  <span>{p.id === settings.default_profile_id ? `★ ${p.name}` : p.name}</span>
                </button>
              ))}
              <button
                onClick={() => setSelectedId(null)}
                className="mt-1 px-2 py-1.5 rounded border border-dashed border-slate-600 hover:bg-slate-700/50"
              >
                + Thêm
              </button>
            </div>

            <div className="flex-1 flex flex-col gap-2.5 overflow-y-auto">
              <label className="flex flex-col gap-1">
                <span>Loại</span>
                <select
                  aria-label="Loại"
                  value={form.kind}
                  onChange={(e) => setForm({ ...form, kind: e.target.value as ProviderProfile['kind'] })}
                  className={inputCls}
                >
                  {PROVIDER_KINDS.map((k) => (
                    <option key={k} value={k}>
                      {KIND_LABELS[k]}
                    </option>
                  ))}
                </select>
              </label>
              <label className="flex flex-col gap-1">
                <span>Tên</span>
                <input
                  aria-label="Tên"
                  value={form.name}
                  onChange={(e) => setForm({ ...form, name: e.target.value })}
                  className={inputCls}
                />
              </label>
              <label className="flex flex-col gap-1">
                <span>Base URL</span>
                <input
                  aria-label="Base URL"
                  value={form.base_url ?? ''}
                  placeholder={defaultBaseUrl(form.kind) ?? 'https://...'}
                  onChange={(e) => setForm({ ...form, base_url: e.target.value })}
                  className={inputCls}
                />
              </label>
              {requiresApiKey(form.kind) && (
                <label className="flex flex-col gap-1">
                  <span className="flex items-center justify-between">
                    <span>API key</span>
                    {selected?.has_key && (
                      <button
                        type="button"
                        onClick={() => run(() => api.saveProfile(form, ''))}
                        className="text-rose-400 hover:underline"
                      >
                        Xóa key
                      </button>
                    )}
                  </span>
                  <input
                    aria-label="API key"
                    type="password"
                    value={apiKey}
                    placeholder={selected?.has_key ? '••• đã lưu' : 'sk-...'}
                    onChange={(e) => setApiKey(e.target.value)}
                    className={inputCls}
                  />
                </label>
              )}
              <label className="flex flex-col gap-1">
                <span className="flex items-center justify-between">
                  <span>Model</span>
                  <button
                    type="button"
                    aria-label="Tải danh sách model"
                    onClick={() => refreshModels()}
                    className="text-slate-400 hover:text-white"
                  >
                    <RefreshCw className="w-3.5 h-3.5" />
                  </button>
                </span>
                <input
                  aria-label="Model"
                  list="model-suggestions"
                  value={form.model}
                  onChange={(e) => setForm({ ...form, model: e.target.value })}
                  className={inputCls}
                />
                <datalist id="model-suggestions">
                  {models.map((m) => (
                    <option key={m} value={m} />
                  ))}
                </datalist>
              </label>

              {testResult && (
                <div className={testResult.ok ? 'text-emerald-400' : 'text-rose-400'}>
                  {testResult.ok ? `✅ Kết nối OK (${testResult.latency_ms} ms)` : `❌ ${testResult.error}`}
                </div>
              )}
              {error && <div className="text-rose-400">{error}</div>}

              <div className="flex flex-wrap gap-2 pt-1">
                <button
                  onClick={handleTest}
                  disabled={testing}
                  className="px-3 py-1.5 rounded bg-slate-700 hover:bg-slate-600 disabled:opacity-50"
                >
                  {testing ? 'Đang test...' : 'Test kết nối'}
                </button>
                <button onClick={handleSave} className="px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-white">
                  Lưu
                </button>
                {!isNew && selected && (
                  <>
                    {selected.id !== settings.default_profile_id && (
                      <button
                        onClick={() => run(() => api.setDefaultProfile(selected.id))}
                        className="px-3 py-1.5 rounded bg-slate-700 hover:bg-slate-600"
                      >
                        Đặt mặc định
                      </button>
                    )}
                    <button onClick={handleDelete} className="px-3 py-1.5 rounded bg-rose-700 hover:bg-rose-600 text-white">
                      Xóa
                    </button>
                  </>
                )}
              </div>
            </div>
          </div>
        ) : (
          <div className="flex-1 min-h-0 flex flex-col gap-3 p-4 overflow-y-auto">
            <label className="flex flex-col gap-1">
              <span>Whisper model</span>
              <select
                aria-label="Whisper model"
                value={general.whisper_model}
                onChange={(e) => patchGeneral({ whisper_model: e.target.value })}
                className={inputCls}
              >
                {whisperModels.map((m) => (
                  <option key={m.name} value={m.name}>
                    {m.name} ({m.size_mb}MB){m.downloaded ? ' ✓' : ''}
                  </option>
                ))}
              </select>
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={general.use_cloud_stt}
                onChange={(e) => patchGeneral({ use_cloud_stt: e.target.checked })}
              />
              <span>Dùng Whisper API (OpenAI) thay cho Whisper local</span>
            </label>
            {general.use_cloud_stt && (
              <div className="flex gap-2">
                <input
                  aria-label="API key Whisper"
                  type="password"
                  value={sttKey}
                  placeholder={settings.stt_has_key ? '••• đã lưu' : 'sk-...'}
                  onChange={(e) => setSttKeyInput(e.target.value)}
                  className={inputCls}
                />
                <button
                  onClick={async () => {
                    if (await run(() => api.setSttKey(sttKey))) setSttKeyInput('');
                  }}
                  className="px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-white shrink-0"
                >
                  Lưu key
                </button>
              </div>
            )}
            <label className="flex flex-col gap-1">
              <span>Ngôn ngữ đích</span>
              <select
                aria-label="Ngôn ngữ đích"
                value={general.target_lang}
                onChange={(e) => patchGeneral({ target_lang: e.target.value })}
                className={inputCls}
              >
                <option value="vi">Tiếng Việt (VI)</option>
                <option value="en">English (EN)</option>
                <option value="ja">日本語 (JA)</option>
                <option value="ko">한국어 (KO)</option>
                <option value="zh">中文 (ZH)</option>
              </select>
            </label>
            <label className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={general.bilingual}
                onChange={(e) => patchGeneral({ bilingual: e.target.checked })}
              />
              <span>Phụ đề song ngữ</span>
            </label>
            <label className="flex flex-col gap-1">
              <span>Style mặc định</span>
              <select
                aria-label="Style mặc định"
                value={general.style_preset}
                onChange={(e) => patchGeneral({ style_preset: e.target.value as StylePreset })}
                className={inputCls}
              >
                <option value="youtube_yellow">YouTube Yellow</option>
                <option value="classic_white">Classic White</option>
                <option value="netflix_modern">Netflix Modern</option>
                <option value="bilingual_highlight">Bilingual Highlight</option>
              </select>
            </label>
            {error && <div className="text-rose-400">{error}</div>}
          </div>
        )}
      </div>
    </div>
  );
};
```

- [ ] **Step 4: Run tests**

Run: `npm --prefix ui test -- src/components/SettingsModal.test.tsx`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/SettingsModal.tsx ui/src/components/SettingsModal.test.tsx
git commit -m "feat(ui): SettingsModal for provider profiles and general settings"
```

---

### Task 12: HeaderBar profile select + App persistence

**Files:**
- Modify: `ui/src/components/HeaderBar.tsx`, `ui/src/components/HeaderBar.test.tsx`, `ui/src/App.tsx`
- Create: `ui/src/App.test.tsx`

**Interfaces:**
- Consumes: Task 10 IPC + types, Task 11 `SettingsModal`.
- Produces: `HeaderBar` props — remove `ollamaModel`, `onSelectOllamaModel`; add `profiles: ProfileView[]`, `defaultProfileId: string | null`, `onSelectProfile: (id: string) => void`, `onOpenSettings: () => void`.

- [ ] **Step 1: Write the failing tests**

`ui/src/components/HeaderBar.test.tsx` — in both existing `render(<HeaderBar ... />)` calls replace the two lines

```tsx
        ollamaModel="qwen2.5:7b"
        onSelectOllamaModel={vi.fn()}
```

with

```tsx
        profiles={[]}
        defaultProfileId={null}
        onSelectProfile={vi.fn()}
        onOpenSettings={vi.fn()}
```

and append inside `describe('HeaderBar', ...)`:

```tsx
  it('renders profiles, marks default, and reports selection and settings click', () => {
    const onSelectProfile = vi.fn();
    const onOpenSettings = vi.fn();
    render(
      <HeaderBar
        videoPath=""
        onSelectVideo={vi.fn()}
        whisperModel="base"
        onSelectWhisperModel={vi.fn()}
        profiles={[
          { id: 'o', name: 'Ollama local', kind: 'ollama', base_url: null, model: 'qwen2.5:7b', has_key: false },
          { id: 'd', name: 'DeepSeek', kind: 'deepseek', base_url: null, model: 'deepseek-chat', has_key: true },
        ]}
        defaultProfileId="o"
        onSelectProfile={onSelectProfile}
        onOpenSettings={onOpenSettings}
        targetLang="vi"
        onSelectTargetLang={vi.fn()}
        bilingual={true}
        onToggleBilingual={vi.fn()}
        stylePreset="youtube_yellow"
        onSelectPreset={vi.fn()}
        isRunning={false}
        onStart={vi.fn()}
        onCancel={vi.fn()}
        systemInfo={null}
        onDownloadModel={vi.fn()}
        downloadingModel={null}
      />
    );
    const select = screen.getByLabelText('Provider profile') as HTMLSelectElement;
    expect(select.value).toBe('o');
    expect(screen.getByRole('option', { name: /DeepSeek · deepseek-chat/ })).toBeDefined();
    fireEvent.change(select, { target: { value: 'd' } });
    expect(onSelectProfile).toHaveBeenCalledWith('d');
    fireEvent.click(screen.getByRole('button', { name: 'Cài đặt' }));
    expect(onOpenSettings).toHaveBeenCalled();
  });
```

and change its first import line to `import { render, screen, fireEvent } from '@testing-library/react';`.

`ui/src/App.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
import type { SettingsView } from './types';

// vi.mock is hoisted above imports, so shared fixtures must come from vi.hoisted.
const { loaded, saveGeneral } = vi.hoisted(() => {
  const loaded: SettingsView = {
    profiles: [
      { id: 'd', name: 'DeepSeek', kind: 'deepseek', base_url: null, model: 'deepseek-chat', has_key: true },
    ],
    default_profile_id: 'd',
    general: {
      whisper_model: 'small',
      target_lang: 'en',
      bilingual: true,
      style_preset: 'netflix_modern',
      use_cloud_stt: false,
    },
    stt_has_key: false,
    secrets_backend: 'keyring',
    load_warning: 'File cài đặt bị hỏng',
  };
  return { loaded, saveGeneral: vi.fn().mockResolvedValue(loaded) };
});

vi.mock('./lib/ipc', async (importOriginal) => {
  const mod = await importOriginal<typeof import('./lib/ipc')>();
  return {
    ...mod,
    isTauri: () => false,
    getSystemInfo: vi.fn().mockResolvedValue({
      has_cuda: false,
      ollama_running: false,
      ollama_models: [],
      whisper_models: [
        { name: 'base', downloaded: true, size_mb: 142 },
        { name: 'small', downloaded: true, size_mb: 466 },
      ],
    }),
    getOutputDir: vi.fn().mockResolvedValue('/out'),
    listenPipelineProgress: vi.fn().mockResolvedValue(() => {}),
    getSettings: vi.fn().mockResolvedValue(loaded),
    saveGeneral: (...args: unknown[]) => saveGeneral(...args),
    setDefaultProfile: vi.fn().mockResolvedValue(loaded),
  };
});

import App from './App';

beforeEach(() => saveGeneral.mockClear());

describe('App settings persistence', () => {
  it('loads general settings and default profile without re-saving', async () => {
    render(<App />);
    await waitFor(() =>
      expect((screen.getByDisplayValue('English (EN)') as HTMLSelectElement).value).toBe('en')
    );
    expect((screen.getByLabelText('Provider profile') as HTMLSelectElement).value).toBe('d');
    expect(saveGeneral).not.toHaveBeenCalled();
  });

  it('debounces save_general after header change', async () => {
    render(<App />);
    await waitFor(() => screen.getByDisplayValue('English (EN)'));
    fireEvent.click(screen.getByRole('button', { name: /Song ngữ/ }));
    expect(saveGeneral).not.toHaveBeenCalled();
    await waitFor(
      () => expect(saveGeneral).toHaveBeenCalledWith(expect.objectContaining({ bilingual: false, target_lang: 'en' })),
      { timeout: 2000 }
    );
    expect(saveGeneral).toHaveBeenCalledTimes(1);
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm --prefix ui test -- src/components/HeaderBar.test.tsx src/App.test.tsx`
Expected: FAIL (no `Provider profile` select; App still uses `ollamaModel`).

- [ ] **Step 3: Implement**

`ui/src/components/HeaderBar.tsx`:

1. Add `Settings` to the `lucide-react` import list; change the types import to `import type { SystemInfo, StylePreset, ProfileView } from '../types';` and add `import { KIND_ICONS } from '../lib/providers';`.
2. In `HeaderBarProps`, replace

```ts
  ollamaModel: string;
  onSelectOllamaModel: (m: string) => void;
```

with

```ts
  profiles: ProfileView[];
  defaultProfileId: string | null;
  onSelectProfile: (id: string) => void;
  onOpenSettings: () => void;
```

and in the destructuring replace `ollamaModel, onSelectOllamaModel,` with `profiles, defaultProfileId, onSelectProfile, onOpenSettings,`.
3. Replace the whole `{/* Ollama / LLM Translation selector */}` `<div>…</div>` block with:

```tsx
        {/* Provider profile selector */}
        <div className="flex items-center bg-slate-900/60 rounded-md border border-slate-700 px-1.5 py-0.5">
          <Languages className="w-3.5 h-3.5 text-indigo-400 mr-1.5" />
          <select
            aria-label="Provider profile"
            value={defaultProfileId ?? ''}
            onChange={(e) => onSelectProfile(e.target.value)}
            disabled={isRunning || profiles.length === 0}
            className="bg-transparent text-slate-200 focus:outline-none py-1 text-xs cursor-pointer max-w-[180px] truncate"
          >
            {profiles.length === 0 && (
              <option value="" className="bg-slate-800">
                Chưa có profile
              </option>
            )}
            {profiles.map((p) => (
              <option key={p.id} value={p.id} className="bg-slate-800">
                {KIND_ICONS[p.kind]} {p.name} · {p.model}
              </option>
            ))}
          </select>
          <button
            onClick={onOpenSettings}
            aria-label="Cài đặt"
            title="Cài đặt provider & chung"
            className="ml-1 text-slate-400 hover:text-white p-0.5"
          >
            <Settings className="w-3.5 h-3.5" />
          </button>
        </div>
```

`ui/src/App.tsx`:

1. Imports: add `getSettings, saveGeneral, setDefaultProfile,` to the `./lib/ipc` import; add `import { SettingsModal } from './components/SettingsModal';`; add `GeneralSettings, SettingsView` to the type import.
2. Replace the five "App settings state" lines (`whisperModel` … `stylePreset`) with:

```tsx
  const [general, setGeneral] = useState<GeneralSettings>({
    whisper_model: 'base',
    target_lang: 'vi',
    bilingual: true,
    style_preset: 'youtube_yellow',
    use_cloud_stt: false,
  });
  const [settingsView, setSettingsView] = useState<SettingsView | null>(null);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const lastSavedGeneral = useRef<string | null>(null);
  const patchGeneral = (patch: Partial<GeneralSettings>) => setGeneral((g) => ({ ...g, ...patch }));
  const { whisper_model: whisperModel, target_lang: targetLang, bilingual, style_preset: stylePreset } = general;
```

3. Delete the "Auto-select first available Ollama model" `useEffect` block entirely.
4. After the `getOutputDir` `useEffect`, add:

```tsx
  // Load persisted settings once
  useEffect(() => {
    getSettings()
      .then((s) => {
        lastSavedGeneral.current = JSON.stringify(s.general);
        setGeneral(s.general);
        setSettingsView(s);
        if (s.load_warning) setLogs((prev) => [...prev, `[CẢNH BÁO] ${s.load_warning}`]);
      })
      .catch((err) => console.warn('getSettings failed:', err));
  }, []);

  // Persist general settings, debounced
  useEffect(() => {
    if (lastSavedGeneral.current === null) return;
    const serialized = JSON.stringify(general);
    if (serialized === lastSavedGeneral.current) return;
    const timer = setTimeout(() => {
      lastSavedGeneral.current = serialized;
      saveGeneral(general).catch((err) => console.warn('saveGeneral failed:', err));
    }, 500);
    return () => clearTimeout(timer);
  }, [general]);

  const handleSelectProfile = async (id: string) => {
    try {
      setSettingsView(await setDefaultProfile(id));
    } catch (err) {
      setLogs((prev) => [...prev, `[LỖI] ${String(err)}`]);
    }
  };
```

5. In `runJob`, replace the `startPipeline({ ... })` argument object with:

```tsx
      const result = await startPipeline({
        job_id: job.id,
        video_path: job.videoPath,
        whisper_model: whisperModel,
        use_cloud_stt: general.use_cloud_stt,
        target_lang: targetLang,
        skip_translate: false,
        profile_id: settingsView?.default_profile_id ?? null,
      });
```

6. In `<HeaderBar …>` replace

```tsx
        onSelectWhisperModel={setWhisperModel}
        ollamaModel={ollamaModel}
        onSelectOllamaModel={setOllamaModel}
        targetLang={targetLang}
        onSelectTargetLang={setTargetLang}
        bilingual={bilingual}
        onToggleBilingual={() => setBilingual(!bilingual)}
        stylePreset={stylePreset}
        onSelectPreset={setStylePreset}
```

with

```tsx
        onSelectWhisperModel={(m) => patchGeneral({ whisper_model: m })}
        profiles={settingsView?.profiles ?? []}
        defaultProfileId={settingsView?.default_profile_id ?? null}
        onSelectProfile={handleSelectProfile}
        onOpenSettings={() => setIsSettingsOpen(true)}
        targetLang={targetLang}
        onSelectTargetLang={(l) => patchGeneral({ target_lang: l })}
        bilingual={bilingual}
        onToggleBilingual={() => patchGeneral({ bilingual: !bilingual })}
        stylePreset={stylePreset}
        onSelectPreset={(p) => patchGeneral({ style_preset: p })}
```

7. Just before the closing `</div>` of the root element (after `<ExportModal … />`), add:

```tsx
      {/* 5. Settings Modal */}
      {settingsView && (
        <SettingsModal
          isOpen={isSettingsOpen}
          onClose={() => setIsSettingsOpen(false)}
          settings={settingsView}
          onSettingsChange={setSettingsView}
          general={general}
          onGeneralChange={setGeneral}
          whisperModels={systemInfo?.whisper_models ?? []}
        />
      )}
```

8. Remove any now-unused imports (`StylePreset` if unused).

- [ ] **Step 4: Run tests, typecheck, build**

Run: `npm --prefix ui test`
Expected: all test files pass.

Run: `npm --prefix ui run build`
Expected: `tsc` clean, vite build succeeds.

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/HeaderBar.tsx ui/src/components/HeaderBar.test.tsx ui/src/App.tsx ui/src/App.test.tsx
git commit -m "feat(ui): profile selector in header, settings modal, persisted general settings"
```

---

### Task 13: Full verification

**Files:** none (fix-ups only if something fails).

- [ ] **Step 1: Rust workspace**

Run: `cargo test --workspace`
Expected: all pass (ffmpeg-dependent e2e test behaves as before this change).

Run: `cargo clippy --workspace --all-targets 2>&1 | grep -E "^(warning|error)" | sort | uniq -c`
Expected: no new warnings in `src/settings`, `src/llm`, `src/translate`, `src-tauri/src/settings_commands.rs`.

- [ ] **Step 2: Frontend**

Run: `npm --prefix ui test && npm --prefix ui run build`
Expected: pass.

- [ ] **Step 3: Desktop bundle**

Run: `npx tauri build --bundles deb`
Expected: `Finished 1 bundle`.

- [ ] **Step 4: Manual smoke (user)**

1. Start `npm run dev`; open ⚙️ → Providers. Add a DeepSeek profile with a real key → **Test kết nối** shows ✅; model list suggests `deepseek-chat`.
2. Add an Anthropic profile → Test ✅.
3. Select each profile in the header and run one short video per profile; translated subtitles appear.
4. Change target language and bilingual, restart the app → values persist.
5. `cargo run --bin auto-subtitle -- --list-profiles` lists the same profiles; `cargo run --bin auto-subtitle -- -i jfk.flac --profile DeepSeek --mode subs-only` produces `.vi.srt`.

- [ ] **Step 5: Commit any fix-ups**

```bash
git add -A
git commit -m "chore: fix-ups from provider settings verification"
```

(Skip if nothing changed.)
