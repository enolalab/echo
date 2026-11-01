# Translation Skills Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Domain "Skills" (system prompt + glossary) that steer translation terminology, with six read-only education presets, user-created skills, CSV glossary import, file import/export, a "try it" panel, a global default with per-job override, and CLI support.

**Architecture:** A new `skills` module in the core crate holds the model, embedded presets, prompt rendering, file I/O and the try-run helper. User skills live in `settings.json` via the `SettingsStore` from sub-project A. `LlmTranslator` takes an `Option<Skill>` and renders the system prompt per batch with only the glossary terms that occur in that batch. Tauri exposes thin commands; React adds a Skills tab, a header select and a per-job select.

**Tech Stack:** Rust 2021 (serde, serde_json, uuid, rfd, wiremock for tests), Tauri 2, React 18 + TypeScript + Vitest + Testing Library.

**Spec:** `docs/superpowers/specs/2026-11-01-translation-skills-design.md`

## Global Constraints

- **Sub-project A must be fully implemented first** (plan `docs/superpowers/plans/2026-11-01-provider-settings.md`). This plan edits files A creates; re-read each before editing.
- `Settings.version` stays `1`; new fields are `#[serde(default)]`.
- Preset ids: `builtin:math`, `builtin:ai-ml`, `builtin:programming`, `builtin:physics`, `builtin:chemistry`, `builtin:biology`.
- Reserved choice value `"none"` = no skill. Preset id prefix `builtin:`.
- Limits: system prompt ≤ 8000 chars; glossary ≤ 1000 entries; ≤ 100 glossary terms per batch; try-run ≤ 20 lines; import file ≤ 1 MB.
- Preset glossaries are included only when `target_lang` lowercased starts with `vi`; user-skill glossaries always.
- Glossary header line, exactly: `Glossary — use these translations exactly; "(keep)" means keep the original term unchanged:`
- Export format: `{"format":"echo-skill","version":1,"skill":{name,description,system_prompt,glossary}}`, file name `<name>.echo-skill.json`.
- User-facing strings are Vietnamese, exactly as written in this plan.

## File Structure

| File | Responsibility |
|---|---|
| `src/skills/mod.rs` | Re-exports |
| `src/skills/model.rs` | `Skill`, `GlossaryEntry`, constants, validation, `unique_name` |
| `src/skills/presets.rs` + `src/skills/presets/*.json` | Embedded presets |
| `src/skills/prompt.rs` | Placeholder substitution, glossary matching, system prompt rendering |
| `src/skills/io.rs` | Export/import JSON, safe file names |
| `src/skills/try_run.rs` | `try_skill` helper + `TrySkillResult` |
| `src/settings/{model,store}.rs` | Skill fields + store operations |
| `src/translate/llm_translator.rs` | Skill-aware system prompt |
| `src/cli.rs`, `src/main.rs` | `--skill`, `--list-skills` |
| `src-tauri/src/skill_commands.rs` | Skill Tauri commands |
| `ui/src/lib/csv.ts` | Glossary CSV parse/merge |
| `ui/src/lib/skills.ts` | Job → `skill_id`/label resolution |
| `ui/src/components/SkillsTab.tsx` | Skills management UI |

---

### Task 1: Skill model and validation

**Files:**
- Create: `src/skills/mod.rs`, `src/skills/model.rs`
- Modify: `src/lib.rs`
- Test: `tests/skills_model_test.rs`

**Interfaces:**
- Produces: `Skill { id, name, description, system_prompt, glossary: Vec<GlossaryEntry> }` (`Debug, Clone, Serialize, Deserialize, PartialEq, Eq`) with `is_builtin()`, `normalized(self) -> Self`, `validate(&self, others: &[Skill]) -> Result<(), String>`; `GlossaryEntry { source, target }`; consts `NO_SKILL`, `BUILTIN_PREFIX`, `MAX_PROMPT_CHARS`, `MAX_GLOSSARY`; `unique_name(base: &str, taken: &[String]) -> String`.

- [ ] **Step 1: Write the failing test** — `tests/skills_model_test.rs`

```rust
use auto_subtitle::skills::model::*;

fn skill(id: &str, name: &str) -> Skill {
    Skill {
        id: id.into(),
        name: name.into(),
        description: String::new(),
        system_prompt: "Translate carefully.".into(),
        glossary: vec![],
    }
}

fn entry(s: &str, t: &str) -> GlossaryEntry {
    GlossaryEntry { source: s.into(), target: t.into() }
}

#[test]
fn builtin_detection_and_constants() {
    assert!(skill("builtin:math", "x").is_builtin());
    assert!(!skill("abc", "x").is_builtin());
    assert_eq!(NO_SKILL, "none");
    assert_eq!(BUILTIN_PREFIX, "builtin:");
}

#[test]
fn normalized_trims_everything() {
    let s = Skill {
        id: "1".into(),
        name: "  ML  ".into(),
        description: " d ".into(),
        system_prompt: "  p  ".into(),
        glossary: vec![entry(" relu ", " (x) ")],
    }
    .normalized();
    assert_eq!(s.name, "ML");
    assert_eq!(s.description, "d");
    assert_eq!(s.system_prompt, "p");
    assert_eq!(s.glossary, vec![entry("relu", "(x)")]);
}

#[test]
fn validation_rules() {
    let others = vec![skill("builtin:math", "Toán học"), skill("u1", "Của tôi")];
    assert!(skill("new", "ML").validate(&others).is_ok());
    assert_eq!(skill("new", " ").validate(&others).unwrap_err(), "Tên skill không được để trống");
    assert_eq!(skill("new", "toán HỌC").validate(&others).unwrap_err(), "Tên skill đã tồn tại");

    let mut s = skill("new", "ML");
    s.system_prompt = "  ".into();
    assert_eq!(s.validate(&others).unwrap_err(), "System prompt không được để trống");
    s.system_prompt = "a".repeat(MAX_PROMPT_CHARS + 1);
    assert_eq!(s.validate(&others).unwrap_err(), "System prompt tối đa 8000 ký tự");

    let mut s = skill("new", "ML");
    s.glossary = (0..=MAX_GLOSSARY).map(|i| entry(&format!("t{i}"), "")).collect();
    assert_eq!(s.validate(&others).unwrap_err(), "Glossary tối đa 1000 thuật ngữ");

    let mut s = skill("new", "ML");
    s.glossary = vec![entry(" ", "x")];
    assert_eq!(s.validate(&others).unwrap_err(), "Thuật ngữ gốc không được để trống");
    s.glossary = vec![entry("ReLU", "a"), entry("relu", "b")];
    assert_eq!(s.validate(&others).unwrap_err(), "Thuật ngữ bị trùng: relu");
}

#[test]
fn prompt_length_counts_chars_not_bytes() {
    let mut s = skill("new", "ML");
    s.system_prompt = "đ".repeat(MAX_PROMPT_CHARS);
    assert!(s.validate(&[]).is_ok());
}

#[test]
fn unique_name_appends_suffix() {
    let taken = vec!["A".to_string(), "a (2)".to_string()];
    assert_eq!(unique_name("B", &taken), "B");
    assert_eq!(unique_name("A", &taken), "A (3)");
}

#[test]
fn serde_defaults() {
    let s: Skill = serde_json::from_str(r#"{"id":"x","name":"n","system_prompt":"p"}"#).unwrap();
    assert!(s.glossary.is_empty() && s.description.is_empty());
    let e: GlossaryEntry = serde_json::from_str(r#"{"source":"softmax"}"#).unwrap();
    assert_eq!(e.target, "");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test skills_model_test`
Expected: compile error `could not find skills in auto_subtitle`.

- [ ] **Step 3: Implement** — `src/skills/model.rs`

```rust
use serde::{Deserialize, Serialize};

pub const NO_SKILL: &str = "none";
pub const BUILTIN_PREFIX: &str = "builtin:";
pub const MAX_PROMPT_CHARS: usize = 8000;
pub const MAX_GLOSSARY: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlossaryEntry {
    pub source: String,
    #[serde(default)]
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Skill {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub system_prompt: String,
    #[serde(default)]
    pub glossary: Vec<GlossaryEntry>,
}

impl Skill {
    pub fn is_builtin(&self) -> bool {
        self.id.starts_with(BUILTIN_PREFIX)
    }

    pub fn normalized(mut self) -> Self {
        self.name = self.name.trim().to_string();
        self.description = self.description.trim().to_string();
        self.system_prompt = self.system_prompt.trim().to_string();
        for e in &mut self.glossary {
            e.source = e.source.trim().to_string();
            e.target = e.target.trim().to_string();
        }
        self
    }

    pub fn validate(&self, others: &[Skill]) -> Result<(), String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Tên skill không được để trống".into());
        }
        let lower = name.to_lowercase();
        if others
            .iter()
            .any(|o| o.id != self.id && o.name.trim().to_lowercase() == lower)
        {
            return Err("Tên skill đã tồn tại".into());
        }
        if self.system_prompt.trim().is_empty() {
            return Err("System prompt không được để trống".into());
        }
        if self.system_prompt.chars().count() > MAX_PROMPT_CHARS {
            return Err("System prompt tối đa 8000 ký tự".into());
        }
        if self.glossary.len() > MAX_GLOSSARY {
            return Err("Glossary tối đa 1000 thuật ngữ".into());
        }
        let mut seen = std::collections::HashSet::new();
        for e in &self.glossary {
            let src = e.source.trim();
            if src.is_empty() {
                return Err("Thuật ngữ gốc không được để trống".into());
            }
            if !seen.insert(src.to_lowercase()) {
                return Err(format!("Thuật ngữ bị trùng: {src}"));
            }
        }
        Ok(())
    }
}

pub fn unique_name(base: &str, taken: &[String]) -> String {
    let is_taken = |n: &str| taken.iter().any(|t| t.trim().to_lowercase() == n.to_lowercase());
    if !is_taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|i| format!("{base} ({i})"))
        .find(|n| !is_taken(n))
        .expect("infinite iterator")
}
```

`src/skills/mod.rs` (later tasks extend it):

```rust
pub mod model;

pub use model::{GlossaryEntry, Skill, BUILTIN_PREFIX, NO_SKILL};
```

`src/lib.rs` — add `pub mod skills;` after `pub mod settings;`.

- [ ] **Step 4: Run tests**

Run: `cargo test --test skills_model_test`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/skills tests/skills_model_test.rs
git commit -m "feat(skills): add skill model and validation"
```

---

### Task 2: Embedded presets

**Files:**
- Create: `src/skills/presets.rs`, `src/skills/presets/{math,ai-ml,programming,physics,chemistry,biology}.json`
- Modify: `src/skills/mod.rs`
- Test: `tests/skills_presets_test.rs`

**Interfaces:**
- Produces: `skills::presets() -> &'static [Skill]` (order: math, ai-ml, programming, physics, chemistry, biology).

- [ ] **Step 1: Write the failing test** — `tests/skills_presets_test.rs`

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test skills_presets_test`
Expected: compile error `cannot find function presets`.

- [ ] **Step 3: Implement** — `src/skills/presets.rs`

```rust
use std::sync::OnceLock;

use super::model::Skill;

const SOURCES: [(&str, &str); 6] = [
    ("math", include_str!("presets/math.json")),
    ("ai-ml", include_str!("presets/ai-ml.json")),
    ("programming", include_str!("presets/programming.json")),
    ("physics", include_str!("presets/physics.json")),
    ("chemistry", include_str!("presets/chemistry.json")),
    ("biology", include_str!("presets/biology.json")),
];

pub fn presets() -> &'static [Skill] {
    static PRESETS: OnceLock<Vec<Skill>> = OnceLock::new();
    PRESETS.get_or_init(|| {
        SOURCES
            .iter()
            .map(|(slug, json)| {
                serde_json::from_str(json).unwrap_or_else(|e| panic!("invalid preset {slug}: {e}"))
            })
            .collect()
    })
}
```

`src/skills/mod.rs`:

```rust
pub mod model;
pub mod presets;

