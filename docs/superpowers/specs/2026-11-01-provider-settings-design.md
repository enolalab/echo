# Provider Profiles & Persistent Settings — Design

Sub-project **A** of 4 (A: Provider + Settings → B: Translation Skills → C: UI/UX redesign → D: Progress visualization).

## Goal

Let users configure multiple LLM provider profiles (OpenAI, OpenAI-compatible, Anthropic, Anthropic-compatible, DeepSeek, Ollama), store API keys securely, persist general app settings across restarts, and share all of it between the desktop app and the CLI.

## Non-goals

- Translation skills / system prompts (sub-project B). Only the `system_prompt` hook is added here, always `None`.
- Visual redesign, dark/light theming (sub-project C). Settings UI here is functional, using existing styling.
- Progress UI changes (sub-project D).
- Per-skill or per-job profile selection beyond one profile per pipeline run.

## 1. Data model & storage

Location: `dirs::config_dir()/echo/settings.json`.

```rust
// src/settings/model.rs
pub struct Settings {
    pub version: u32,                       // 1
    pub profiles: Vec<ProviderProfile>,
    pub default_profile_id: Option<String>,
    pub general: GeneralSettings,
}

// JSON values: "openai", "openai_compatible", "anthropic", "anthropic_compatible", "deepseek", "ollama"
pub enum ProviderKind { OpenAi, OpenAiCompatible, Anthropic, AnthropicCompatible, DeepSeek, Ollama }

pub struct ProviderProfile {
    pub id: String,              // uuid v4; also the keychain account suffix
    pub name: String,            // unique (case-insensitive) display name
    pub kind: ProviderKind,
    pub base_url: Option<String>,// None => kind default
    pub model: String,
}

pub struct GeneralSettings {
    pub whisper_model: String,   // default "base"
    pub target_lang: String,     // default "vi"
    pub bilingual: bool,         // default true
    pub style_preset: String,    // default "youtube_yellow"
    pub use_cloud_stt: bool,     // default false
}
```

Default base URLs:

| Kind | Default |
|---|---|
| OpenAi | `https://api.openai.com/v1` |
| DeepSeek | `https://api.deepseek.com/v1` |
| Anthropic | `https://api.anthropic.com` |
| Ollama | `http://localhost:11434` |
| OpenAiCompatible / AnthropicCompatible | none — `base_url` required; save rejected if empty |

### Secrets

API keys never appear in `settings.json`.

```rust
// src/settings/secrets.rs
pub trait SecretStore: Send + Sync {
    fn get(&self, account: &str) -> anyhow::Result<Option<String>>;
    fn set(&self, account: &str, value: &str) -> anyhow::Result<()>;
    fn delete(&self, account: &str) -> anyhow::Result<()>;
}
```

- Accounts: `profile:<id>` for LLM keys, `stt:openai` for the Whisper API key.
- `KeyringStore` — crate `keyring` **3.6** (stable sync API; 4.x has a new API), features `apple-native`, `windows-native`, `sync-secret-service`, `crypto-rust`; service `com.echo.subtitles`. Linux CI needs `libdbus-1-dev`.
- `FileStore` — `secrets.json` next to `settings.json`, permissions `0600` on Unix.
- `open_secret_store()` probes the keyring once (set + get + delete a probe entry). On any failure it returns `FileStore` and reports `SecretsBackend::File`; otherwise `SecretsBackend::Keyring`.

### Persistence rules

- Writes are atomic: write `settings.json.tmp`, then rename.
- If `settings.json` is missing → create defaults with one profile `"Ollama local"` (kind Ollama, model `qwen2.5:7b`) set as default.
- If `settings.json` fails to parse → rename to `settings.json.bak`, create defaults, and surface a one-time warning.
- Unknown fields are ignored on read (forward compatibility).

## 2. Provider & translator layer

New module `src/llm/`:

```rust
#[async_trait::async_trait]
pub trait ChatClient: Send + Sync {
    async fn chat(&self, system: Option<&str>, user: &str) -> Result<String, LlmError>;
    async fn list_models(&self) -> Result<Vec<String>, LlmError>;
}

pub fn build_chat_client(profile: &ProviderProfile, api_key: Option<String>) -> Box<dyn ChatClient>;
```

| Client | Kinds | Chat endpoint | Models endpoint |
|---|---|---|---|
| `OpenAiChatClient` | OpenAi, OpenAiCompatible, DeepSeek | `POST {base}/chat/completions`, `Authorization: Bearer` | `GET {base}/models` |
| `AnthropicChatClient` | Anthropic, AnthropicCompatible | `POST {base}/v1/messages`, headers `x-api-key`, `anthropic-version: 2023-06-01`, `max_tokens: 8192`, `system` top-level field | `GET {base}/v1/models` |
| `OllamaChatClient` | Ollama | `POST {base}/api/chat`, `stream: false` | `GET {base}/api/tags` |

