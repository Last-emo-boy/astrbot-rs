use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use astrbot_core::{AstrbotError, Result};
use astrbot_storage::TempArtifactRoot;

static NEXT_ARTIFACT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub(crate) struct GeneratedMediaArtifactWriter {
    output_dir: PathBuf,
    file_prefix: String,
    extension: String,
}

impl GeneratedMediaArtifactWriter {
    pub(crate) fn new(
        output_dir: impl Into<PathBuf>,
        file_prefix: impl Into<String>,
        extension: impl Into<String>,
    ) -> Self {
        Self {
            output_dir: output_dir.into(),
            file_prefix: file_prefix.into(),
            extension: safe_media_extension(extension),
        }
    }

    pub(crate) fn write_audio(
        &self,
        audio: &[u8],
        empty_error: impl Into<String>,
    ) -> Result<String> {
        if audio.is_empty() {
            return Err(AstrbotError::Provider(empty_error.into()));
        }

        fs::create_dir_all(&self.output_dir).map_err(|err| {
            AstrbotError::Provider(format!(
                "failed to create TTS output directory {}: {err}",
                display_path(&self.output_dir)
            ))
        })?;

        let path = self.next_path();
        fs::write(&path, audio).map_err(|err| {
            AstrbotError::Provider(format!(
                "failed to write TTS audio file {}: {err}",
                display_path(&path)
            ))
        })?;

        Ok(display_path(&path))
    }

    fn next_path(&self) -> PathBuf {
        self.output_dir.join(format!(
            "{}_{}.{}",
            self.file_prefix,
            next_artifact_id(),
            self.extension
        ))
    }
}

pub(crate) fn default_tts_output_dir() -> PathBuf {
    TempArtifactRoot::default()
        .bucket("generated_media")
        .join("tts")
}

pub(crate) fn safe_media_extension(extension: impl Into<String>) -> String {
    let extension = extension
        .into()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>();
    if extension.is_empty() {
        "audio".to_string()
    } else {
        extension
    }
}

fn next_artifact_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = NEXT_ARTIFACT_ID.fetch_add(1, Ordering::Relaxed);
    format!("{timestamp}_{sequence}")
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::default_tts_output_dir;

    #[test]
    fn default_tts_output_dir_uses_astrbot_temp_artifact_root() {
        let output_dir = default_tts_output_dir();
        let rendered = output_dir.to_string_lossy();

        assert!(rendered.contains("data"));
        assert!(rendered.contains("temp"));
        assert!(rendered.contains("generated_media"));
        assert!(rendered.ends_with("tts"));
    }
}
