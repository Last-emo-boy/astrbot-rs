use std::fmt;
use std::fs;
use std::sync::Arc;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioFormat {
    Silk,
    Amr,
}

impl AudioFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Silk => "silk",
            Self::Amr => "amr",
        }
    }
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug)]
pub struct AudioConversionRequest {
    pub provider_label: String,
    pub audio_url: String,
    pub format: AudioFormat,
    pub audio: Vec<u8>,
}

#[async_trait]
pub trait AudioMediaConverter: Send + Sync {
    async fn convert(&self, request: AudioConversionRequest) -> Result<Vec<u8>>;
}

#[derive(Clone, Debug, Default)]
pub struct UnsupportedAudioMediaConverter;

#[async_trait]
impl AudioMediaConverter for UnsupportedAudioMediaConverter {
    async fn convert(&self, request: AudioConversionRequest) -> Result<Vec<u8>> {
        Err(AstrbotError::Provider(format!(
            "{} audio requires {} conversion, but no audio media converter is configured for the media conversion boundary",
            request.provider_label, request.format
        )))
    }
}

#[derive(Clone)]
pub struct AudioInputLoader {
    client: reqwest::Client,
    converter: Arc<dyn AudioMediaConverter>,
}

impl fmt::Debug for AudioInputLoader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioInputLoader").finish_non_exhaustive()
    }
}

impl AudioInputLoader {
    pub fn new(timeout: Duration) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|err| {
                AstrbotError::Provider(format!("failed to build HTTP audio client: {err}"))
            })?;

        Ok(Self {
            client,
            converter: Arc::new(UnsupportedAudioMediaConverter),
        })
    }

    pub fn with_converter(mut self, converter: Arc<dyn AudioMediaConverter>) -> Self {
        self.converter = converter;
        self
    }

    pub async fn load(&self, audio_url: &str, provider_label: &str) -> Result<Vec<u8>> {
        if audio_url.trim().is_empty() {
            return Err(AstrbotError::Provider(
                "speech-to-text request must contain an audio URL".to_string(),
            ));
        }

        let audio = if is_http_url(audio_url) {
            self.download(audio_url, provider_label).await?
        } else {
            let audio = fs::read(audio_url).map_err(|err| {
                AstrbotError::Provider(format!("failed to read audio file {audio_url}: {err}"))
            })?;
            if audio.is_empty() {
                return Err(AstrbotError::Provider(format!(
                    "audio file {audio_url} was empty"
                )));
            }
            audio
        };

        let Some(format) = detect_audio_conversion_requirement(audio_url, &audio) else {
            return Ok(audio);
        };

        let converted = self
            .converter
            .convert(AudioConversionRequest {
                provider_label: provider_label.to_string(),
                audio_url: audio_url.to_string(),
                format,
                audio,
            })
            .await?;
        if converted.is_empty() {
            return Err(AstrbotError::Provider(format!(
                "{provider_label} audio media converter returned empty audio"
            )));
        }

        Ok(converted)
    }

    async fn download(&self, audio_url: &str, provider_label: &str) -> Result<Vec<u8>> {
        let response = self.client.get(audio_url).send().await.map_err(|err| {
            AstrbotError::Provider(format!("{provider_label} audio download failed: {err}"))
        })?;
        let status = response.status();
        let body = response.bytes().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read audio download response: {err}"))
        })?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&body);
            return Err(AstrbotError::Provider(format!(
                "{provider_label} audio download returned {status}: {}",
                truncate(body.trim())
            )));
        }
        if body.is_empty() {
            return Err(AstrbotError::Provider(
                "downloaded audio was empty".to_string(),
            ));
        }

        Ok(body.to_vec())
    }
}

pub fn detect_audio_conversion_requirement(audio_url: &str, audio: &[u8]) -> Option<AudioFormat> {
    let audio_url = audio_url.to_ascii_lowercase();
    if audio.starts_with(b"SILK")
        || audio_url.ends_with(".silk")
        || audio_url.contains("multimedia.nt.qq.com.cn")
    {
        return Some(AudioFormat::Silk);
    }
    if audio.starts_with(b"#!AMR") || audio_url.ends_with(".amr") {
        return Some(AudioFormat::Amr);
    }

    None
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn truncate(text: &str) -> String {
    const ERROR_TEXT_MAX_CHARS: usize = 4096;
    if text.chars().count() <= ERROR_TEXT_MAX_CHARS {
        return text.to_string();
    }

    text.chars().take(ERROR_TEXT_MAX_CHARS).collect()
}
