# Translation Skills — Design

Sub-project **B** of 4 (A: Provider + Settings → **B: Translation Skills** → C: UI/UX redesign → D: Progress visualization).

**Depends on A** (`docs/superpowers/specs/2026-11-01-provider-settings-design.md`): `Settings`, `SettingsStore`, `SettingsView`, `LlmTranslator`, `ChatClient`, `SettingsModal`, the header profile select, and the `--list-profiles` pre-parse pattern. B must be implemented after A.

## Goal

Let users pick a domain "Skill" (system prompt + glossary) so translations use correct subject terminology. Ship read-only education presets; let users create, duplicate, import/export, and test their own skills. Skills apply per queue (default) with per-job override, in both the desktop app and the CLI.

## Non-goals

- Per-skill provider profile or sampling parameters (temperature stays 0.2).
- Visual redesign of the new UI pieces (sub-project C).
- Automatic skill detection from video content.
- Glossary enforcement after translation (no post-processing/replacement of model output).

## 1. Data model

```rust
// src/skills/model.rs
pub struct Skill {
    pub id: String,              // uuid v4 for user skills; "builtin:<slug>" for presets
    pub name: String,
    pub description: String,     // one line; may be empty
    pub system_prompt: String,   // may contain {source_lang} and {target_lang}
    #[serde(default)]
    pub glossary: Vec<GlossaryEntry>,
}

pub struct GlossaryEntry {
    pub source: String,          // term as it appears in the source language
    pub target: String,          // translation; "" = keep the source term unchanged
}

pub const NO_SKILL: &str = "none";
pub const BUILTIN_PREFIX: &str = "builtin:";
```

### Settings changes (additive; `version` stays 1)

```rust
pub struct Settings {
    // ...fields from A...
    #[serde(default)]
    pub skills: Vec<Skill>,                // user skills only; presets are never stored
    #[serde(default)]
    pub default_skill_id: Option<String>,  // None = plain translation
}
```

`Settings::with_defaults()` sets `skills: vec![]`, `default_skill_id: None`.

### Validation (`Skill::validate(&self, others: &[Skill]) -> Result<(), String>`)

`others` = all presets + all user skills except the one being validated (same `id`).

| Rule | Message |
|---|---|
| name trimmed non-empty | "Tên skill không được để trống" |
| name unique case-insensitively across `others` (presets included) | "Tên skill đã tồn tại" |
| system_prompt trimmed non-empty | "System prompt không được để trống" |
| system_prompt ≤ 8000 chars | "System prompt tối đa 8000 ký tự" |
| glossary ≤ 1000 entries | "Glossary tối đa 1000 thuật ngữ" |
| every `source` trimmed non-empty | "Thuật ngữ gốc không được để trống" |
| `source` unique case-insensitively within the skill | "Thuật ngữ bị trùng: {source}" |

Saving trims `name`, `description`, each `source` and `target`.

## 2. Presets

Six JSON files in `src/skills/presets/`, embedded with `include_str!`, parsed once (`OnceLock`) by `skills::presets() -> &'static [Skill]`. Never written to disk, so app updates refresh them.

| id | name | Prompt rules (beyond the shared base) | Starter glossary |
|---|---|---|---|
| `builtin:math` | Toán học | Keep formulas, symbols, variable names and LaTeX unchanged; read-aloud math ("x squared") rendered in target-language math phrasing; Vietnamese textbook terminology | 30–40 terms (đạo hàm, tích phân, ma trận, vector, xác suất, …) |
| `builtin:ai-ml` | AI / Machine Learning | Keep model names, library names, and widely used English terms (transformer, embedding, fine-tune) when the Vietnamese community does; translate explanations naturally | 30–40 terms (overfitting → quá khớp, loss function → hàm mất mát, `transformer` → keep, …) |
| `builtin:programming` | Lập trình / CS | Never translate code, identifiers, commands, file paths, keywords; translate explanations | 25–35 terms (`array` → mảng, `compile` → biên dịch, `git` → keep, …) |
| `builtin:physics` | Vật lý | Keep SI units and symbols; standard Vietnamese physics terminology | 25–35 terms |
| `builtin:chemistry` | Hóa học | Keep chemical formulas and IUPAC names as written; standard Vietnamese chemistry terminology | 25–35 terms |
| `builtin:biology` | Sinh học | Keep Latin species names; standard Vietnamese biology terminology | 25–35 terms |