pub use model::{GlossaryEntry, Skill, BUILTIN_PREFIX, NO_SKILL};
pub use presets::presets;
```

`src/skills/presets/math.json`:

```json
{
  "id": "builtin:math",
  "name": "Toán học",
  "description": "Giải tích, đại số tuyến tính, xác suất — giữ nguyên ký hiệu và công thức",
  "system_prompt": "You are translating subtitles of an educational lecture in mathematics from {source_lang} into {target_lang}. Use the standard terminology a {target_lang}-speaking university lecturer in mathematics would use. Keep sentences short and natural for subtitles. Do not add explanations.\n\nRules:\n- Keep formulas, symbols, variable names and LaTeX exactly as written.\n- When a formula is read aloud (e.g. \"x squared plus one\"), render it in natural {target_lang} mathematical phrasing.\n- Prefer the terminology used in {target_lang} mathematics textbooks.",
  "glossary": [
    { "source": "derivative", "target": "đạo hàm" },
    { "source": "partial derivative", "target": "đạo hàm riêng" },
    { "source": "integral", "target": "tích phân" },
    { "source": "limit", "target": "giới hạn" },
    { "source": "function", "target": "hàm số" },
    { "source": "continuous", "target": "liên tục" },
    { "source": "differentiable", "target": "khả vi" },
    { "source": "convergence", "target": "sự hội tụ" },
    { "source": "sequence", "target": "dãy số" },
    { "source": "series", "target": "chuỗi" },
    { "source": "polynomial", "target": "đa thức" },
    { "source": "equation", "target": "phương trình" },
    { "source": "inequality", "target": "bất phương trình" },
    { "source": "matrix", "target": "ma trận" },
    { "source": "vector", "target": "vectơ" },
    { "source": "vector space", "target": "không gian vectơ" },
    { "source": "linear transformation", "target": "phép biến đổi tuyến tính" },
    { "source": "determinant", "target": "định thức" },
    { "source": "eigenvalue", "target": "trị riêng" },
    { "source": "eigenvector", "target": "vectơ riêng" },
    { "source": "dot product", "target": "tích vô hướng" },
    { "source": "cross product", "target": "tích có hướng" },
    { "source": "gradient", "target": "" },
    { "source": "probability", "target": "xác suất" },
    { "source": "random variable", "target": "biến ngẫu nhiên" },
    { "source": "expected value", "target": "kỳ vọng" },
    { "source": "variance", "target": "phương sai" },
    { "source": "standard deviation", "target": "độ lệch chuẩn" },
    { "source": "theorem", "target": "định lý" },
    { "source": "lemma", "target": "bổ đề" },
    { "source": "proof", "target": "chứng minh" },
    { "source": "empty set", "target": "tập rỗng" }
  ]
}
```

`src/skills/presets/ai-ml.json`:

```json
{
  "id": "builtin:ai-ml",
  "name": "AI / Machine Learning",
  "description": "AI, Machine Learning, Deep Learning — giữ thuật ngữ tiếng Anh phổ biến",
  "system_prompt": "You are translating subtitles of an educational lecture in artificial intelligence and machine learning from {source_lang} into {target_lang}. Use the standard terminology a {target_lang}-speaking university lecturer in artificial intelligence and machine learning would use. Keep sentences short and natural for subtitles. Do not add explanations.\n\nRules:\n- Keep model names, library names, dataset names and code exactly as written.\n- Keep widely used English terms (transformer, embedding, token, epoch) when the {target_lang} AI community normally uses them untranslated.\n- Translate explanations naturally; do not translate word by word.",
  "glossary": [
    { "source": "machine learning", "target": "học máy" },
    { "source": "deep learning", "target": "học sâu" },
    { "source": "neural network", "target": "mạng nơ-ron" },
    { "source": "convolutional neural network", "target": "mạng nơ-ron tích chập" },
    { "source": "supervised learning", "target": "học có giám sát" },
    { "source": "unsupervised learning", "target": "học không giám sát" },
    { "source": "reinforcement learning", "target": "học tăng cường" },
    { "source": "overfitting", "target": "quá khớp" },
    { "source": "underfitting", "target": "chưa khớp" },
    { "source": "loss function", "target": "hàm mất mát" },
    { "source": "gradient descent", "target": "hạ gradient" },
    { "source": "learning rate", "target": "tốc độ học" },
    { "source": "backpropagation", "target": "lan truyền ngược" },
    { "source": "activation function", "target": "hàm kích hoạt" },
    { "source": "weight", "target": "trọng số" },
    { "source": "bias", "target": "" },
    { "source": "training set", "target": "tập huấn luyện" },
    { "source": "validation set", "target": "tập kiểm định" },
    { "source": "test set", "target": "tập kiểm tra" },
    { "source": "dataset", "target": "tập dữ liệu" },
    { "source": "feature", "target": "đặc trưng" },
    { "source": "label", "target": "nhãn" },
    { "source": "classification", "target": "phân loại" },
    { "source": "regression", "target": "hồi quy" },
    { "source": "clustering", "target": "phân cụm" },
    { "source": "regularization", "target": "chính quy hóa" },
    { "source": "inference", "target": "suy luận" },
    { "source": "fine-tuning", "target": "tinh chỉnh" },
    { "source": "large language model", "target": "mô hình ngôn ngữ lớn" },
    { "source": "attention mechanism", "target": "cơ chế chú ý" },
    { "source": "transformer", "target": "" },
    { "source": "embedding", "target": "" },
    { "source": "token", "target": "" },
    { "source": "epoch", "target": "" },
    { "source": "dropout", "target": "" },
    { "source": "batch size", "target": "kích thước batch" }
  ]
}
```

`src/skills/presets/programming.json`:

```json
{
  "id": "builtin:programming",
  "name": "Lập trình / CS",
  "description": "Lập trình, khoa học máy tính — không dịch code, tên hàm, lệnh",
  "system_prompt": "You are translating subtitles of an educational lecture in computer science and programming from {source_lang} into {target_lang}. Use the standard terminology a {target_lang}-speaking university lecturer in computer science and programming would use. Keep sentences short and natural for subtitles. Do not add explanations.\n\nRules:\n- Never translate code, identifiers, function names, keywords, commands, file paths or URLs.\n- Keep product and tool names (git, Docker, Python, React) as written.\n- Translate the explanation around the code naturally.",
  "glossary": [
    { "source": "variable", "target": "biến" },
    { "source": "function", "target": "hàm" },
    { "source": "array", "target": "mảng" },
    { "source": "string", "target": "chuỗi" },
    { "source": "loop", "target": "vòng lặp" },
    { "source": "recursion", "target": "đệ quy" },
    { "source": "algorithm", "target": "thuật toán" },
    { "source": "data structure", "target": "cấu trúc dữ liệu" },
    { "source": "linked list", "target": "danh sách liên kết" },
    { "source": "hash table", "target": "bảng băm" },
    { "source": "stack", "target": "ngăn xếp" },
    { "source": "queue", "target": "hàng đợi" },
    { "source": "tree", "target": "cây" },
    { "source": "graph", "target": "đồ thị" },
    { "source": "pointer", "target": "con trỏ" },
    { "source": "class", "target": "lớp" },
    { "source": "object", "target": "đối tượng" },
    { "source": "inheritance", "target": "kế thừa" },
    { "source": "interface", "target": "giao diện" },
    { "source": "exception", "target": "ngoại lệ" },
    { "source": "thread", "target": "luồng" },
    { "source": "compile", "target": "biên dịch" },
    { "source": "compiler", "target": "trình biên dịch" },
    { "source": "interpreter", "target": "trình thông dịch" },
    { "source": "debug", "target": "gỡ lỗi" },
    { "source": "time complexity", "target": "độ phức tạp thời gian" },
    { "source": "boolean", "target": "" },
    { "source": "API", "target": "" },
    { "source": "framework", "target": "" },
    { "source": "git", "target": "" }
  ]
}
```

`src/skills/presets/physics.json`:

```json
{
  "id": "builtin:physics",
  "name": "Vật lý",
  "description": "Cơ học, điện từ, sóng, lượng tử — giữ đơn vị SI và ký hiệu",
  "system_prompt": "You are translating subtitles of an educational lecture in physics from {source_lang} into {target_lang}. Use the standard terminology a {target_lang}-speaking university lecturer in physics would use. Keep sentences short and natural for subtitles. Do not add explanations.\n\nRules:\n- Keep SI units, unit symbols, physical constants and formulas exactly as written.\n- Keep names of laws and scientists in their usual {target_lang} form.",
  "glossary": [
    { "source": "velocity", "target": "vận tốc" },
    { "source": "acceleration", "target": "gia tốc" },
    { "source": "force", "target": "lực" },
    { "source": "mass", "target": "khối lượng" },
    { "source": "momentum", "target": "động lượng" },
    { "source": "angular momentum", "target": "mô-men động lượng" },
    { "source": "torque", "target": "mô-men lực" },
    { "source": "kinetic energy", "target": "động năng" },
    { "source": "potential energy", "target": "thế năng" },
    { "source": "friction", "target": "ma sát" },
    { "source": "gravity", "target": "trọng lực" },
    { "source": "gravitational force", "target": "lực hấp dẫn" },
    { "source": "oscillation", "target": "dao động" },
    { "source": "wavelength", "target": "bước sóng" },
    { "source": "frequency", "target": "tần số" },
    { "source": "amplitude", "target": "biên độ" },
    { "source": "interference", "target": "giao thoa" },
    { "source": "diffraction", "target": "nhiễu xạ" },
    { "source": "refraction", "target": "khúc xạ" },
    { "source": "electric field", "target": "điện trường" },
    { "source": "magnetic field", "target": "từ trường" },
    { "source": "electric current", "target": "dòng điện" },
    { "source": "voltage", "target": "hiệu điện thế" },
    { "source": "resistance", "target": "điện trở" },
    { "source": "capacitor", "target": "tụ điện" },
    { "source": "thermodynamics", "target": "nhiệt động lực học" },
    { "source": "quantum mechanics", "target": "cơ học lượng tử" },
    { "source": "entropy", "target": "" },
    { "source": "photon", "target": "" }
  ]
}
```

`src/skills/presets/chemistry.json`:

```json
{
  "id": "builtin:chemistry",
  "name": "Hóa học",
  "description": "Hóa đại cương, vô cơ, hữu cơ — giữ công thức hóa học",
  "system_prompt": "You are translating subtitles of an educational lecture in chemistry from {source_lang} into {target_lang}. Use the standard terminology a {target_lang}-speaking university lecturer in chemistry would use. Keep sentences short and natural for subtitles. Do not add explanations.\n\nRules:\n- Keep chemical formulas, equations, element symbols and IUPAC names exactly as written.\n- Use the element and compound names standard in {target_lang} chemistry textbooks.",
  "glossary": [
    { "source": "atom", "target": "nguyên tử" },
    { "source": "molecule", "target": "phân tử" },
    { "source": "isotope", "target": "đồng vị" },
    { "source": "periodic table", "target": "bảng tuần hoàn" },
    { "source": "valence electron", "target": "electron hóa trị" },
    { "source": "electronegativity", "target": "độ âm điện" },
    { "source": "chemical bond", "target": "liên kết hóa học" },
    { "source": "covalent bond", "target": "liên kết cộng hóa trị" },
    { "source": "ionic bond", "target": "liên kết ion" },
    { "source": "oxidation", "target": "sự oxi hóa" },
    { "source": "reduction", "target": "sự khử" },
    { "source": "catalyst", "target": "chất xúc tác" },
    { "source": "reaction rate", "target": "tốc độ phản ứng" },
    { "source": "chemical equilibrium", "target": "cân bằng hóa học" },
    { "source": "acid", "target": "axit" },
    { "source": "salt", "target": "muối" },
    { "source": "concentration", "target": "nồng độ" },
    { "source": "mole", "target": "mol" },
    { "source": "molar mass", "target": "khối lượng mol" },
    { "source": "solvent", "target": "dung môi" },
    { "source": "precipitate", "target": "kết tủa" },
    { "source": "organic compound", "target": "hợp chất hữu cơ" },
    { "source": "hydrocarbon", "target": "hiđrocacbon" },
    { "source": "polymer", "target": "polime" },
    { "source": "enthalpy", "target": "entanpi" },
    { "source": "ion", "target": "" },
    { "source": "electron", "target": "" },
    { "source": "proton", "target": "" },
    { "source": "neutron", "target": "" }
  ]
}
```

`src/skills/presets/biology.json`:

```json
{
  "id": "builtin:biology",
  "name": "Sinh học",
  "description": "Tế bào, di truyền, tiến hóa, sinh thái — giữ tên khoa học Latin",
  "system_prompt": "You are translating subtitles of an educational lecture in biology from {source_lang} into {target_lang}. Use the standard terminology a {target_lang}-speaking university lecturer in biology would use. Keep sentences short and natural for subtitles. Do not add explanations.\n\nRules:\n- Keep Latin species names, gene names and abbreviations (DNA, RNA, ATP) exactly as written.\n- Use the terminology standard in {target_lang} biology textbooks.",
  "glossary": [
    { "source": "cell", "target": "tế bào" },
    { "source": "cell membrane", "target": "màng tế bào" },
    { "source": "nucleus", "target": "nhân tế bào" },
    { "source": "mitochondria", "target": "ti thể" },
    { "source": "tissue", "target": "mô" },
    { "source": "organism", "target": "sinh vật" },
    { "source": "gene", "target": "gen" },
    { "source": "allele", "target": "alen" },
    { "source": "chromosome", "target": "nhiễm sắc thể" },
    { "source": "genotype", "target": "kiểu gen" },
    { "source": "phenotype", "target": "kiểu hình" },
    { "source": "heredity", "target": "di truyền" },
    { "source": "mutation", "target": "đột biến" },
    { "source": "mitosis", "target": "nguyên phân" },
    { "source": "meiosis", "target": "giảm phân" },
    { "source": "enzyme", "target": "enzim" },
    { "source": "hormone", "target": "hoocmôn" },
    { "source": "neuron", "target": "nơ-ron" },
    { "source": "immune system", "target": "hệ miễn dịch" },
    { "source": "photosynthesis", "target": "quang hợp" },
    { "source": "cellular respiration", "target": "hô hấp tế bào" },
    { "source": "natural selection", "target": "chọn lọc tự nhiên" },
    { "source": "evolution", "target": "tiến hóa" },
    { "source": "species", "target": "loài" },
    { "source": "ecosystem", "target": "hệ sinh thái" },
    { "source": "bacteria", "target": "vi khuẩn" },
    { "source": "DNA", "target": "" },
    { "source": "RNA", "target": "" },
    { "source": "protein", "target": "" },
    { "source": "virus", "target": "" }
  ]
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test skills_presets_test`
Expected: 1 passed.

- [ ] **Step 5: Commit**

```bash
git add src/skills tests/skills_presets_test.rs
git commit -m "feat(skills): embed six education presets"
```

---

### Task 3: Prompt rendering

**Files:**
- Create: `src/skills/prompt.rs`
- Modify: `src/skills/mod.rs`
- Test: `tests/skills_prompt_test.rs`

**Interfaces:**
- Consumes: `Skill`, `GlossaryEntry`, `SubtitleSegment`.
- Produces: `substitute(template: &str, source_lang: &str, target_lang: &str) -> String`; `glossary_for_batch<'a>(glossary: &'a [GlossaryEntry], batch: &[SubtitleSegment]) -> Vec<&'a GlossaryEntry>`; `glossary_applies(skill: &Skill, target_lang: &str) -> bool`; `render_system_prompt(skill: &Skill, batch: &[SubtitleSegment], source_lang: &str, target_lang: &str) -> String`; consts `MAX_TERMS_PER_BATCH = 100`, `GLOSSARY_HEADER`.

- [ ] **Step 1: Write the failing test** — `tests/skills_prompt_test.rs`