`system` maps to: OpenAI/Ollama → a leading `{"role":"system"}` message; Anthropic → top-level `system`.

### Errors

```rust
pub enum LlmError { Auth, NotFound, RateLimit, Timeout, Network(String), Other { status: Option<u16>, body: String } }
```

HTTP 401/403 → `Auth`; 404 → `NotFound`; 429 → `RateLimit`; reqwest timeout → `Timeout`; connect error → `Network(host)`; anything else → `Other` with the first 200 chars of the body. `LlmError::user_message()` returns the Vietnamese text shown in the UI:

| Variant | Message |
|---|---|
| Auth | "API key không hợp lệ" |
| NotFound | "Sai base URL hoặc tên model" |
| RateLimit | "Bị giới hạn tốc độ, thử lại sau" |
| Timeout / Network | "Không kết nối được tới {host}" |
| Other | "Lỗi provider ({status}): {body}" |

### Translator

```rust
// src/translate/llm_translator.rs
pub struct LlmTranslator { client: Box<dyn ChatClient>, system_prompt: Option<String> }
```

Implements the existing `Translator` trait: `build_translation_prompt` → `client.chat(system_prompt, prompt)` → `parse_translated_lines`. It does **not** retry itself; it returns the `LlmError` wrapped in `anyhow::Error` so the pipeline can inspect it.

Retry lives in the existing loop in `pipeline::translate_all_with_progress` (`MAX_ATTEMPTS = 3`). Changes to that loop:
- If the error chain contains an `LlmError` with `!is_retryable()` (`Auth`, `NotFound`, `Other` with 4xx status) → stop immediately.
- If it is `RateLimit` or `Other` with 5xx status → sleep `2s × attempt` (2s, then 4s) before the next attempt.
- Any other error (including non-`LlmError` errors from test doubles) → retry immediately, as today.

`OllamaTranslator` and `OpenAiCompatTranslator` are deleted; their HTTP code moves into the chat clients. The `Translator` trait is unchanged; `pipeline.rs` changes only in the retry loop described above.

### Connection test

`test_connection(client)` sends `chat(None, "Reply with: OK")` with a 20s timeout and returns `{ ok: bool, latency_ms: u64, error: Option<String> }`. Content of the reply is not validated — any successful response counts.

## 3. Tauri commands, UI, CLI

### Commands (`src-tauri/src/commands.rs`, thin wrappers over `auto_subtitle::settings`)

| Command | Notes |
|---|---|
| `get_settings() -> SettingsView` | `SettingsView` = `Settings` + per-profile `has_key: bool` + `stt_has_key: bool` + `secrets_backend: "keyring" \| "file"` + `load_warning: Option<String>` |
| `save_profile(profile, api_key: Option<String>)` | Creates if `id` unknown. `None` = keep existing key; `Some("")` = delete key; `Some(k)` = set key. Rejects duplicate names and empty `base_url` for `*Compatible` kinds. |
| `delete_profile(id)` | Also deletes its secret. If it was default, default becomes the first remaining profile or `None`. |
| `set_default_profile(id)` | |
| `save_general(general)` | |
| `set_stt_key(key: String)` | Empty string deletes. |
| `test_profile(profile, api_key: Option<String>)` | Works on unsaved form values; `None` falls back to the stored key for that `id`. |
| `list_profile_models(profile, api_key: Option<String>) -> Vec<String>` | Errors returned as `user_message()`; UI treats failure as "no suggestions". |

### `PipelineConfig` changes

Remove `use_cloud_llm`, `llm_model`, `llm_api_base`, `llm_api_key`, `stt_api_key`. Add `profile_id: Option<String>` (`None` = default profile). The backend resolves the profile and reads keys from the secret store, so keys never cross IPC during a pipeline run. Starting a pipeline with no resolvable profile while translation is enabled fails with "Chưa cấu hình provider dịch".

`use_cloud_stt` stays on `PipelineConfig`. Today the desktop pipeline ignores it and always runs local Whisper; after this change, `use_cloud_stt = true` extracts MP3 (`media::build_extract_mp3_command`) and transcribes with `ApiTranscriber` (`https://api.openai.com/v1`, model `whisper-1`) using the `stt:openai` secret. Missing key fails with "Chưa nhập API key Whisper".

### UI (functional; restyled in sub-project C)

- **HeaderBar**: the Ollama model select becomes a **Profile** select (profile name + kind icon); add a ⚙️ button opening Settings. Changing the select calls `set_default_profile`, so the header always shows and persists the default profile (one concept, no separate "session profile").
- **`SettingsModal`** (new component), two tabs:
  - *Providers*: profile list (left, with "+ Thêm", default marked ★) and form (right): Kind, Name, Base URL (placeholder = kind default), API key (password input; shows "••• đã lưu" when `has_key`, empty input = keep), Model (combobox: suggestions from `list_profile_models`, fetched on open and on a ↻ button; free text always allowed). Buttons: Test kết nối (shows ✅ latency or ❌ message inline), Lưu, Xóa (confirm), Đặt mặc định.
  - *Chung*: Whisper model, STT cloud toggle + Whisper API key field, target language, bilingual, style preset.