Shared base, included at the start of every preset prompt (English, because LLMs follow English instructions most reliably). `{domain}` is written literally into each JSON file when authoring (e.g. "mathematics"); only `{source_lang}`/`{target_lang}` are runtime placeholders:

> You are translating subtitles of an educational lecture in {domain} from {source_lang} into {target_lang}. Use the standard terminology a {target_lang}-speaking university lecturer in {domain} would use. Keep sentences short and natural for subtitles. Do not add explanations.

Preset glossaries target Vietnamese; for other target languages only the prompt rules apply (see §3).

## 3. Prompt composition

`src/skills/prompt.rs`:

```rust
pub fn render_system_prompt(skill: &Skill, batch: &[SubtitleSegment], source_lang: &str, target_lang: &str) -> String;
pub fn glossary_for_batch<'a>(glossary: &'a [GlossaryEntry], batch: &[SubtitleSegment]) -> Vec<&'a GlossaryEntry>;
```

- Placeholders: `{source_lang}` → the source language, or `"the source language"` when it is `auto`; `{target_lang}` → target code. Unknown `{...}` text is left as is.
- `glossary_for_batch`:
  - Haystack = all `original_text` in the batch, joined with `\n`, lowercased.
  - An entry matches if its lowercased `source` occurs in the haystack.
  - **ASCII-only terms** additionally need word boundaries: the char before the match is not `[a-z0-9]` (or is start-of-text) and the char after is not `[a-z0-9]` (or is end-of-text). So `ion` does not match `function`, but `relu` matches `relu,`.
  - **Terms containing any non-ASCII char** (`đạo hàm`, `微分`) match as plain substrings, because CJK text has no spaces between words.
  - Sort matches by `source` char length descending, then alphabetically; keep at most 100.
- **Preset glossaries are Vietnamese.** They are included only when `target_lang` starts with `vi`; for other targets a preset sends its prompt rules only. User-skill glossaries are always included.
- Output when the glossary is included and has matches:

```
<system_prompt with placeholders replaced>

Glossary — use these translations exactly; "(keep)" means keep the original term unchanged:
- gradient descent → hạ gradient
- softmax → (keep)
```

- No matches (or glossary excluded) → system prompt only, no Glossary section.
- No skill → `system = None` (behaviour identical to A).

### `LlmTranslator` change

A's `LlmTranslator::new(client, system_prompt: Option<String>)` becomes `LlmTranslator::new(client, skill: Option<Skill>)`. `translate_batch` calls `render_system_prompt(skill, segments, source_lang, target_lang)` per batch. The user message (numbered `[1]…[n]` prompt) and parsing are unchanged.

## 4. Selection & pipeline

- Global default: `Settings.default_skill_id`, chosen from the header (persisted via `set_default_skill`).
- Per-job override in the UI: `BatchJob.skillId?: string` — `undefined` = follow default **at run start**; `"none"` = no skill; `"<id>"` = that skill.
- `PipelineConfig.skill_id: Option<String>` — `None` = default skill; `Some("none")` = no skill; `Some(id)` = that skill.
- `SettingsStore::resolve_skill(&self, choice: Option<&str>) -> anyhow::Result<Option<Skill>>`:
  - `None` → default skill. If `default_skill_id` points to a skill that no longer exists (only possible after manual edits, since `delete_skill` clears it) → `Ok(None)`; a missing *default* is not an error.
  - `Some("none")` → `Ok(None)`.
  - `Some(q)` → match by id, then by case-insensitive name, across presets + user skills; no match → `Err("Không tìm thấy skill '{q}'")`. Never silently fall back.
- The desktop pipeline emits a `pipeline-log` line `Skill: {name}` (or `Skill: không dùng`) at job start.

### Store operations (`SettingsStore`)

| Method | Behaviour |
|---|---|
| `skills_all() -> Vec<Skill>` | presets followed by user skills |
| `upsert_skill(skill) -> Result<Skill>` | empty `id` → new uuid; `id` starting with `builtin:` → `Err("Không thể sửa skill có sẵn")`; validates; persists |
| `delete_skill(id)` | removes; if it was default → `default_skill_id = None`; `builtin:` → error |
| `duplicate_skill(id) -> Result<Skill>` | copies preset or user skill with new id and name `"{name} (bản sao)"`, suffixed ` (2)`, ` (3)`… if taken; persists |
| `set_default_skill(id: Option<&str>)` | `None` clears; unknown id → `Err("Không tìm thấy skill")` |
| `import_skill_json(text) -> Result<Skill>` | see §6 |
| `export_skill_json(id) -> Result<String>` | see §6 |