```rust
use auto_subtitle::skills::model::{GlossaryEntry, Skill};
use auto_subtitle::skills::prompt::*;
use auto_subtitle::subtitle::model::SubtitleSegment as S;

fn e(s: &str, t: &str) -> GlossaryEntry {
    GlossaryEntry { source: s.into(), target: t.into() }
}

fn batch(lines: &[&str]) -> Vec<S> {
    lines.iter().enumerate().map(|(i, l)| S::new(i + 1, 0, 1, *l)).collect()
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
    assert_eq!(substitute("{source_lang}->{target_lang} {x}", "en", "vi"), "en->vi {x}");
    assert_eq!(substitute("{source_lang}", "auto", "vi"), "the source language");
    assert_eq!(substitute("{source_lang}", "AUTO", "vi"), "the source language");
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
    let b = batch(&["The function uses ReLU, then Gradient Descent", "đạo hàm của f", "これは微分です"]);
    let found: Vec<&str> = glossary_for_batch(&g, &b).iter().map(|x| x.source.as_str()).collect();
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
    let found: Vec<&str> = glossary_for_batch(&g, &batch(&["aa ab abc d"])).iter().map(|x| x.source.as_str()).collect();
    assert_eq!(found, vec!["abc d", "aa", "ab"]);
}

#[test]
fn render_with_and_without_glossary() {
    let s = skill("u1", vec![e("softmax", ""), e("logit", "logit thô"), e("absent", "z")]);
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test skills_prompt_test`
Expected: compile error `could not find prompt in skills`.

- [ ] **Step 3: Implement** — `src/skills/prompt.rs`

```rust
use crate::subtitle::model::SubtitleSegment;

use super::model::{GlossaryEntry, Skill};

pub const MAX_TERMS_PER_BATCH: usize = 100;
pub const GLOSSARY_HEADER: &str =
    "Glossary — use these translations exactly; \"(keep)\" means keep the original term unchanged:";

pub fn substitute(template: &str, source_lang: &str, target_lang: &str) -> String {
    let src = if source_lang.trim().eq_ignore_ascii_case("auto") {
        "the source language"
    } else {
        source_lang
    };
    template.replace("{source_lang}", src).replace("{target_lang}", target_lang)
}

fn term_matches(haystack: &str, term: &str) -> bool {
    if !term.is_ascii() {
        return haystack.contains(term);
    }
    haystack.match_indices(term).any(|(i, m)| {
        let before_ok = haystack[..i]
            .chars()
            .next_back()
            .map_or(true, |c| !c.is_ascii_alphanumeric());
        let after_ok = haystack[i + m.len()..]
            .chars()
            .next()
            .map_or(true, |c| !c.is_ascii_alphanumeric());
        before_ok && after_ok
    })
}

pub fn glossary_for_batch<'a>(
    glossary: &'a [GlossaryEntry],
    batch: &[SubtitleSegment],
) -> Vec<&'a GlossaryEntry> {
    let haystack = batch
        .iter()
        .map(|s| s.original_text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    let mut found: Vec<&GlossaryEntry> = glossary
        .iter()
        .filter(|e| {
            let term = e.source.trim().to_lowercase();
            !term.is_empty() && term_matches(&haystack, &term)
        })
        .collect();
    found.sort_by(|a, b| {
        b.source
            .chars()
            .count()
            .cmp(&a.source.chars().count())
            .then_with(|| a.source.cmp(&b.source))
    });
    found.truncate(MAX_TERMS_PER_BATCH);
    found
}

pub fn glossary_applies(skill: &Skill, target_lang: &str) -> bool {
    !skill.is_builtin() || target_lang.trim().to_lowercase().starts_with("vi")
}

pub fn render_system_prompt(
    skill: &Skill,
    batch: &[SubtitleSegment],
    source_lang: &str,
    target_lang: &str,
) -> String {
    let mut out = substitute(&skill.system_prompt, source_lang, target_lang);
    if glossary_applies(skill, target_lang) {
        let terms = glossary_for_batch(&skill.glossary, batch);
        if !terms.is_empty() {
            out.push_str("\n\n");
            out.push_str(GLOSSARY_HEADER);
            for t in terms {
                let target = if t.target.trim().is_empty() { "(keep)" } else { t.target.as_str() };
                out.push_str(&format!("\n- {} → {}", t.source, target));
            }
        }
    }
    out
}
```

`src/skills/mod.rs` — add `pub mod prompt;` after `pub mod presets;`.

Note on `ordering_and_cap`: sort is length-desc then alphabetical, so `t000`… are all length 4; alphabetical keeps `t000`–`t099`.

- [ ] **Step 4: Run tests**

Run: `cargo test --test skills_prompt_test`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```bash
git add src/skills tests/skills_prompt_test.rs
git commit -m "feat(skills): render per-batch system prompt with filtered glossary"
```

---

### Task 4: Skill file import/export

**Files:**
- Create: `src/skills/io.rs`
- Modify: `src/skills/mod.rs`
- Test: `tests/skills_io_test.rs`

**Interfaces:**
- Produces: `export_json(skill: &Skill) -> String`; `parse_import(text: &str) -> Result<Skill, String>` (returns skill with empty `id`, normalized, **not** validated); `safe_file_name(name: &str) -> String`; consts `FORMAT = "echo-skill"`, `FORMAT_VERSION = 1`, `MAX_IMPORT_BYTES = 1_048_576`.

- [ ] **Step 1: Write the failing test** — `tests/skills_io_test.rs`

```rust
use auto_subtitle::skills::io::*;
use auto_subtitle::skills::model::{GlossaryEntry, Skill};

fn sample() -> Skill {
    Skill {
        id: "abc".into(),
        name: "ML của tôi".into(),
        description: "d".into(),
        system_prompt: "p {target_lang}".into(),
        glossary: vec![GlossaryEntry { source: "relu".into(), target: "".into() }],
    }
}

#[test]
fn export_shape_has_no_id() {
    let v: serde_json::Value = serde_json::from_str(&export_json(&sample())).unwrap();
    assert_eq!(v["format"], "echo-skill");
    assert_eq!(v["version"], 1);
    assert_eq!(v["skill"]["name"], "ML của tôi");
    assert!(v["skill"].get("id").is_none());
}

#[test]
fn roundtrip_minus_id() {
    let back = parse_import(&export_json(&sample())).unwrap();
    assert_eq!(back, Skill { id: String::new(), ..sample() });
}

#[test]
fn import_errors() {
    assert_eq!(
        parse_import(r#"{"format":"other","version":1,"skill":{}}"#).unwrap_err(),
        "File không phải skill Echo"
    );
    assert_eq!(
        parse_import(r#"{"format":"echo-skill","version":2,"skill":{}}"#).unwrap_err(),
        "Phiên bản skill không hỗ trợ: 2"
    );
    assert!(parse_import("{oops").unwrap_err().starts_with("File skill không hợp lệ:"));
    assert!(parse_import(r#"{"format":"echo-skill","version":1,"skill":{"name":1}}"#)
        .unwrap_err()
        .starts_with("File skill không hợp lệ:"));
    let big = format!(r#"{{"format":"echo-skill","version":1,"pad":"{}"}}"#, "x".repeat(MAX_IMPORT_BYTES));
    assert_eq!(parse_import(&big).unwrap_err(), "File skill quá lớn (tối đa 1 MB)");
}

#[test]
fn import_trims_fields() {
    let s = parse_import(
        r#"{"format":"echo-skill","version":1,"skill":{"name":"  X ","system_prompt":" p ","glossary":[{"source":" a ","target":" b "}]}}"#,
    )
    .unwrap();
    assert_eq!(s.name, "X");
    assert_eq!(s.glossary[0].source, "a");
}

#[test]
fn file_names_are_safe() {
    assert_eq!(safe_file_name("AI / ML: v2?"), "AI _ ML_ v2_.echo-skill.json");
    assert_eq!(safe_file_name("Toán học"), "Toán học.echo-skill.json");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test skills_io_test`
Expected: compile error `could not find io in skills`.

- [ ] **Step 3: Implement** — `src/skills/io.rs`

```rust
use serde::{Deserialize, Serialize};

use super::model::{GlossaryEntry, Skill};

pub const FORMAT: &str = "echo-skill";
pub const FORMAT_VERSION: u64 = 1;
pub const MAX_IMPORT_BYTES: usize = 1_048_576;

#[derive(Serialize, Deserialize)]
struct SkillBody {
    name: String,
    #[serde(default)]
    description: String,
    system_prompt: String,
    #[serde(default)]
    glossary: Vec<GlossaryEntry>,
}

#[derive(Serialize, Deserialize)]
struct SkillFile {
    format: String,
    version: u64,
    skill: SkillBody,
}

pub fn export_json(skill: &Skill) -> String {
    let file = SkillFile {
        format: FORMAT.into(),
        version: FORMAT_VERSION,
        skill: SkillBody {
            name: skill.name.clone(),
            description: skill.description.clone(),
            system_prompt: skill.system_prompt.clone(),
            glossary: skill.glossary.clone(),
        },
    };
    serde_json::to_string_pretty(&file).expect("skill serializes")
}

pub fn parse_import(text: &str) -> Result<Skill, String> {
    if text.len() > MAX_IMPORT_BYTES {
        return Err("File skill quá lớn (tối đa 1 MB)".into());
    }
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("File skill không hợp lệ: {e}"))?;
    if value.get("format").and_then(|v| v.as_str()) != Some(FORMAT) {
        return Err("File không phải skill Echo".into());
    }
    match value.get("version").and_then(|v| v.as_u64()) {
        Some(FORMAT_VERSION) => {}
        other => {
            let shown = other.map(|v| v.to_string()).unwrap_or_else(|| "?".into());
            return Err(format!("Phiên bản skill không hỗ trợ: {shown}"));
        }
    }
    let file: SkillFile =
        serde_json::from_value(value).map_err(|e| format!("File skill không hợp lệ: {e}"))?;
    Ok(Skill {
        id: String::new(),
        name: file.skill.name,
        description: file.skill.description,
        system_prompt: file.skill.system_prompt,
        glossary: file.skill.glossary,
    }
    .normalized())
}

pub fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    format!("{}.echo-skill.json", cleaned.trim())
}
```

`src/skills/mod.rs` — add `pub mod io;` after `pub mod model;`... final module list order: `io`, `model`, `presets`, `prompt`.

- [ ] **Step 4: Run tests**

Run: `cargo test --test skills_io_test`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```bash
git add src/skills tests/skills_io_test.rs
git commit -m "feat(skills): echo-skill file import/export"
```

---

### Task 5: Skills in Settings and SettingsStore

**Files:**
- Modify: `src/settings/model.rs`, `src/settings/store.rs`
- Test: `tests/settings_skills_test.rs`

**Interfaces:**
- Consumes: Tasks 1–4; A's `Settings`, `SettingsStore` (private `persist()`, field `settings`).
- Produces: `Settings.skills: Vec<Skill>`, `Settings.default_skill_id: Option<String>`; `SettingsStore` methods:
  - `skills_all(&self) -> Vec<Skill>`
  - `find_skill(&self, q: &str) -> Option<Skill>` (id, then case-insensitive name)
  - `other_skills(&self, id: &str) -> Vec<Skill>` (all skills except `id`)
  - `upsert_skill(&mut self, skill: Skill) -> anyhow::Result<Skill>`
  - `delete_skill(&mut self, id: &str) -> anyhow::Result<()>`
  - `duplicate_skill(&mut self, id: &str) -> anyhow::Result<Skill>`
  - `set_default_skill(&mut self, id: Option<&str>) -> anyhow::Result<()>`
  - `resolve_skill(&self, choice: Option<&str>) -> anyhow::Result<Option<Skill>>`
  - `import_skill_json(&mut self, text: &str) -> anyhow::Result<Skill>`
  - `export_skill_json(&self, id: &str) -> anyhow::Result<(String, String)>` → `(file_name, json)`

- [ ] **Step 1: Write the failing test** — `tests/settings_skills_test.rs`

```rust
use auto_subtitle::settings::secrets::{MemoryStore, SecretsBackend};
use auto_subtitle::settings::{Settings, SettingsStore};
use auto_subtitle::skills::{GlossaryEntry, Skill};

fn open(dir: &std::path::Path) -> SettingsStore {
    SettingsStore::open(dir, Box::new(MemoryStore::default()), SecretsBackend::Keyring).unwrap()
}

fn mine(name: &str) -> Skill {
    Skill {
        id: String::new(),
        name: name.into(),
        description: String::new(),
        system_prompt: "p".into(),
        glossary: vec![GlossaryEntry { source: "a".into(), target: "b".into() }],
    }
}

#[test]
fn defaults_and_old_files_have_no_skills() {
    let s = Settings::with_defaults();
    assert!(s.skills.is_empty() && s.default_skill_id.is_none());
    let old: Settings = serde_json::from_str(r#"{"version":1,"profiles":[]}"#).unwrap();
    assert!(old.skills.is_empty());
}

#[test]
fn skills_all_lists_presets_then_user() {
    let dir = tempfile::tempdir().unwrap();
    let mut st = open(dir.path());
    st.upsert_skill(mine("Của tôi")).unwrap();
    let all = st.skills_all();
    assert_eq!(all.len(), 7);
    assert_eq!(all[0].id, "builtin:math");
    assert_eq!(all[6].name, "Của tôi");
}

#[test]
fn upsert_validates_persists_and_rejects_builtin() {
    let dir = tempfile::tempdir().unwrap();
    let mut st = open(dir.path());
    let s = st.upsert_skill(mine("  ML  ")).unwrap();
    assert!(!s.id.is_empty() && s.name == "ML");
    assert_eq!(open(dir.path()).settings().skills.len(), 1);

    let err = st.upsert_skill(mine("toán học")).unwrap_err().to_string();
    assert_eq!(err, "Tên skill đã tồn tại");

    let mut preset = st.find_skill("builtin:math").unwrap();
    preset.system_prompt = "hack".into();
    assert_eq!(st.upsert_skill(preset).unwrap_err().to_string(), "Không thể sửa skill có sẵn");

    let mut edited = s.clone();
    edited.description = "new".into();
    st.upsert_skill(edited).unwrap();
    assert_eq!(st.settings().skills[0].description, "new");
}

#[test]
fn delete_and_default() {
    let dir = tempfile::tempdir().unwrap();
    let mut st = open(dir.path());
    let s = st.upsert_skill(mine("ML")).unwrap();
    st.set_default_skill(Some(&s.id)).unwrap();
    st.delete_skill(&s.id).unwrap();
    assert_eq!(st.settings().default_skill_id, None);
    assert_eq!(st.delete_skill("builtin:math").unwrap_err().to_string(), "Không thể xóa skill có sẵn");
    st.set_default_skill(Some("builtin:math")).unwrap();
    assert_eq!(st.settings().default_skill_id.as_deref(), Some("builtin:math"));
    assert_eq!(st.set_default_skill(Some("nope")).unwrap_err().to_string(), "Không tìm thấy skill");
    st.set_default_skill(None).unwrap();
    assert_eq!(st.settings().default_skill_id, None);
}

#[test]
fn duplicate_names() {
    let dir = tempfile::tempdir().unwrap();
    let mut st = open(dir.path());
    let a = st.duplicate_skill("builtin:math").unwrap();
    let b = st.duplicate_skill("builtin:math").unwrap();
    assert_eq!(a.name, "Toán học (bản sao)");
    assert_eq!(b.name, "Toán học (bản sao) (2)");
    assert!(!a.is_builtin());
    assert_eq!(a.glossary, st.find_skill("builtin:math").unwrap().glossary);
    assert!(st.duplicate_skill("nope").is_err());
}

#[test]
fn resolve_rules() {
    let dir = tempfile::tempdir().unwrap();
    let mut st = open(dir.path());
    assert_eq!(st.resolve_skill(None).unwrap(), None);
    let s = st.upsert_skill(mine("ML")).unwrap();
    st.set_default_skill(Some(&s.id)).unwrap();
    assert_eq!(st.resolve_skill(None).unwrap().unwrap().id, s.id);
    assert_eq!(st.resolve_skill(Some("none")).unwrap(), None);
    assert_eq!(st.resolve_skill(Some("builtin:ai-ml")).unwrap().unwrap().name, "AI / Machine Learning");
    assert_eq!(st.resolve_skill(Some("ml")).unwrap().unwrap().id, s.id);
    assert_eq!(
        st.resolve_skill(Some("xyz")).unwrap_err().to_string(),
        "Không tìm thấy skill 'xyz'"
    );
}

#[test]
fn missing_default_is_not_an_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.json"),
        r#"{"version":1,"profiles":[],"default_skill_id":"gone"}"#,
    )
    .unwrap();
    let st = open(dir.path());
    assert_eq!(st.resolve_skill(None).unwrap(), None);
}

#[test]
fn import_export() {
    let dir = tempfile::tempdir().unwrap();
    let mut st = open(dir.path());
    let (file_name, json) = st.export_skill_json("builtin:math").unwrap();
    assert_eq!(file_name, "Toán học.echo-skill.json");
    let imported = st.import_skill_json(&json).unwrap();
    assert_eq!(imported.name, "Toán học (2)");
    assert!(!imported.is_builtin());
    let again = st.import_skill_json(&json).unwrap();
    assert_eq!(again.name, "Toán học (3)");
    assert_eq!(
        st.import_skill_json(r#"{"format":"x","version":1}"#).unwrap_err().to_string(),
        "File không phải skill Echo"
    );
    assert!(st.export_skill_json("nope").is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test settings_skills_test`
