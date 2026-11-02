use std::path::PathBuf;

use anyhow::Context;
use tokio::io::AsyncWriteExt;

use super::TtsEngine;

pub struct PiperTts {
    piper: PathBuf,
    model: PathBuf,
    config: PathBuf,
}

impl PiperTts {
    pub fn new(piper: PathBuf, model: PathBuf, config: PathBuf) -> Self {
        Self {
            piper,
            model,
            config,
        }
    }
}

#[async_trait::async_trait]
impl TtsEngine for PiperTts {
    fn name(&self) -> &'static str {
        "piper"
    }

    async fn synthesize(&self, text: &str) -> anyhow::Result<Vec<u8>> {
        let dir = tempfile::tempdir().context("piper temp dir")?;
        let out = dir.path().join("out.wav");
        let mut cmd = tokio::process::Command::new(&self.piper);
        cmd.arg("-m")
            .arg(&self.model)
            .arg("-c")
            .arg(&self.config)
            .arg("-f")
            .arg(&out)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());
        // espeak-ng-data + bundled .so files resolve relative to the binary's dir
        // (release layout may put them beside the binary or under <bundle>/lib/).
        if let Some(parent) = self.piper.parent() {
            cmd.current_dir(parent);
            let mut ld = format!("{}:{}", parent.join("lib").display(), parent.display());
            if let Ok(existing) = std::env::var("LD_LIBRARY_PATH") {
                if !existing.is_empty() {
                    ld.push(':');
                    ld.push_str(&existing);
                }
            }
            cmd.env("LD_LIBRARY_PATH", ld);
        }
        let mut child = cmd.spawn().context("failed to spawn piper")?;
        child
            .stdin
            .as_mut()
            .expect("stdin piped")
            .write_all(text.as_bytes())
            .await
            .context("failed to write piper stdin")?;
        drop(child.stdin.take());
        let output = child
            .wait_with_output()
            .await
            .context("piper wait failed")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let tail: String = stderr
                .lines()
                .rev()
                .take(10)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n");
            anyhow::bail!("piper failed ({}): {tail}", output.status);
        }
        let bytes = tokio::fs::read(&out)
            .await
            .with_context(|| format!("piper did not write {}", out.display()))?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs() {
        let e = PiperTts::new("p".into(), "m".into(), "c".into());
        assert_eq!(e.name(), "piper");
    }
}