- **Keychain warning banner** inside Settings when `secrets_backend == "file"`: "Không tìm thấy keychain hệ thống — API key đang lưu dạng file".
- **Load warning**: if `load_warning` is set, show it once as a toast.
- **Persistence**: `App.tsx` loads settings on mount and replaces its local `useState` defaults. Changing whisper model, target language, bilingual, or style preset from the header calls `save_general`, debounced 500ms.
- `lib/ipc.ts` gets typed wrappers plus browser-mode mocks for every new command (matching the existing mock pattern).

### CLI

- New flag `--profile <name|id>`; resolves against `settings.json` (name match is case-insensitive). Unknown name/id → error listing available profile names.
- Precedence, highest first:
  1. `--translate-engine` given explicitly → legacy path using `--ollama-model`, `--ollama-url`, `--api-key`, `--api-base-url`, `--api-model` exactly as today (existing scripts unaffected).
  2. `--profile` given → that profile.
  3. Otherwise → default profile from settings; if settings have no profiles → legacy Ollama defaults.
- To detect "given explicitly", `translate_engine`, `ollama_model`, `ollama_url`, `api_base_url`, `api_model` change from `default_value` to `Option<_>`; the current defaults (`ollama`, `qwen2.5:7b`, `http://localhost:11434`, `https://api.openai.com/v1`, `gpt-4o-mini`) are applied in code on the legacy path.
- `--stt-engine api` without `--api-key`/`OPENAI_API_KEY` reads the `stt:openai` secret.
- The CLI reads only profiles and secrets, **not** `GeneralSettings`; its existing flag defaults (e.g. `--whisper-model small`) are unchanged.
- New flag `--list-profiles` printing `name | kind | model | default` and exiting. `main.rs` checks for this flag in raw `std::env::args()` **before** `Cli::parse()`, so `--input` stays a required `PathBuf` and `pipeline.rs` is untouched. The flag is also declared on `Cli` (`#[arg(long)] list_profiles: bool`) so it appears in `--help`.

## 4. Testing

> Not presented in conversation before approval — please review this section specifically.

Rust (`cargo test`, using the existing `wiremock` dev-dependency):

- `settings`: round-trip serialize/deserialize; defaults created when file missing; corrupt file → `.bak` + defaults + warning; atomic write leaves no partial file; validation (duplicate name, empty compatible base URL).
- `secrets`: `FileStore` get/set/delete in a `tempfile` dir; 0600 permission on Unix. `KeyringStore` is not unit-tested (needs OS keychain); `open_secret_store` fallback logic is tested via an injected failing store.
- `llm` clients, each against `wiremock`: correct URL, headers (`Authorization` vs `x-api-key` + `anthropic-version`), system-prompt placement, response parsing, model-list parsing; status → `LlmError` mapping (401, 404, 429, 500, timeout).
- `LlmTranslator`: system prompt forwarded to the client; existing prompt/parse behaviour preserved (reuse current translator tests).
- `pipeline::translate_all_with_progress`: stops after 1 attempt on `LlmError::Auth`; retries with backoff on `RateLimit` (tested with `tokio::time::pause`); existing retry test still passes.
- CLI: `--profile` resolution by name and id; unknown profile error; explicit `--translate-engine` takes the legacy path even when a default profile exists; `--list-profiles` works without `--input`.

Frontend (`vitest`, existing setup):

- `SettingsModal`: add/edit/delete profile flows call the right IPC wrappers; empty API key input sends `None`; Test button renders success/failure states; keychain banner shown only for `file` backend.
- `HeaderBar`: profile select renders profiles and marks the default.
- `App`: general settings load on mount; header changes trigger debounced `save_general`.

Manual: build the desktop app and run one translation each with an OpenAI-compatible endpoint (DeepSeek), Anthropic, and Ollama.

## Files touched

- New: `src/settings/{mod,model,secrets,store}.rs`, `src/llm/{mod,openai,anthropic,ollama,error}.rs`, `src/translate/llm_translator.rs`, `ui/src/components/SettingsModal.tsx` (+ test).
- Modified: `src/lib.rs`, `src/translate/mod.rs`, `src/cli.rs`, `src/main.rs`, `src-tauri/src/{commands,types,main,state}.rs`, `ui/src/App.tsx`, `ui/src/components/HeaderBar.tsx`, `ui/src/lib/ipc.ts`, `ui/src/types/index.ts`, `Cargo.toml` (`keyring`, `uuid`).
- Deleted: `src/translate/ollama.rs`, `src/translate/openai_compat.rs`.