## 5. Try a skill

Tauri command `try_skill(skill: Skill, profile_id: Option<String>, sample: String, source_lang: String, target_lang: String) -> TrySkillResult`:

```rust
pub struct TrySkillResult { pub lines: Vec<String>, pub system_prompt: String, pub error: Option<String> }
```

- `skill` is the unsaved form value (validated first; validation error → `error`, empty `lines`).
- `sample` is split on newlines; blank lines dropped; more than 20 lines → `error = "Tối đa 20 dòng"`; zero lines → `error = "Nhập ít nhất 1 dòng"`.
- Lines become segments with ids 1..=n; one `LlmTranslator::translate_batch` call; `profile_id: None` = default profile.
- `system_prompt` is the exact rendered prompt for that batch, returned even when translation fails.
- LLM errors → `error = LlmError::user_message()` (or the anyhow chain for parse errors).

## 6. Import / Export

File extension `.echo-skill.json`; content:

```json
{ "format": "echo-skill", "version": 1,
  "skill": { "name": "...", "description": "...", "system_prompt": "...", "glossary": [{ "source": "...", "target": "..." }] } }
```

- Export: any skill (preset or user); `id` not written. Tauri `export_skill(id)` opens a save dialog (default file name `{name}.echo-skill.json`, filesystem-unsafe chars replaced with `_`) and returns the saved path, or `None` if cancelled.
- Import: Tauri `import_skill()` opens a file dialog, reads ≤ 1 MB, and calls `import_skill_json`:
  - `format != "echo-skill"` → "File không phải skill Echo"; `version != 1` → "Phiên bản skill không hỗ trợ: {v}"; JSON error → "File skill không hợp lệ: {e}".
  - New uuid; name conflicts get ` (2)`, ` (3)`…; then normal validation.
  - Returns `SettingsView` (dialog cancelled → unchanged view).

## 7. UI

### SettingsModal — new tab "Skills"

- Left column, two groups: **Có sẵn** (presets, 🔒) and **Của tôi** (user skills), with "+ Thêm" and "Nhập file"; default skill marked ★.
- Form: Tên, Mô tả, System prompt (textarea; helper text "Có thể dùng {source_lang} và {target_lang}"), Glossary table (Gốc | Dịch, empty Dịch shows placeholder "(giữ nguyên)"; add row, delete row), "Nhập CSV".
- Presets: all fields read-only; buttons **Nhân bản**, **Xuất file**, **Đặt mặc định**.
- User skills: **Lưu**, **Xóa** (confirm), **Nhân bản**, **Xuất file**, **Đặt mặc định**.
- **Nhập CSV**: textarea paste or `.csv` file picker (browser `<input type=file>`, read with `FileReader`). Parsed by `ui/src/lib/csv.ts`:
  - `parseGlossaryCsv(text) -> { entries: GlossaryEntry[], skipped: number }`; RFC-4180 quoting (`"a, b"`, `""` escape); `,` or `;` delimiter auto-detected from the first non-empty line; first row skipped if its cells are (case-insensitive) `source,target` or `gốc,dịch`; rows with empty first cell or >2 cells count as skipped; a 1-cell row means target `""`.
  - `mergeGlossary(existing, incoming) -> { glossary, added, updated }`, matching `source` case-insensitively; incoming wins.
  - Result shown as "Đã thêm {added}, cập nhật {updated}, bỏ qua {skipped} dòng lỗi". Changes apply to the form; user clicks Lưu to persist.
- **Chạy thử** panel: sample textarea, profile dropdown (default selected), "Dịch thử" button (calls `try_skill` with `source_lang = "auto"` and `target_lang` from general settings); shows a two-column original/translated table, error text if any, and a collapsible "Xem prompt" with `system_prompt`.

### Header

A **Skill** dropdown next to the Profile dropdown: "Không dùng skill", then presets, then user skills (default selected). Change calls `set_default_skill`.

### JobList

