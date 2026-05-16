use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::media::{GeneratedMediaArtifactWriter, default_tts_output_dir};
use crate::{TextToSpeechProvider, TextToSpeechRequest, TextToSpeechResponse};

const ERROR_TEXT_MAX_CHARS: usize = 4096;

#[derive(Clone, Debug)]
pub struct GsviTextToSpeechConfig {
    pub api_base: String,
    pub character: Option<String>,
    pub emotion: Option<String>,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub output_dir: PathBuf,
}

impl GsviTextToSpeechConfig {
    pub fn new(api_base: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            character: None,
            emotion: None,
            timeout: Duration::from_secs(60),
            custom_headers: HashMap::new(),
            output_dir: default_tts_output_dir(),
        }
    }

    pub fn with_character(mut self, character: impl Into<String>) -> Self {
        let character = character.into();
        self.character = (!character.trim().is_empty()).then_some(character);
        self
    }

    pub fn with_emotion(mut self, emotion: impl Into<String>) -> Self {
        let emotion = emotion.into();
        self.emotion = (!emotion.trim().is_empty()).then_some(emotion);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_headers.insert(key.into(), value.into());
        self
    }

    pub fn with_output_dir(mut self, output_dir: impl Into<PathBuf>) -> Self {
        self.output_dir = output_dir.into();
        self
    }

    fn tts_url(&self, text: &str) -> String {
        let mut query = vec![format!("text={}", percent_encode(text))];
        if let Some(character) = &self.character {
            query.push(format!("character={}", percent_encode(character)));
        }
        if let Some(emotion) = &self.emotion {
            query.push(format!("emotion={}", percent_encode(emotion)));
        }

        format!(
            "{}/tts?{}",
            self.api_base.trim_end_matches('/'),
            query.join("&")
        )
    }

    fn artifact_writer(&self) -> GeneratedMediaArtifactWriter {
        GeneratedMediaArtifactWriter::new(self.output_dir.clone(), "gsvi_tts", "wav")
    }
}

#[derive(Clone, Debug)]
pub struct GsviTextToSpeechProvider {
    config: GsviTextToSpeechConfig,
    client: reqwest::Client,
}

impl GsviTextToSpeechProvider {
    pub fn new(config: GsviTextToSpeechConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .default_headers(build_headers(&config)?)
            .build()
            .map_err(|err| AstrbotError::Provider(format!("failed to build HTTP client: {err}")))?;

        Ok(Self { config, client })
    }

    fn write_audio(&self, audio: &[u8]) -> Result<String> {
        self.config
            .artifact_writer()
            .write_audio(audio, "GSVI TTS provider returned empty audio")
    }
}

#[async_trait]
impl TextToSpeechProvider for GsviTextToSpeechProvider {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        if request.text.trim().is_empty() {
            return Err(AstrbotError::Provider(
                "text-to-speech request must contain text".to_string(),
            ));
        }

        let response = self
            .client
            .get(self.config.tts_url(&request.text))
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("GSVI TTS request failed: {err}")))?;

        let status = response.status();
        let body = response.bytes().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;

        if !status.is_success() {
            let body = String::from_utf8_lossy(&body);
            return Err(AstrbotError::Provider(format!(
                "GSVI TTS provider returned {status}: {}",
                truncate(body.trim())
            )));
        }

        Ok(TextToSpeechResponse::new(self.write_audio(&body)?))
    }
}

fn build_headers(config: &GsviTextToSpeechConfig) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();

    for (key, value) in &config.custom_headers {
        let name = HeaderName::from_bytes(key.as_bytes()).map_err(|_| {
            AstrbotError::Provider(format!("invalid custom provider header name: {key}"))
        })?;
        let value = HeaderValue::from_str(value).map_err(|_| {
            AstrbotError::Provider(format!("invalid custom provider header value for: {key}"))
        })?;
        headers.insert(name, value);
    }

    Ok(headers)
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char);
            }
            _ => {
                let _ = write!(&mut encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= ERROR_TEXT_MAX_CHARS {
        return text.to_string();
    }

    text.chars().take(ERROR_TEXT_MAX_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::percent_encode;

    #[test]
    fn percent_encoding_matches_astrbot_query_style() {
        assert_eq!(percent_encode("hello world"), "hello%20world");
        assert_eq!(percent_encode("可莉"), "%E5%8F%AF%E8%8E%89");
    }
}
