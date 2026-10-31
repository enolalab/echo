use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub cache_dir: PathBuf,
    pub models_dir: PathBuf,
    pub bin_dir: PathBuf,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let cache_root = dirs::cache_dir()
            .context("failed to determine cache directory")?
            .join("auto-subtitle");
        Self::with_base(cache_root)
    }

    pub fn with_base(base: PathBuf) -> Result<Self> {
        let models_dir = base.join("models");
        let bin_dir = base.join("bin");

        fs::create_dir_all(&models_dir)
            .with_context(|| format!("failed to create models directory at {}", models_dir.display()))?;
        fs::create_dir_all(&bin_dir)
            .with_context(|| format!("failed to create bin directory at {}", bin_dir.display()))?;

        Ok(Self {
            cache_dir: base,
            models_dir,
            bin_dir,
        })
    }
}