Expected: compile errors (`no field skills`, `no method upsert_skill`).

- [ ] **Step 3: Implement**

`src/settings/model.rs`:
- Add `use crate::skills::Skill;` at the top.
- Add to `Settings` (after `general`):

```rust
    #[serde(default)]
    pub skills: Vec<Skill>,
    #[serde(default)]
    pub default_skill_id: Option<String>,
```

- In `Settings::with_defaults()` add `skills: vec![],` and `default_skill_id: None,` to the struct literal.

`src/settings/store.rs` — add imports:

```rust
use crate::skills::{self, io as skill_io, model::unique_name, Skill, NO_SKILL};
```

and append inside `impl SettingsStore`:

```rust
    pub fn skills_all(&self) -> Vec<Skill> {
        skills::presets()
            .iter()
            .cloned()
            .chain(self.settings.skills.iter().cloned())
            .collect()
    }

    pub fn find_skill(&self, q: &str) -> Option<Skill> {
        let q = q.trim();
        let all = self.skills_all();
        all.iter().find(|s| s.id == q).cloned().or_else(|| {
            let lower = q.to_lowercase();
            all.into_iter().find(|s| s.name.trim().to_lowercase() == lower)
        })
    }

    pub fn other_skills(&self, id: &str) -> Vec<Skill> {
        self.skills_all().into_iter().filter(|s| s.id != id).collect()
    }

    fn skill_names(&self) -> Vec<String> {
        self.skills_all().into_iter().map(|s| s.name).collect()
    }

    pub fn upsert_skill(&mut self, skill: Skill) -> anyhow::Result<Skill> {
        let mut skill = skill.normalized();
        if skill.is_builtin() {
            anyhow::bail!("Không thể sửa skill có sẵn");
        }
        if skill.id.is_empty() {
            skill.id = uuid::Uuid::new_v4().to_string();
        }
        skill.validate(&self.other_skills(&skill.id)).map_err(anyhow::Error::msg)?;
        match self.settings.skills.iter_mut().find(|s| s.id == skill.id) {
            Some(existing) => *existing = skill.clone(),
            None => self.settings.skills.push(skill.clone()),
        }
        self.persist()?;
        Ok(skill)
    }

    pub fn delete_skill(&mut self, id: &str) -> anyhow::Result<()> {
        if id.starts_with(skills::BUILTIN_PREFIX) {
            anyhow::bail!("Không thể xóa skill có sẵn");
        }
        self.settings.skills.retain(|s| s.id != id);
        if self.settings.default_skill_id.as_deref() == Some(id) {
            self.settings.default_skill_id = None;
        }
        self.persist()
    }

    pub fn duplicate_skill(&mut self, id: &str) -> anyhow::Result<Skill> {
        let source = self
            .skills_all()
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| anyhow!("Không tìm thấy skill"))?;
        let name = unique_name(&format!("{} (bản sao)", source.name), &self.skill_names());
        self.upsert_skill(Skill { id: String::new(), name, ..source })
    }

    pub fn set_default_skill(&mut self, id: Option<&str>) -> anyhow::Result<()> {
        if let Some(id) = id {
            if !self.skills_all().iter().any(|s| s.id == id) {
                anyhow::bail!("Không tìm thấy skill");
            }
        }
        self.settings.default_skill_id = id.map(String::from);
        self.persist()
    }

    pub fn resolve_skill(&self, choice: Option<&str>) -> anyhow::Result<Option<Skill>> {
        match choice {
            None => Ok(self
                .settings
                .default_skill_id
                .as_deref()
                .and_then(|id| self.skills_all().into_iter().find(|s| s.id == id))),
            Some(q) if q.trim() == NO_SKILL => Ok(None),
            Some(q) => self
                .find_skill(q)
                .map(Some)
                .ok_or_else(|| anyhow!("Không tìm thấy skill '{}'", q.trim())),
        }
    }

    pub fn import_skill_json(&mut self, text: &str) -> anyhow::Result<Skill> {
        let mut skill = skill_io::parse_import(text).map_err(anyhow::Error::msg)?;
        skill.name = unique_name(&skill.name, &self.skill_names());
        self.upsert_skill(skill)
    }

    pub fn export_skill_json(&self, id: &str) -> anyhow::Result<(String, String)> {
        let skill = self
            .skills_all()
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| anyhow!("Không tìm thấy skill"))?;
        Ok((skill_io::safe_file_name(&skill.name), skill_io::export_json(&skill)))
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test settings_skills_test --test settings_store_test --test settings_model_test`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/settings tests/settings_skills_test.rs
git commit -m "feat(settings): store user skills, default skill, import/export"
```

---

### Task 6: Skill-aware LlmTranslator and try-run helper

**Files:**
- Modify: `src/translate/llm_translator.rs`, `tests/translator_test.rs`
- Create: `src/skills/try_run.rs`
- Modify: `src/skills/mod.rs`
- Test: `tests/skills_try_test.rs`

**Interfaces:**
- Consumes: `render_system_prompt` (Task 3), `ChatClient`, `LlmError` (A).
- Produces: `LlmTranslator::new(client: Box<dyn ChatClient>, skill: Option<Skill>)`; `#[derive(Debug, Clone, Serialize, PartialEq, Eq)] TrySkillResult { lines: Vec<String>, system_prompt: String, error: Option<String> }`; `const MAX_TRY_LINES: usize = 20`; `async fn try_skill(client: Box<dyn ChatClient>, skill: Skill, others: &[Skill], sample: &str, source_lang: &str, target_lang: &str) -> TrySkillResult`.

Callers in `src/main.rs` and `src-tauri/src/commands.rs` pass `None`, which still compiles with the new type.

- [ ] **Step 1: Write the failing tests**

`tests/translator_test.rs` — replace A's `llm_translator_forwards_prompt_and_system` test with:

```rust
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
```

`tests/skills_try_test.rs`:

```rust
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
        glossary: vec![GlossaryEntry { source: "hello".into(), target: "chào".into() }],
    }
}

#[tokio::test]
async fn translates_sample_and_returns_prompt() {
    let r = try_skill(Box::new(Fixed(Ok("[1] Chào\n[2] Tạm biệt".into()))), skill(), &[], "Hello\n\n  Bye  \n", "auto", "vi").await;
    assert_eq!(r.error, None);
    assert_eq!(r.lines, vec!["Chào", "Tạm biệt"]);
    assert!(r.system_prompt.starts_with("Sys vi") && r.system_prompt.contains("- hello → chào"));
}

#[tokio::test]
async fn sample_limits() {
    let r = try_skill(Box::new(Fixed(Ok(String::new()))), skill(), &[], "  \n ", "auto", "vi").await;
    assert_eq!(r.error.as_deref(), Some("Nhập ít nhất 1 dòng"));
    let many = (0..21).map(|i| format!("l{i}")).collect::<Vec<_>>().join("\n");
    let r = try_skill(Box::new(Fixed(Ok(String::new()))), skill(), &[], &many, "auto", "vi").await;
    assert_eq!(r.error.as_deref(), Some("Tối đa 20 dòng"));
    assert_eq!(MAX_TRY_LINES, 20);
}

#[tokio::test]
async fn invalid_skill_reports_validation_error() {
    let mut s = skill();
    s.name = " ".into();
    let r = try_skill(Box::new(Fixed(Ok(String::new()))), s, &[], "Hello", "auto", "vi").await;
    assert_eq!(r.error.as_deref(), Some("Tên skill không được để trống"));
    assert!(r.lines.is_empty() && r.system_prompt.is_empty());
}

#[tokio::test]
async fn provider_error_keeps_prompt() {
    let r = try_skill(Box::new(Fixed(Err(LlmError::Auth))), skill(), &[], "Hello", "auto", "vi").await;
    assert_eq!(r.error.as_deref(), Some("API key không hợp lệ"));
    assert!(r.lines.is_empty());
    assert!(r.system_prompt.contains("hello → chào"));
}

#[tokio::test]
async fn parse_error_is_reported() {
    let r = try_skill(Box::new(Fixed(Ok("garbage".into()))), skill(), &[], "Hello", "auto", "vi").await;
    assert!(r.error.unwrap().contains("Missing translated line [1]"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test translator_test --test skills_try_test`
Expected: compile errors (type mismatch `Option<Skill>`; `try_run` missing).

- [ ] **Step 3: Implement**

`src/translate/llm_translator.rs` — full replacement:

```rust
use crate::llm::ChatClient;
use crate::skills::prompt::render_system_prompt;
use crate::skills::Skill;
use crate::subtitle::model::SubtitleSegment;
use crate::translate::prompt::{build_translation_prompt, parse_translated_lines};
use crate::translate::Translator;

pub struct LlmTranslator {
    client: Box<dyn ChatClient>,
    skill: Option<Skill>,
}

impl LlmTranslator {
    pub fn new(client: Box<dyn ChatClient>, skill: Option<Skill>) -> Self {
        Self { client, skill }
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
        let system = self
            .skill
            .as_ref()
            .map(|s| render_system_prompt(s, segments, source_lang, target_lang));
        let prompt = build_translation_prompt(segments, source_lang, target_lang);
        let raw = self
            .client
            .chat(system.as_deref(), &prompt)
            .await
            .map_err(anyhow::Error::new)?;
        parse_translated_lines(&raw, segments.len())
    }
}
```

`src/skills/try_run.rs`:

```rust
use serde::Serialize;

use crate::llm::{ChatClient, LlmError};
use crate::subtitle::model::SubtitleSegment;
use crate::translate::{LlmTranslator, Translator};

use super::model::Skill;
use super::prompt::render_system_prompt;

pub const MAX_TRY_LINES: usize = 20;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TrySkillResult {
    pub lines: Vec<String>,
    pub system_prompt: String,
    pub error: Option<String>,
}

fn failed(system_prompt: String, error: String) -> TrySkillResult {
    TrySkillResult { lines: vec![], system_prompt, error: Some(error) }
}

pub async fn try_skill(
    client: Box<dyn ChatClient>,
    skill: Skill,
    others: &[Skill],
    sample: &str,
    source_lang: &str,
    target_lang: &str,
) -> TrySkillResult {
    let skill = skill.normalized();
    if let Err(e) = skill.validate(others) {
        return failed(String::new(), e);
    }
    let lines: Vec<&str> = sample.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if lines.is_empty() {
        return failed(String::new(), "Nhập ít nhất 1 dòng".into());
    }
    if lines.len() > MAX_TRY_LINES {
        return failed(String::new(), "Tối đa 20 dòng".into());
    }
    let segments: Vec<SubtitleSegment> = lines
        .iter()
        .enumerate()
        .map(|(i, l)| SubtitleSegment::new(i + 1, i as u64 * 1000, i as u64 * 1000 + 900, *l))
        .collect();
    let system_prompt = render_system_prompt(&skill, &segments, source_lang, target_lang);
    let translator = LlmTranslator::new(client, Some(skill));
    match translator.translate_batch(&segments, source_lang, target_lang).await {
        Ok(lines) => TrySkillResult { lines, system_prompt, error: None },
        Err(e) => {
            let msg = e
                .chain()
                .find_map(|c| c.downcast_ref::<LlmError>())
                .map(LlmError::user_message)
                .unwrap_or_else(|| format!("{e:#}"));
            failed(system_prompt, msg)
        }
    }
}
```

`src/skills/mod.rs` — final:

```rust
pub mod io;
pub mod model;
pub mod presets;
pub mod prompt;
pub mod try_run;

pub use model::{GlossaryEntry, Skill, BUILTIN_PREFIX, NO_SKILL};
pub use presets::presets;
```