Each job row gets a compact skill select: "Mặc định ({default name or 'không dùng'})", "Không dùng skill", then all skills. Enabled only when the job status is `queued`, `error`, or `cancelled`; for `running`/`done` it shows a read-only badge with the skill name used (recorded on the job when it started: `BatchJob.skillUsed?: string`).

### IPC (`lib/ipc.ts`, with browser-mode mocks)

`saveSkill(skill)`, `deleteSkill(id)`, `duplicateSkill(id)`, `setDefaultSkill(id | null)`, `importSkill()` → `SettingsView`; `exportSkill(id) → string | null`; `trySkill(skill, profileId, sample, sourceLang, targetLang) → TrySkillResult`. `SettingsView` gains `presets: Skill[]`, `skills: Skill[]`, `default_skill_id: string | null`.

## 8. CLI

- `--skill <name|id|none>`; without it the settings default skill is used.
- Skill applies to every provider path, including legacy `--translate-engine`.
- `--skill` given but settings unreadable → error "Không đọc được settings.json nên không dùng được --skill"; unknown skill → the `resolve_skill` error.
- `--list-skills` prints `name | id | {n} terms | default` for presets then user skills and exits; detected in raw args before `Cli::parse()` (same as `--list-profiles`) and also declared on `Cli` for `--help`.

## 9. Testing

Rust (`cargo test`, `wiremock` where HTTP is involved):

- Presets: all six parse, ids `builtin:*`, unique names, non-empty prompts, glossaries pass validation.
- `render_system_prompt`: placeholder substitution incl. `auto`; Glossary section present only with matches; preset glossary included for `vi`/`vi-VN` and excluded for `en`; user glossary included for any target; `(keep)` for empty target.
- `glossary_for_batch`: case-insensitive; `ion` not matched in `function`; `ReLU` matched in `ReLU,`; non-ASCII term (`đạo hàm`, `微分`) matched as substring; ordering longest-first; cap 100.
- Validation: each rule in §1.
- Store: upsert (new id, builtin rejected), delete clears default, duplicate naming with suffixes, set_default unknown id, `resolve_skill` for `None` / `"none"` / id / name / unknown / missing default.
- Import/export: round-trip equality (minus id); wrong format, wrong version, invalid JSON, oversized; name conflict suffix.
- `LlmTranslator`: mock `ChatClient` receives a system prompt containing only the current batch's terms; `None` skill sends `system = None`.
- `try_skill`: lines + system_prompt returned; >20 lines and empty sample errors; invalid skill error; provider error still returns `system_prompt`.
- CLI: `--skill` by name, id, `none`; unknown skill error; `--list-skills` without `-i`; skill applied on legacy path (`choose_skill` pure function tested like A's `choose_translator`).

Frontend (`vitest`):

- `csv.ts`: header detection (both languages), quoted commas, `""` escapes, `;` delimiter, 1-cell rows, skipped rows; `mergeGlossary` counts.
- Skills tab: presets read-only with only Nhân bản / Xuất file / Đặt mặc định; save sends trimmed payload; add/remove glossary rows; CSV import updates the table and shows counts; Dịch thử renders lines and prompt.
- HeaderBar: Skill select lists none + presets + user skills and calls `onSelectSkill`.
- JobList: override select enabled only for queued/error/cancelled; badge for running/done.
- App: `skill_id` sent to `startPipeline` — `undefined` job → `null`, `"none"` → `"none"`, id → id.

Manual: run one ML lecture clip with `AI / Machine Learning` and once with "Không dùng skill"; compare terminology.

## Files touched

- New: `src/skills/{mod,model,prompt,presets,io}.rs`, `src/skills/presets/{math,ai-ml,programming,physics,chemistry,biology}.json`, `src-tauri/src/skill_commands.rs`, `ui/src/lib/csv.ts` (+ test), `ui/src/components/SkillsTab.tsx` (+ test).
- Modified: `src/lib.rs`, `src/settings/{model,store}.rs`, `src/translate/llm_translator.rs`, `src/cli.rs`, `src/main.rs`, `src-tauri/src/{commands,main,types,settings_commands}.rs`, `ui/src/components/{SettingsModal,HeaderBar,JobList}.tsx` (+ tests), `ui/src/App.tsx`, `ui/src/lib/ipc.ts`, `ui/src/types/index.ts`.
