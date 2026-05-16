use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::Serialize;

use crate::http::{build_http_client, extract_error_message, join_api_path, json_bearer_headers};
use crate::media::{GeneratedMediaArtifactWriter, default_tts_output_dir, safe_media_extension};
use crate::{TextToSpeechProvider, TextToSpeechRequest, TextToSpeechResponse};

#[derive(Clone, Debug)]
pub struct OpenAiTextToSpeechConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub model: String,
    pub voice: String,
    pub response_format: String,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub output_dir: PathBuf,
}

impl OpenAiTextToSpeechConfig {
    pub fn new(api_base: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: None,
            model: model.into(),
            voice: "alloy".to_string(),
            response_format: "wav".to_string(),
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            output_dir: default_tts_output_dir(),
        }
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_voice(mut self, voice: impl Into<String>) -> Self {
        self.voice = voice.into();
        self
    }

    pub fn with_response_format(mut self, response_format: impl Into<String>) -> Self {
        self.response_format = response_format.into();
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

    fn speech_url(&self) -> String {
        join_api_path(&self.api_base, "audio/speech")
    }

    fn artifact_writer(&self) -> GeneratedMediaArtifactWriter {
        GeneratedMediaArtifactWriter::new(
            self.output_dir.clone(),
            "openai_tts_api",
            safe_media_extension(&self.response_format),
        )
    }
}

#[derive(Clone, Debug)]
pub struct OpenAiTextToSpeechProvider {
    config: OpenAiTextToSpeechConfig,
    client: reqwest::Client,
}

impl OpenAiTextToSpeechProvider {
    pub fn new(config: OpenAiTextToSpeechConfig) -> Result<Self> {
        let client = build_http_client(
            config.timeout,
            json_bearer_headers(
                config.api_key.as_deref(),
                &config.custom_headers,
                "invalid OpenAI TTS API key header",
            )?,
        )?;

        Ok(Self { config, client })
    }

    fn build_payload(&self, request: &TextToSpeechRequest) -> Result<OpenAiTextToSpeechRequest> {
        if request.text.trim().is_empty() {
            return Err(AstrbotError::Provider(
                "text-to-speech request must contain text".to_string(),
            ));
        }

        Ok(OpenAiTextToSpeechRequest {
            model: self.config.model.clone(),
            voice: self.config.voice.clone(),
            input: request.text.clone(),
            response_format: self.config.response_format.clone(),
        })
    }

    fn write_audio(&self, audio: &[u8]) -> Result<String> {
        self.config
            .artifact_writer()
            .write_audio(audio, "OpenAI TTS provider returned empty audio")
    }
}

#[async_trait]
impl TextToSpeechProvider for OpenAiTextToSpeechProvider {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        let response = self
            .client
            .post(self.config.speech_url())
            .json(&self.build_payload(&request)?)
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("OpenAI TTS request failed: {err}")))?;

        let status = response.status();
        let body = response.bytes().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;

        if !status.is_success() {
            let body = String::from_utf8_lossy(&body);
            return Err(AstrbotError::Provider(format!(
                "OpenAI TTS provider returned {status}: {}",
                extract_error_message(&body)
            )));
        }

        Ok(TextToSpeechResponse::new(self.write_audio(&body)?))
    }
}

#[derive(Debug, Serialize)]
struct OpenAiTextToSpeechRequest {
    model: String,
    voice: String,
    input: String,
    response_format: String,
}