If `SubtitleSegment::new`'s text parameter is `impl Into<String>`, `*l` (`&str`) works; if it is `String`, use `l.to_string()`.

- [ ] **Step 4: Run tests**

Run: `cargo test --test translator_test --test skills_try_test && cargo build --bin auto-subtitle`
Expected: all pass; binary builds.

- [ ] **Step 5: Commit**

```bash
git add src/translate src/skills tests/translator_test.rs tests/skills_try_test.rs
git commit -m "feat(translate): skill-aware LlmTranslator and try_skill helper"
```

---

### Task 7: CLI `--skill` and `--list-skills`

**Files:**
- Modify: `src/cli.rs`, `src/main.rs`
- Test: `tests/cli_test.rs`

**Interfaces:**
- Consumes: `SettingsStore::resolve_skill`, `skills_all`.
- Produces: `Cli.skill: Option<String>`, `Cli.list_skills: bool`; `choose_skill(cli: &Cli, store: Option<&SettingsStore>) -> anyhow::Result<Option<Skill>>`.

- [ ] **Step 1: Write the failing test** — append to `tests/cli_test.rs`:

```rust
use auto_subtitle::settings::secrets::{MemoryStore, SecretsBackend};
use auto_subtitle::settings::SettingsStore;

fn store_in(dir: &std::path::Path) -> SettingsStore {
    SettingsStore::open(dir, Box::new(MemoryStore::default()), SecretsBackend::Keyring).unwrap()
}

#[test]
fn skill_flag_resolution() {
    let dir = tempfile::tempdir().unwrap();
    let mut st = store_in(dir.path());
    assert_eq!(choose_skill(&parse(&[]), Some(&st)).unwrap(), None);
    st.set_default_skill(Some("builtin:math")).unwrap();
    assert_eq!(choose_skill(&parse(&[]), Some(&st)).unwrap().unwrap().id, "builtin:math");
    assert_eq!(choose_skill(&parse(&["--skill", "none"]), Some(&st)).unwrap(), None);
    assert_eq!(
        choose_skill(&parse(&["--skill", "ai / machine learning"]), Some(&st)).unwrap().unwrap().id,
        "builtin:ai-ml"
    );
    assert_eq!(
        choose_skill(&parse(&["--skill", "builtin:biology"]), Some(&st)).unwrap().unwrap().name,
        "Sinh học"
    );
    assert!(choose_skill(&parse(&["--skill", "xyz"]), Some(&st)).is_err());
}

#[test]
fn skill_flag_without_settings() {
    assert_eq!(choose_skill(&parse(&[]), None).unwrap(), None);
    assert_eq!(choose_skill(&parse(&["--skill", "none"]), None).unwrap(), None);
    let err = choose_skill(&parse(&["--skill", "Toán học"]), None).unwrap_err();
    assert_eq!(err.to_string(), "Không đọc được settings.json nên không dùng được --skill");
}

#[test]
fn skill_applies_with_legacy_engine() {
    let dir = tempfile::tempdir().unwrap();
    let st = store_in(dir.path());
    let c = parse(&["--translate-engine", "ollama", "--skill", "builtin:math"]);
    assert!(matches!(choose_translator(&c, Some(st.settings())).unwrap(), TranslatorChoice::Legacy(_)));
    assert_eq!(choose_skill(&c, Some(&st)).unwrap().unwrap().id, "builtin:math");
}

#[test]
fn list_skills_flag_parses() {
    assert!(parse(&["--list-skills"]).list_skills);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test cli_test`
Expected: compile error `cannot find function choose_skill`.

- [ ] **Step 3: Implement**

`src/cli.rs` — add fields after `list_profiles`:

```rust
    /// Translation skill name or id, or "none"
    #[arg(long)]
    pub skill: Option<String>,

    /// Print available translation skills and exit
    #[arg(long)]
    pub list_skills: bool,
```

Append:

```rust
use crate::settings::SettingsStore;
use crate::skills::{Skill, NO_SKILL};

pub fn choose_skill(cli: &Cli, store: Option<&SettingsStore>) -> anyhow::Result<Option<Skill>> {
    match (cli.skill.as_deref().map(str::trim), store) {
        (Some(q), _) if q == NO_SKILL => Ok(None),
        (Some(q), Some(s)) => s.resolve_skill(Some(q)),
        (Some(_), None) => anyhow::bail!("Không đọc được settings.json nên không dùng được --skill"),
        (None, Some(s)) => s.resolve_skill(None),
        (None, None) => Ok(None),
    }
}
```

`src/main.rs`:

1. Add imports `use auto_subtitle::cli::choose_skill;` (extend the existing `cli::{...}` list) and `use auto_subtitle::skills::Skill;`.
2. Add after `print_profiles`:

```rust
fn print_skills() -> anyhow::Result<()> {
    let store = SettingsStore::open_default()?;
    let default = store.settings().default_skill_id.clone();
    for s in store.skills_all() {
        let mark = if default.as_deref() == Some(s.id.as_str()) { "default" } else { "" };
        println!("{} | {} | {} terms | {}", s.name, s.id, s.glossary.len(), mark);
    }
    Ok(())
}
```

3. Change `build_translator` signature to `async fn build_translator(cli: &Cli, store: Option<&SettingsStore>, skill: Option<Skill>) -> anyhow::Result<Box<dyn Translator>>` and replace every `LlmTranslator::new(<client>, None)` inside it with `LlmTranslator::new(<client>, skill.clone())`.
4. In `run_app`, replace the first statement block with:

```rust
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    if raw_args.iter().any(|a| a == "--list-profiles") {
        return print_profiles();
    }
    if raw_args.iter().any(|a| a == "--list-skills") {
        return print_skills();
    }
```

5. Replace `Some(build_translator(&cli, store.as_ref()).await?)` with:

```rust
        let skill = choose_skill(&cli, store.as_ref())?;
        if let Some(s) = &skill {
            eprintln!("Skill: {}", s.name);
        }
        Some(build_translator(&cli, store.as_ref(), skill).await?)
```

- [ ] **Step 4: Run tests**

Run: `cargo test --test cli_test && cargo run --bin auto-subtitle -- --list-skills`
Expected: tests pass; output starts with `Toán học | builtin:math | 32 terms |`.

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/main.rs tests/cli_test.rs
git commit -m "feat(cli): --skill and --list-skills"
```

---

### Task 8: Tauri skill commands and pipeline wiring

**Files:**
- Create: `src-tauri/src/skill_commands.rs`
- Modify: `src-tauri/src/types.rs`, `src-tauri/src/settings_commands.rs`, `src-tauri/src/commands.rs`, `src-tauri/src/main.rs`
- Test: `#[cfg(test)]` in `src-tauri/src/skill_commands.rs` and `src-tauri/src/settings_commands.rs`

**Interfaces:**
- Consumes: Tasks 5–6; A's `build_view`, `lock`-style access to `state.settings`.
- Produces IPC:
  - `save_skill(skill: Skill) -> SettingsView`, `delete_skill(id) -> SettingsView`, `duplicate_skill(id) -> SettingsView`, `set_default_skill(id: Option<String>) -> SettingsView`
  - `import_skill() -> SettingsView` (dialog), `export_skill(id) -> Option<String>` (dialog)
  - `try_skill(skill: Skill, profileId: Option<String>, sample: String, sourceLang: String, targetLang: String) -> TrySkillResult`
  - `SettingsView` gains `presets: Vec<Skill>`, `skills: Vec<Skill>`, `default_skill_id: Option<String>`
  - `PipelineConfig.skill_id: Option<String>` (`#[serde(default)]`)

- [ ] **Step 1: Write the failing test**

Append to the `tests` module in `src-tauri/src/settings_commands.rs`:

```rust
    #[test]
    fn view_includes_presets_and_user_skills() {
        let dir = tempfile::tempdir().unwrap();
        let mut store =
            SettingsStore::open(dir.path(), Box::new(MemoryStore::default()), SecretsBackend::Keyring).unwrap();
        store.duplicate_skill("builtin:math").unwrap();
        store.set_default_skill(Some("builtin:math")).unwrap();
        let v = build_view(&mut store);
        assert_eq!(v.presets.len(), 6);
        assert_eq!(v.skills.len(), 1);
        assert_eq!(v.default_skill_id.as_deref(), Some("builtin:math"));
    }
```

`src-tauri/src/skill_commands.rs` — test module first:

```rust
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
```

Add `pub mod skill_commands;` to `src-tauri/src/main.rs`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p auto-subtitle-desktop`
Expected: compile errors (`presets` field, `skill_log_line`, `skill_id` missing).

- [ ] **Step 3: Implement**

`src-tauri/src/types.rs`:
- Add `use auto_subtitle::skills::Skill;`.
- Add to `PipelineConfig`:

```rust
    #[serde(default)]
    pub skill_id: Option<String>,
```

- Add to `SettingsView` (after `general`):

```rust
    pub presets: Vec<Skill>,
    pub skills: Vec<Skill>,
    pub default_skill_id: Option<String>,
```

`src-tauri/src/settings_commands.rs` — in `build_view`, add to the struct literal:

```rust
        presets: auto_subtitle::skills::presets().to_vec(),
        skills: s.skills.clone(),
        default_skill_id: s.default_skill_id.clone(),
