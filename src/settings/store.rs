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
