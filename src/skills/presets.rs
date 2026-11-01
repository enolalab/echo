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