```

(Place these before the fields that move out of `s`, or clone `s` fields as shown so ordering does not matter.) Also make `lock` `pub(crate)` so `skill_commands.rs` can reuse it.

`src-tauri/src/skill_commands.rs` — above the test module:

```rust
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
```

`src-tauri/src/main.rs` — add `use skill_commands::*;` and append to `generate_handler!`: `save_skill, delete_skill, duplicate_skill, set_default_skill, import_skill, export_skill, try_skill`.

`src-tauri/src/commands.rs` (`run_pipeline`, the resolution block added by A):

1. Change the block's binding to `let (translator, stt_key, skill_line): (Option<Box<dyn Translator>>, Option<String>, Option<String>) = {`.
2. Inside the `else` branch (translation enabled), before building the translator, add:

```rust
            let skill = store
                .resolve_skill(config.skill_id.as_deref())
                .map_err(|e| format!("{e:#}"))?;
```

   and change `Some(Box::new(LlmTranslator::new(client, None)) as Box<dyn Translator>)` to:

```rust
            let line = crate::skill_commands::skill_log_line(skill.as_ref());
            (Some(Box::new(LlmTranslator::new(client, skill)) as Box<dyn Translator>), Some(line))
```

   so the `if/else` yields `(Option<Box<dyn Translator>>, Option<String>)`; make the `if config.skip_translate` branch yield `(None, None)` and bind it as `let (translator, skill_line) = if ... { (None, None) } else { ... };`.
3. Return `(translator, stt_key, skill_line)` from the block.
4. Right after the block add:

```rust
    if let Some(line) = skill_line {
        let _ = app.emit("pipeline-log", format!("[{job_id}] {line}"));
    }
```

- [ ] **Step 4: Run tests and build**

Run: `cargo test -p auto-subtitle-desktop && cargo build -p auto-subtitle-desktop`
Expected: pass; build succeeds.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src
git commit -m "feat(desktop): skill commands, try_skill, per-job skill in pipeline"
```

---

### Task 9: Frontend types, IPC, CSV and skill helpers

**Files:**
- Modify: `ui/src/types/index.ts`, `ui/src/lib/ipc.ts`, `ui/src/lib/ipc.test.ts`
- Create: `ui/src/lib/csv.ts`, `ui/src/lib/csv.test.ts`, `ui/src/lib/skills.ts`, `ui/src/lib/skills.test.ts`

**Interfaces:**
- Produces (TS):
  - Types `GlossaryEntry`, `Skill`, `TrySkillResult`; `SettingsView` + `presets`, `skills`, `default_skill_id`; `PipelineConfig.skill_id?: string | null`; `BatchJob.skillId?: string`, `BatchJob.skillUsed?: string`.
  - `ipc.ts`: `saveSkill(skill)`, `deleteSkill(id)`, `duplicateSkill(id)`, `setDefaultSkill(id: string | null)`, `importSkill()` → `Promise<SettingsView>`; `exportSkill(id) → Promise<string | null>`; `trySkill(skill, profileId: string | null, sample, sourceLang, targetLang) → Promise<TrySkillResult>`.
  - `csv.ts`: `parseGlossaryCsv(text: string): { entries: GlossaryEntry[]; skipped: number }`; `mergeGlossary(existing, incoming): { glossary: GlossaryEntry[]; added: number; updated: number }`.
  - `skills.ts`: `isBuiltinSkill(s: Skill): boolean`; `emptySkill(): Skill`; `allSkills(v: SettingsView): Skill[]`; `skillName(v: SettingsView | null, id: string | null | undefined): string | null`; `jobSkillChoice(job: BatchJob, v: SettingsView | null): { skill_id: string | null; label: string }`.

- [ ] **Step 1: Write the failing tests**

`ui/src/lib/csv.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { parseGlossaryCsv, mergeGlossary } from './csv';

describe('parseGlossaryCsv', () => {
  it('parses rows, skips header and bad rows', () => {
    const r = parseGlossaryCsv('source,target\nrelu,\n"gradient, descent","hạ ""gradient"""\n,orphan\na,b,c\n\nsoftmax');
    expect(r.entries).toEqual([
      { source: 'relu', target: '' },
      { source: 'gradient, descent', target: 'hạ "gradient"' },
      { source: 'softmax', target: '' },
    ]);
    expect(r.skipped).toBe(2);
  });

  it('detects semicolon delimiter and Vietnamese header', () => {
    const r = parseGlossaryCsv('Gốc;Dịch\r\nđạo hàm;derivative\r\n');
    expect(r.entries).toEqual([{ source: 'đạo hàm', target: 'derivative' }]);
    expect(r.skipped).toBe(0);
  });

  it('keeps quoted newlines inside a cell', () => {
    const r = parseGlossaryCsv('"a\nb",c');
    expect(r.entries).toEqual([{ source: 'a\nb', target: 'c' }]);
  });
});

describe('mergeGlossary', () => {
  it('adds new and overwrites case-insensitively', () => {
    const r = mergeGlossary(
      [{ source: 'ReLU', target: 'x' }, { source: 'loss', target: 'mất mát' }],
      [{ source: 'relu', target: '' }, { source: 'epoch', target: '' }, { source: 'EPOCH', target: 'kỷ' }]
    );
    expect(r.glossary).toEqual([
      { source: 'relu', target: '' },
      { source: 'loss', target: 'mất mát' },
      { source: 'EPOCH', target: 'kỷ' },
    ]);
    expect(r.added).toBe(1);
    expect(r.updated).toBe(2);
  });
});
```

`ui/src/lib/skills.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { jobSkillChoice, skillName, allSkills, isBuiltinSkill, emptySkill } from './skills';
import type { BatchJob, SettingsView } from '../types';

const view = {
  presets: [{ id: 'builtin:math', name: 'Toán học', description: '', system_prompt: 'p', glossary: [] }],
  skills: [{ id: 'u1', name: 'Của tôi', description: '', system_prompt: 'p', glossary: [] }],
  default_skill_id: 'builtin:math',
} as unknown as SettingsView;

const job = (skillId?: string) =>
  ({ id: 'j', videoPath: 'v', status: 'queued', progress: null, segments: [], outputVideoPath: null, error: null, skillId }) as BatchJob;

describe('skill helpers', () => {
  it('maps job override to skill_id and label', () => {
    expect(jobSkillChoice(job(), view)).toEqual({ skill_id: null, label: 'Toán học' });
    expect(jobSkillChoice(job('none'), view)).toEqual({ skill_id: 'none', label: 'Không dùng skill' });
    expect(jobSkillChoice(job('u1'), view)).toEqual({ skill_id: 'u1', label: 'Của tôi' });
    expect(jobSkillChoice(job(), { ...view, default_skill_id: null })).toEqual({ skill_id: null, label: 'Không dùng skill' });
    expect(jobSkillChoice(job(), null)).toEqual({ skill_id: null, label: 'Không dùng skill' });
  });

  it('lookups', () => {
    expect(allSkills(view).map((s) => s.id)).toEqual(['builtin:math', 'u1']);
    expect(skillName(view, 'u1')).toBe('Của tôi');
    expect(skillName(view, 'gone')).toBeNull();
    expect(isBuiltinSkill(view.presets[0])).toBe(true);
    expect(emptySkill()).toEqual({ id: '', name: '', description: '', system_prompt: '', glossary: [] });
  });
});
```

Append to `ui/src/lib/ipc.test.ts`:

```ts
import { saveSkill, setDefaultSkill, trySkill, duplicateSkill } from './ipc';

describe('skill IPC', () => {
  it('uses the agreed command names and camelCase args', async () => {
    const skill = { id: '', name: 'X', description: '', system_prompt: 'p', glossary: [] };
    await saveSkill(skill);
    expect(invoke).toHaveBeenCalledWith('save_skill', { skill });
    await setDefaultSkill(null);
    expect(invoke).toHaveBeenCalledWith('set_default_skill', { id: null });
    await duplicateSkill('builtin:math');
    expect(invoke).toHaveBeenCalledWith('duplicate_skill', { id: 'builtin:math' });
    await trySkill(skill, null, 'Hello', 'auto', 'vi');
    expect(invoke).toHaveBeenCalledWith('try_skill', {
      skill,
      profileId: null,
      sample: 'Hello',
      sourceLang: 'auto',
      targetLang: 'vi',
    });
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm --prefix ui test -- src/lib`
Expected: FAIL — modules/exports not found.

- [ ] **Step 3: Implement**

`ui/src/types/index.ts` — append:

```ts
export interface GlossaryEntry {
  source: string;
  target: string;
}

export interface Skill {
  id: string;
  name: string;
  description: string;
  system_prompt: string;
  glossary: GlossaryEntry[];
}

export interface TrySkillResult {
  lines: string[];
  system_prompt: string;
  error: string | null;
}
```

Add to `SettingsView`: `presets: Skill[]; skills: Skill[]; default_skill_id: string | null;`. Add to `PipelineConfig`: `skill_id?: string | null;`. Add to `BatchJob`: `skillId?: string; skillUsed?: string;`.

`ui/src/lib/csv.ts`:

```ts
import type { GlossaryEntry } from '../types';

function detectDelimiter(text: string): ',' | ';' {
  const first = text.split(/\r?\n/).find((l) => l.trim() !== '') ?? '';
  const count = (ch: string) => first.split(ch).length - 1;
  return count(';') > count(',') ? ';' : ',';
}

function parseRows(text: string, delim: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = '';
  let inQuotes = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (inQuotes) {
      if (ch === '"' && text[i + 1] === '"') {
        cell += '"';
        i++;
      } else if (ch === '"') {
        inQuotes = false;
      } else {
        cell += ch;
      }
    } else if (ch === '"') {
      inQuotes = true;
    } else if (ch === delim) {
      row.push(cell);
      cell = '';
    } else if (ch === '\n' || ch === '\r') {
      if (ch === '\r' && text[i + 1] === '\n') i++;
      row.push(cell);
      rows.push(row);
      row = [];
      cell = '';
    } else {
      cell += ch;
    }
  }
  row.push(cell);
  rows.push(row);
  return rows;
}

const HEADERS = [
  ['source', 'target'],
  ['gốc', 'dịch'],
];

export function parseGlossaryCsv(text: string): { entries: GlossaryEntry[]; skipped: number } {
  const rows = parseRows(text, detectDelimiter(text))
    .map((r) => r.map((c) => c.trim()))
    .filter((r) => r.some((c) => c !== ''));
  if (rows.length > 0) {
    const head = rows[0].map((c) => c.toLowerCase());
    if (HEADERS.some((h) => h.length === head.length && h.every((v, i) => v === head[i]))) rows.shift();
  }
  const entries: GlossaryEntry[] = [];
  let skipped = 0;
  for (const r of rows) {
    if (r.length > 2 || !r[0]) {
      skipped++;
      continue;
    }
    entries.push({ source: r[0], target: r[1] ?? '' });
  }
  return { entries, skipped };
}

export function mergeGlossary(
  existing: GlossaryEntry[],
  incoming: GlossaryEntry[]
): { glossary: GlossaryEntry[]; added: number; updated: number } {
  const glossary = existing.map((e) => ({ ...e }));
  let added = 0;
  let updated = 0;
  for (const inc of incoming) {
    const idx = glossary.findIndex((e) => e.source.toLowerCase() === inc.source.toLowerCase());
    if (idx >= 0) {
      glossary[idx] = { ...inc };
      updated++;
    } else {
      glossary.push({ ...inc });
      added++;
    }
  }
  return { glossary, added, updated };
}
```

`ui/src/lib/skills.ts`:

```ts
import type { BatchJob, SettingsView, Skill } from '../types';

export const NO_SKILL = 'none';
export const NO_SKILL_LABEL = 'Không dùng skill';

export const isBuiltinSkill = (s: Skill): boolean => s.id.startsWith('builtin:');

export const emptySkill = (): Skill => ({ id: '', name: '', description: '', system_prompt: '', glossary: [] });

export const allSkills = (v: SettingsView): Skill[] => [...v.presets, ...v.skills];

export function skillName(v: SettingsView | null, id: string | null | undefined): string | null {
  if (!v || !id) return null;
  return allSkills(v).find((s) => s.id === id)?.name ?? null;
}

export function jobSkillChoice(job: BatchJob, v: SettingsView | null): { skill_id: string | null; label: string } {
  if (job.skillId === NO_SKILL) return { skill_id: NO_SKILL, label: NO_SKILL_LABEL };
  if (job.skillId) return { skill_id: job.skillId, label: skillName(v, job.skillId) ?? job.skillId };
  return { skill_id: null, label: skillName(v, v?.default_skill_id) ?? NO_SKILL_LABEL };
}
```

`ui/src/lib/ipc.ts`:
- Extend the type import with `Skill, TrySkillResult`.
- In A's `mockSettings` literal add:

```ts
  presets: [
    { id: 'builtin:math', name: 'Toán học', description: 'Mock preset', system_prompt: 'Math {target_lang}', glossary: [] },
    { id: 'builtin:ai-ml', name: 'AI / Machine Learning', description: 'Mock preset', system_prompt: 'ML {target_lang}', glossary: [] },
  ],
  skills: [],
  default_skill_id: null,
```

- Append:

```ts
export async function saveSkill(skill: Skill): Promise<SettingsView> {
  if (!isTauri()) {
    const id = skill.id || `mock-skill-${Date.now()}`;
    const next = { ...skill, id };
    mockSettings.skills = mockSettings.skills.some((s) => s.id === id)
      ? mockSettings.skills.map((s) => (s.id === id ? next : s))
      : [...mockSettings.skills, next];
    return cloneMock();
  }
  return await invoke<SettingsView>('save_skill', { skill });
}

export async function deleteSkill(id: string): Promise<SettingsView> {
  if (!isTauri()) {
    mockSettings.skills = mockSettings.skills.filter((s) => s.id !== id);
    if (mockSettings.default_skill_id === id) mockSettings.default_skill_id = null;
    return cloneMock();
  }
  return await invoke<SettingsView>('delete_skill', { id });
}

export async function duplicateSkill(id: string): Promise<SettingsView> {
  if (!isTauri()) {
    const src = [...mockSettings.presets, ...mockSettings.skills].find((s) => s.id === id);
    if (src) mockSettings.skills.push({ ...src, id: `mock-skill-${Date.now()}`, name: `${src.name} (bản sao)` });
    return cloneMock();
  }
  return await invoke<SettingsView>('duplicate_skill', { id });
}

export async function setDefaultSkill(id: string | null): Promise<SettingsView> {
  if (!isTauri()) {
    mockSettings.default_skill_id = id;
    return cloneMock();
  }
  return await invoke<SettingsView>('set_default_skill', { id });
}

export async function importSkill(): Promise<SettingsView> {
  if (!isTauri()) return cloneMock();
  return await invoke<SettingsView>('import_skill');
}

export async function exportSkill(id: string): Promise<string | null> {
  if (!isTauri()) return `/mock/${id}.echo-skill.json`;
  return await invoke<string | null>('export_skill', { id });
}

export async function trySkill(
  skill: Skill,
  profileId: string | null,
  sample: string,
  sourceLang: string,
  targetLang: string
): Promise<TrySkillResult> {
  if (!isTauri()) {
    const lines = sample.split('\n').map((l) => l.trim()).filter(Boolean);
    return { lines: lines.map((l) => `[mock] ${l}`), system_prompt: skill.system_prompt, error: null };
  }
  return await invoke<TrySkillResult>('try_skill', { skill, profileId, sample, sourceLang, targetLang });
}
```

- [ ] **Step 4: Run tests**

Run: `npm --prefix ui test -- src/lib`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add ui/src/types/index.ts ui/src/lib
git commit -m "feat(ui): skill types, IPC wrappers, CSV glossary parser and helpers"
```

---

### Task 10: SkillsTab and SettingsModal integration

**Files:**
- Create: `ui/src/components/SkillsTab.tsx`, `ui/src/components/SkillsTab.test.tsx`
- Modify: `ui/src/components/SettingsModal.tsx`, `ui/src/components/SettingsModal.test.tsx`

**Interfaces:**
- Consumes: Task 9.
- Produces: `SkillsTab` with props `{ settings: SettingsView; onSettingsChange: (v: SettingsView) => void; targetLang: string; api?: SkillsApi }` and exported `SkillsApi` type (methods as in Task 9 `ipc.ts`). `SettingsModal` gains a third tab "Skills" and an optional `skillsApi?: SkillsApi` prop.

- [ ] **Step 1: Write the failing test** — `ui/src/components/SkillsTab.test.tsx`

```tsx
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
import { SkillsTab, type SkillsApi } from './SkillsTab';
import type { SettingsView } from '../types';

const view = (over: Partial<SettingsView> = {}): SettingsView =>
  ({
    profiles: [{ id: 'p', name: 'DeepSeek', kind: 'deepseek', base_url: null, model: 'm', has_key: true }],
    default_profile_id: 'p',
    general: { whisper_model: 'base', target_lang: 'vi', bilingual: true, style_preset: 'youtube_yellow', use_cloud_stt: false },
    stt_has_key: false,
    secrets_backend: 'keyring',
    load_warning: null,
    presets: [{ id: 'builtin:math', name: 'Toán học', description: 'd', system_prompt: 'Math', glossary: [{ source: 'limit', target: 'giới hạn' }] }],
    skills: [{ id: 'u1', name: 'Của tôi', description: '', system_prompt: 'Mine', glossary: [{ source: 'a', target: 'b' }] }],
    default_skill_id: 'builtin:math',
    ...over,
  }) as SettingsView;

let api: SkillsApi;
beforeEach(() => {
  api = {
    saveSkill: vi.fn().mockResolvedValue(view()),
    deleteSkill: vi.fn().mockResolvedValue(view()),
    duplicateSkill: vi.fn().mockResolvedValue(view()),
    setDefaultSkill: vi.fn().mockResolvedValue(view()),
    importSkill: vi.fn().mockResolvedValue(view()),
    exportSkill: vi.fn().mockResolvedValue('/tmp/x.echo-skill.json'),
    trySkill: vi.fn().mockResolvedValue({ lines: ['Xin chào'], system_prompt: 'Mine\n\nGlossary…', error: null }),
  };
});

const pick = (name: RegExp) => fireEvent.click(screen.getByRole('button', { name }));
const renderTab = () => render(<SkillsTab settings={view()} onSettingsChange={vi.fn()} targetLang="vi" api={api} />);

describe('SkillsTab', () => {
  it('preset is read-only with limited actions', () => {
    renderTab();
    pick(/Toán học/);
    expect((screen.getByLabelText('Tên skill') as HTMLInputElement).readOnly).toBe(true);
    expect(screen.getByRole('button', { name: 'Nhân bản' })).toBeDefined();
    expect(screen.getByRole('button', { name: 'Xuất file' })).toBeDefined();
    expect(screen.queryByRole('button', { name: 'Lưu' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Xóa' })).toBeNull();
  });

  it('saves a user skill with trimmed payload', async () => {
    renderTab();
    pick(/Của tôi/);
    fireEvent.change(screen.getByLabelText('Tên skill'), { target: { value: '  Mine 2 ' } });
    fireEvent.change(screen.getByLabelText('Dịch 1'), { target: { value: ' bb ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Lưu' }));
    await waitFor(() =>
      expect(api.saveSkill).toHaveBeenCalledWith({
        id: 'u1',
        name: 'Mine 2',
        description: '',
        system_prompt: 'Mine',
        glossary: [{ source: 'a', target: 'bb' }],
      })
    );
  });

  it('adds and removes glossary rows', () => {
    renderTab();
    pick(/Của tôi/);
    fireEvent.click(screen.getByRole('button', { name: '+ Thêm thuật ngữ' }));
    expect(screen.getByLabelText('Gốc 2')).toBeDefined();
    fireEvent.click(screen.getByRole('button', { name: 'Xóa thuật ngữ 1' }));
    expect(screen.queryByLabelText('Gốc 2')).toBeNull();
  });

  it('imports CSV into the form and reports counts', () => {
    renderTab();
    pick(/Của tôi/);
    fireEvent.click(screen.getByRole('button', { name: 'Nhập CSV' }));
    fireEvent.change(screen.getByLabelText('Nội dung CSV'), { target: { value: 'source,target\nA,x\nnew,y\n,bad' } });
    fireEvent.click(screen.getByRole('button', { name: 'Áp dụng CSV' }));
    expect(screen.getByText('Đã thêm 1, cập nhật 1, bỏ qua 1 dòng lỗi')).toBeDefined();
    expect((screen.getByLabelText('Gốc 2') as HTMLInputElement).value).toBe('new');
  });

  it('try panel shows translated lines and prompt', async () => {
    renderTab();
    pick(/Của tôi/);
    fireEvent.change(screen.getByLabelText('Câu mẫu'), { target: { value: 'Hello' } });
    fireEvent.click(screen.getByRole('button', { name: 'Dịch thử' }));
    expect(await screen.findByText('Xin chào')).toBeDefined();
    expect(api.trySkill).toHaveBeenCalledWith(expect.objectContaining({ id: 'u1' }), 'p', 'Hello', 'auto', 'vi');
    fireEvent.click(screen.getByText('Xem prompt'));
    expect(screen.getByText(/Glossary…/)).toBeDefined();
  });

  it('marks the default skill and sets default', async () => {
    renderTab();
    expect(screen.getByRole('button', { name: /★ Toán học/ })).toBeDefined();
    pick(/Của tôi/);
    fireEvent.click(screen.getByRole('button', { name: 'Đặt mặc định' }));
    await waitFor(() => expect(api.setDefaultSkill).toHaveBeenCalledWith('u1'));
  });
});
```

Append to `ui/src/components/SettingsModal.test.tsx` (the `view()` fixture there must gain `presets: [], skills: [], default_skill_id: null` — add them to its object literal):

```tsx
  it('has a Skills tab', () => {
    renderModal();
    fireEvent.click(screen.getByRole('button', { name: 'Skills' }));
    expect(screen.getByRole('button', { name: '+ Thêm skill' })).toBeDefined();
  });
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm --prefix ui test -- src/components/SkillsTab.test.tsx src/components/SettingsModal.test.tsx`
Expected: FAIL — `./SkillsTab` missing; no Skills tab.

- [ ] **Step 3: Implement** — `ui/src/components/SkillsTab.tsx`

```tsx
import React, { useEffect, useState } from 'react';
import type { GlossaryEntry, SettingsView, Skill, TrySkillResult } from '../types';
import * as ipc from '../lib/ipc';
import { allSkills, emptySkill, isBuiltinSkill } from '../lib/skills';
import { mergeGlossary, parseGlossaryCsv } from '../lib/csv';

export interface SkillsApi {
  saveSkill: (s: Skill) => Promise<SettingsView>;
  deleteSkill: (id: string) => Promise<SettingsView>;
  duplicateSkill: (id: string) => Promise<SettingsView>;
  setDefaultSkill: (id: string | null) => Promise<SettingsView>;
  importSkill: () => Promise<SettingsView>;
  exportSkill: (id: string) => Promise<string | null>;
  trySkill: (s: Skill, profileId: string | null, sample: string, sourceLang: string, targetLang: string) => Promise<TrySkillResult>;
}

const defaultApi: SkillsApi = {
  saveSkill: ipc.saveSkill,
  deleteSkill: ipc.deleteSkill,
  duplicateSkill: ipc.duplicateSkill,
  setDefaultSkill: ipc.setDefaultSkill,
  importSkill: ipc.importSkill,
  exportSkill: ipc.exportSkill,
  trySkill: ipc.trySkill,
};

interface SkillsTabProps {
  settings: SettingsView;
  onSettingsChange: (v: SettingsView) => void;
  targetLang: string;
  api?: SkillsApi;
}

const inputCls =
  'w-full bg-slate-900 border border-slate-700 rounded px-2 py-1.5 text-xs text-slate-100 focus:outline-none focus:border-blue-500 read-only:opacity-70';
const btnCls = 'px-3 py-1.5 rounded bg-slate-700 hover:bg-slate-600';

const trimmed = (s: Skill): Skill => ({
  ...s,
  name: s.name.trim(),
  description: s.description.trim(),
  system_prompt: s.system_prompt.trim(),
  glossary: s.glossary.map((g) => ({ source: g.source.trim(), target: g.target.trim() })),
});

export const SkillsTab: React.FC<SkillsTabProps> = ({ settings, onSettingsChange, targetLang, api = defaultApi }) => {
  const all = allSkills(settings);
  const [selectedId, setSelectedId] = useState<string | null>(settings.default_skill_id ?? all[0]?.id ?? null);
  const [form, setForm] = useState<Skill>(emptySkill());
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [csvOpen, setCsvOpen] = useState(false);
  const [csvText, setCsvText] = useState('');
  const [sample, setSample] = useState('');
  const [tryProfileId, setTryProfileId] = useState<string | null>(settings.default_profile_id);
  const [tryResult, setTryResult] = useState<TrySkillResult | null>(null);
  const [trying, setTrying] = useState(false);
  const [showPrompt, setShowPrompt] = useState(false);

  const selected = all.find((s) => s.id === selectedId) ?? null;
  const readOnly = selected ? isBuiltinSkill(selected) : false;

  useEffect(() => {
    setForm(selected ? JSON.parse(JSON.stringify(selected)) : emptySkill());
    setError(null);
    setInfo(null);
    setTryResult(null);
    setCsvOpen(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId]);

  const run = async (fn: () => Promise<SettingsView>): Promise<SettingsView | null> => {
    setError(null);
    try {
      const v = await fn();
      onSettingsChange(v);
      return v;
    } catch (e) {
      setError(String(e));
      return null;
    }
  };

  const setGlossary = (glossary: GlossaryEntry[]) => setForm((f) => ({ ...f, glossary }));

  const handleSave = async () => {
    const payload = trimmed(form);
    const v = await run(() => api.saveSkill(payload));
    if (v && !payload.id) {
      const created = v.skills.find((s) => s.name === payload.name);
      if (created) setSelectedId(created.id);
    }
  };

  const handleDuplicate = async () => {
    if (!selected) return;
    const v = await run(() => api.duplicateSkill(selected.id));
    if (v && v.skills.length > 0) setSelectedId(v.skills[v.skills.length - 1].id);
  };

  const handleDelete = async () => {
    if (!selected || !window.confirm(`Xóa skill "${selected.name}"?`)) return;
    if (await run(() => api.deleteSkill(selected.id))) setSelectedId(null);
  };

  const handleExport = async () => {
    if (!selected) return;
    try {
      const path = await api.exportSkill(selected.id);
      if (path) setInfo(`Đã xuất: ${path}`);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleImport = async () => {
    const before = settings.skills.length;
    const v = await run(() => api.importSkill());
    if (v && v.skills.length > before) setSelectedId(v.skills[v.skills.length - 1].id);
  };

  const applyCsv = () => {
    const { entries, skipped } = parseGlossaryCsv(csvText);
    const { glossary, added, updated } = mergeGlossary(form.glossary, entries);
    setGlossary(glossary);
    setInfo(`Đã thêm ${added}, cập nhật ${updated}, bỏ qua ${skipped} dòng lỗi`);
    setCsvOpen(false);
    setCsvText('');
  };

  const readCsvFile = (file: File) => {
    const reader = new FileReader();
    reader.onload = () => setCsvText(String(reader.result ?? ''));
    reader.readAsText(file);
  };

  const handleTry = async () => {
    setTrying(true);
    setTryResult(null);
    try {
      setTryResult(await api.trySkill(trimmed(form), tryProfileId, sample, 'auto', targetLang));
    } catch (e) {
      setTryResult({ lines: [], system_prompt: '', error: String(e) });
    } finally {
      setTrying(false);
    }
  };

  const sampleLines = sample.split('\n').map((l) => l.trim()).filter(Boolean);

  const listButton = (s: Skill) => (
    <button
      key={s.id}
      onClick={() => setSelectedId(s.id)}
      className={`text-left px-2 py-1.5 rounded truncate ${s.id === selectedId ? 'bg-slate-700 text-white' : 'hover:bg-slate-700/50'}`}
      title={s.description}
    >
      {isBuiltinSkill(s) && <span className="mr-1">🔒</span>}
      <span>{s.id === settings.default_skill_id ? `★ ${s.name}` : s.name}</span>
    </button>
  );

  return (
    <div className="flex-1 min-h-0 flex gap-3 p-4 text-xs">
      <div className="w-52 shrink-0 flex flex-col gap-1 overflow-y-auto">
        <div className="text-[10px] uppercase text-slate-500 mt-1">Có sẵn</div>
        {settings.presets.map(listButton)}
        <div className="text-[10px] uppercase text-slate-500 mt-2">Của tôi</div>
        {settings.skills.map(listButton)}
        <button onClick={() => setSelectedId(null)} className="mt-1 px-2 py-1.5 rounded border border-dashed border-slate-600 hover:bg-slate-700/50">
          + Thêm skill
        </button>
        <button onClick={handleImport} className="px-2 py-1.5 rounded border border-slate-600 hover:bg-slate-700/50">
          Nhập file
        </button>
      </div>

      <div className="flex-1 flex flex-col gap-2.5 overflow-y-auto pr-1">
        <input aria-label="Tên skill" placeholder="Tên" value={form.name} readOnly={readOnly}
          onChange={(e) => setForm({ ...form, name: e.target.value })} className={inputCls} />
        <input aria-label="Mô tả" placeholder="Mô tả ngắn" value={form.description} readOnly={readOnly}
          onChange={(e) => setForm({ ...form, description: e.target.value })} className={inputCls} />
        <textarea aria-label="System prompt" rows={6} value={form.system_prompt} readOnly={readOnly}
          onChange={(e) => setForm({ ...form, system_prompt: e.target.value })} className={`${inputCls} font-mono`} />
        <span className="text-slate-500">Có thể dùng {'{source_lang}'} và {'{target_lang}'}</span>

        <div className="flex items-center justify-between">
          <span className="font-semibold">Glossary ({form.glossary.length})</span>
          {!readOnly && (
            <div className="flex gap-2">
              <button onClick={() => setGlossary([...form.glossary, { source: '', target: '' }])} className={btnCls}>
                + Thêm thuật ngữ
              </button>
              <button onClick={() => setCsvOpen((o) => !o)} className={btnCls}>Nhập CSV</button>
            </div>
          )}
        </div>

        {csvOpen && (
          <div className="flex flex-col gap-2 border border-slate-700 rounded p-2">
            <textarea aria-label="Nội dung CSV" rows={4} value={csvText} placeholder="gốc,dịch"
              onChange={(e) => setCsvText(e.target.value)} className={`${inputCls} font-mono`} />
            <div className="flex gap-2 items-center">
              <input type="file" accept=".csv,text/csv" aria-label="File CSV"
                onChange={(e) => e.target.files?.[0] && readCsvFile(e.target.files[0])} />
              <button onClick={applyCsv} className="px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-white">Áp dụng CSV</button>
            </div>
          </div>
        )}

        <div className="flex flex-col gap-1 max-h-56 overflow-y-auto">
          {form.glossary.map((g, i) => (
            <div key={i} className="flex gap-1.5 items-center">
              <input aria-label={`Gốc ${i + 1}`} value={g.source} readOnly={readOnly} className={inputCls}
                onChange={(e) => setGlossary(form.glossary.map((x, j) => (j === i ? { ...x, source: e.target.value } : x)))} />
              <span className="text-slate-500">→</span>
              <input aria-label={`Dịch ${i + 1}`} value={g.target} readOnly={readOnly} placeholder="(giữ nguyên)" className={inputCls}
                onChange={(e) => setGlossary(form.glossary.map((x, j) => (j === i ? { ...x, target: e.target.value } : x)))} />
              {!readOnly && (
                <button aria-label={`Xóa thuật ngữ ${i + 1}`} className="text-rose-400 px-1"
                  onClick={() => setGlossary(form.glossary.filter((_, j) => j !== i))}>×</button>
              )}
            </div>
          ))}
        </div>

        {info && <div className="text-emerald-400">{info}</div>}
        {error && <div className="text-rose-400">{error}</div>}

        <div className="flex flex-wrap gap-2">
          {!readOnly && (
            <button onClick={handleSave} className="px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-white">Lưu</button>
          )}
          {selected && (
            <>
              <button onClick={handleDuplicate} className={btnCls}>Nhân bản</button>
              <button onClick={handleExport} className={btnCls}>Xuất file</button>
              {selected.id !== settings.default_skill_id && (
                <button onClick={() => run(() => api.setDefaultSkill(selected.id))} className={btnCls}>Đặt mặc định</button>
              )}
              {!readOnly && (
                <button onClick={handleDelete} className="px-3 py-1.5 rounded bg-rose-700 hover:bg-rose-600 text-white">Xóa</button>
              )}
            </>
          )}
        </div>

        <div className="border-t border-slate-700 pt-2 flex flex-col gap-2">
          <span className="font-semibold">Chạy thử</span>
          <textarea aria-label="Câu mẫu" rows={3} value={sample} placeholder="Mỗi dòng một câu (tối đa 20)"
            onChange={(e) => setSample(e.target.value)} className={inputCls} />
          <div className="flex gap-2">
            <select aria-label="Profile chạy thử" value={tryProfileId ?? ''} onChange={(e) => setTryProfileId(e.target.value || null)} className={inputCls}>
              {settings.profiles.map((p) => (
                <option key={p.id} value={p.id}>{p.name}</option>
              ))}
            </select>
            <button onClick={handleTry} disabled={trying} className="px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 text-white shrink-0 disabled:opacity-50">
              {trying ? 'Đang dịch...' : 'Dịch thử'}
            </button>
          </div>
          {tryResult?.error && <div className="text-rose-400">{tryResult.error}</div>}
          {tryResult && tryResult.lines.length > 0 && (
            <table className="w-full text-left">
              <tbody>
                {tryResult.lines.map((l, i) => (
                  <tr key={i} className="border-b border-slate-800">
                    <td className="py-1 pr-2 text-slate-400 w-1/2">{sampleLines[i]}</td>
                    <td className="py-1">{l}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {tryResult && tryResult.system_prompt && (
            <div>
              <button onClick={() => setShowPrompt((s) => !s)} className="text-blue-400 hover:underline">Xem prompt</button>
              {showPrompt && (
                <pre className="mt-1 whitespace-pre-wrap bg-slate-900 border border-slate-700 rounded p-2 font-mono text-[11px]">
                  {tryResult.system_prompt}
                </pre>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
```

`ui/src/components/SettingsModal.tsx`:
1. `import { SkillsTab, type SkillsApi } from './SkillsTab';`
2. Add prop `skillsApi?: SkillsApi;` to `SettingsModalProps` and destructure it.
3. Change tab state type to `'providers' | 'general' | 'skills'` and add a third header button after "Chung":

```tsx
            <button
              onClick={() => setTab('skills')}
              className={`px-3 py-1 rounded ${tab === 'skills' ? 'bg-blue-600 text-white' : 'bg-slate-700'}`}
            >
              Skills
            </button>
```

4. Replace the body conditional `{tab === 'providers' ? ( …providers… ) : ( …general… )}` with:

```tsx
        {tab === 'skills' ? (
          <SkillsTab
            settings={settings}
            onSettingsChange={onSettingsChange}
            targetLang={general.target_lang}
            api={skillsApi}
          />
        ) : tab === 'providers' ? (
          /* existing providers JSX unchanged */
        ) : (
          /* existing general JSX unchanged */
        )}
```

(Keep the existing providers and general JSX blocks verbatim inside the two branches.)

- [ ] **Step 4: Run tests**

Run: `npm --prefix ui test -- src/components/SkillsTab.test.tsx src/components/SettingsModal.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/SkillsTab.tsx ui/src/components/SkillsTab.test.tsx ui/src/components/SettingsModal.tsx ui/src/components/SettingsModal.test.tsx
git commit -m "feat(ui): Skills tab with glossary editor, CSV import and try panel"
```

---

### Task 11: Header skill select, JobList override, App wiring

**Files:**
- Modify: `ui/src/components/HeaderBar.tsx`, `ui/src/components/HeaderBar.test.tsx`, `ui/src/components/JobList.tsx`, `ui/src/components/JobList.test.tsx`, `ui/src/App.tsx`, `ui/src/App.test.tsx`

**Interfaces:**
- Consumes: Task 9 helpers/IPC.
- Produces: `HeaderBar` props `presets: Skill[]`, `userSkills: Skill[]`, `defaultSkillId: string | null`, `onSelectSkill: (id: string | null) => void`; `JobList` props `skillOptions: { id: string; name: string }[]`, `defaultSkillName: string | null`, `onChangeJobSkill: (jobId: string, skillId: string | undefined) => void`.

- [ ] **Step 1: Write the failing tests**

`ui/src/components/HeaderBar.test.tsx` — in every existing `render(<HeaderBar … />)` add after `onOpenSettings={…}`:

```tsx
        presets={[]}
        userSkills={[]}
        defaultSkillId={null}
        onSelectSkill={vi.fn()}
```

Append inside `describe`:

```tsx
  it('skill select lists none, presets and user skills', () => {
    const onSelectSkill = vi.fn();
    render(
      <HeaderBar
        videoPath=""
        onSelectVideo={vi.fn()}
        whisperModel="base"
        onSelectWhisperModel={vi.fn()}
        profiles={[]}
        defaultProfileId={null}
        onSelectProfile={vi.fn()}
        onOpenSettings={vi.fn()}
        presets={[{ id: 'builtin:math', name: 'Toán học', description: '', system_prompt: 'p', glossary: [] }]}
        userSkills={[{ id: 'u1', name: 'Của tôi', description: '', system_prompt: 'p', glossary: [] }]}
        defaultSkillId={null}
        onSelectSkill={onSelectSkill}
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
    const select = screen.getByLabelText('Skill') as HTMLSelectElement;
    expect(select.value).toBe('none');
    expect(Array.from(select.options).map((o) => o.value)).toEqual(['none', 'builtin:math', 'u1']);
    fireEvent.change(select, { target: { value: 'u1' } });
    expect(onSelectSkill).toHaveBeenCalledWith('u1');
    fireEvent.change(select, { target: { value: 'none' } });
    expect(onSelectSkill).toHaveBeenCalledWith(null);
  });
```

`ui/src/components/JobList.test.tsx` — extend `handlers`:

```tsx
const handlers = {
  onSelect: vi.fn(),
  onRemove: vi.fn(),
  onCancelJob: vi.fn(),
  onOpenFolder: vi.fn(),
  skillOptions: [
    { id: 'builtin:math', name: 'Toán học' },
    { id: 'u1', name: 'Của tôi' },
  ],
  defaultSkillName: 'Toán học',
  onChangeJobSkill: vi.fn(),
};
```

Append inside `describe`:

```tsx
  it('per-job skill select only for editable statuses', () => {
    const jobs: BatchJob[] = [
      baseJob,
      { ...baseJob, id: 'job-2', videoPath: '/v/run.mp4', status: 'running', skillUsed: 'Của tôi' },
      { ...baseJob, id: 'job-3', videoPath: '/v/err.mp4', status: 'error', skillId: 'none' },
    ];
    render(<JobList jobs={jobs} selectedJobId={null} {...handlers} />);
    const selects = screen.getAllByLabelText('Skill cho file') as HTMLSelectElement[];
    expect(selects).toHaveLength(2);
    expect(selects[0].value).toBe('');
    expect(selects[0].options[0].text).toBe('Mặc định (Toán học)');
    expect(selects[1].value).toBe('none');
    fireEvent.change(selects[0], { target: { value: 'u1' } });
    expect(handlers.onChangeJobSkill).toHaveBeenCalledWith('job-1', 'u1');
    fireEvent.change(selects[1], { target: { value: '' } });
    expect(handlers.onChangeJobSkill).toHaveBeenCalledWith('job-3', undefined);
    expect(screen.getByText('🎓 Của tôi')).toBeDefined();
  });
```

`ui/src/App.test.tsx` — in the hoisted `loaded` add `presets: [{ id: 'builtin:math', name: 'Toán học', description: '', system_prompt: 'p', glossary: [] }], skills: [], default_skill_id: 'builtin:math',`, add `setDefaultSkill: vi.fn().mockResolvedValue(loaded),` to the mocked module, and append:

```tsx
  it('header shows default skill from settings', async () => {
    render(<App />);
    await waitFor(() =>
      expect((screen.getByLabelText('Skill') as HTMLSelectElement).value).toBe('builtin:math')
    );
  });
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm --prefix ui test -- src/components/HeaderBar.test.tsx src/components/JobList.test.tsx src/App.test.tsx`
Expected: FAIL.

- [ ] **Step 3: Implement**

`ui/src/components/HeaderBar.tsx`:
1. Add `GraduationCap` to the lucide import; add `Skill` to the type import.
2. Add to `HeaderBarProps` and destructuring: `presets: Skill[]; userSkills: Skill[]; defaultSkillId: string | null; onSelectSkill: (id: string | null) => void;`.
3. Right after the Provider profile `<div>` block, add:

```tsx
        {/* Translation skill selector */}
        <div className="flex items-center bg-slate-900/60 rounded-md border border-slate-700 px-1.5 py-0.5">
          <GraduationCap className="w-3.5 h-3.5 text-emerald-400 mr-1.5" />
          <select
            aria-label="Skill"
            value={defaultSkillId ?? 'none'}
            onChange={(e) => onSelectSkill(e.target.value === 'none' ? null : e.target.value)}
            disabled={isRunning}
            className="bg-transparent text-slate-200 focus:outline-none py-1 text-xs cursor-pointer max-w-[160px] truncate"
          >
            <option value="none" className="bg-slate-800">Không dùng skill</option>
            {presets.length > 0 && (
              <optgroup label="Có sẵn" className="bg-slate-800">
                {presets.map((s) => (
                  <option key={s.id} value={s.id} className="bg-slate-800">🔒 {s.name}</option>
                ))}
              </optgroup>
            )}
            {userSkills.length > 0 && (
              <optgroup label="Của tôi" className="bg-slate-800">
                {userSkills.map((s) => (
                  <option key={s.id} value={s.id} className="bg-slate-800">{s.name}</option>
                ))}
              </optgroup>
            )}
          </select>
        </div>
```

`ui/src/components/JobList.tsx`:
1. Add to `JobListProps` and destructuring: `skillOptions: { id: string; name: string }[]; defaultSkillName: string | null; onChangeJobSkill: (jobId: string, skillId: string | undefined) => void;`.
2. Add a helper above the component:

```tsx
const EDITABLE: BatchJob['status'][] = ['queued', 'error', 'cancelled'];
```

3. Directly after the first row `</div>` (the one containing file name + status + button), insert:

```tsx
            {EDITABLE.includes(job.status) ? (
              <select
                aria-label="Skill cho file"
                value={job.skillId ?? ''}
                onClick={(e) => e.stopPropagation()}
                onChange={(e) => onChangeJobSkill(job.id, e.target.value === '' ? undefined : e.target.value)}
                className="self-start bg-slate-900/70 border border-slate-700 rounded px-1.5 py-0.5 text-[10px] text-slate-300 focus:outline-none"
              >
                <option value="">Mặc định ({defaultSkillName ?? 'không dùng'})</option>
                <option value="none">Không dùng skill</option>
                {skillOptions.map((s) => (
                  <option key={s.id} value={s.id}>{s.name}</option>
                ))}
              </select>
            ) : (
              job.skillUsed && <span className="text-slate-500 text-[10px]">🎓 {job.skillUsed}</span>
            )}
```

`ui/src/App.tsx`:
1. Imports: add `setDefaultSkill` to the `./lib/ipc` import; add `import { allSkills, jobSkillChoice, skillName } from './lib/skills';`.
2. After the `settingsView` state, add:

```tsx
  const settingsRef = useRef<SettingsView | null>(null);
  settingsRef.current = settingsView;
```

3. Add handlers after `handleSelectProfile`:

```tsx
  const handleSelectSkill = async (id: string | null) => {
    try {
      setSettingsView(await setDefaultSkill(id));
    } catch (err) {
      setLogs((prev) => [...prev, `[LỖI] ${String(err)}`]);
    }
  };

  const handleChangeJobSkill = (jobId: string, skillId: string | undefined) => updateJob(jobId, { skillId });
```

4. In `runJob`, at the top (before the first `updateJob`), add:

```tsx
    const current = jobsRef.current.find((j) => j.id === job.id) ?? job;
    const { skill_id, label } = jobSkillChoice(current, settingsRef.current);
```

   add `skillUsed: label,` to the first `updateJob(job.id, { status: 'running', … })` patch, and in the `startPipeline({...})` object replace `profile_id: settingsView?.default_profile_id ?? null,` with:

```tsx
        profile_id: settingsRef.current?.default_profile_id ?? null,
        skill_id,
```

5. In `<HeaderBar …>` add:

```tsx
        presets={settingsView?.presets ?? []}
        userSkills={settingsView?.skills ?? []}
        defaultSkillId={settingsView?.default_skill_id ?? null}
        onSelectSkill={handleSelectSkill}
```

6. In `<JobList …>` add:

```tsx
                skillOptions={settingsView ? allSkills(settingsView).map((s) => ({ id: s.id, name: s.name })) : []}
                defaultSkillName={skillName(settingsView, settingsView?.default_skill_id)}
                onChangeJobSkill={handleChangeJobSkill}
```

- [ ] **Step 4: Run tests, typecheck, build**

Run: `npm --prefix ui test && npm --prefix ui run build`
Expected: all pass; build succeeds.

- [ ] **Step 5: Commit**

```bash
git add ui/src/components/HeaderBar.tsx ui/src/components/HeaderBar.test.tsx ui/src/components/JobList.tsx ui/src/components/JobList.test.tsx ui/src/App.tsx ui/src/App.test.tsx
git commit -m "feat(ui): default skill in header, per-job skill override"
```

---

### Task 12: Full verification

**Files:** none (fix-ups only).

- [ ] **Step 1:** Run `cargo test --workspace` — expected all pass.
- [ ] **Step 2:** Run `cargo clippy --workspace --all-targets 2>&1 | grep -E "src/skills|skill_commands" ` — expected no output.
- [ ] **Step 3:** Run `npm --prefix ui test && npm --prefix ui run build` — expected pass.
- [ ] **Step 4:** Run `npx tauri build --bundles deb` — expected `Finished 1 bundle`.
- [ ] **Step 5: Manual smoke (user)**
  1. `npm run dev` → ⚙️ → Skills: presets show 🔒; Nhân bản "AI / Machine Learning" → edit glossary → Lưu.
  2. Nhập CSV with `source,target\nattention head,đầu chú ý` → counts shown → Lưu.
  3. Chạy thử with "We minimize the loss function using gradient descent." → result uses "hàm mất mát", "hạ gradient"; Xem prompt shows only those two glossary lines.
  4. Xuất file → Nhập file → imported copy appears with " (2)".
  5. Queue two videos; set job 1 to "Toán học", job 2 to "Không dùng skill"; run; logs show `Skill: Toán học` and `Skill: không dùng`.
  6. `cargo run --bin auto-subtitle -- --list-skills` and `cargo run --bin auto-subtitle -- -i jfk.flac --mode subs-only --skill none`.
- [ ] **Step 6:** Commit fix-ups if any: `git add -A && git commit -m "chore: fix-ups from skills verification"`.
