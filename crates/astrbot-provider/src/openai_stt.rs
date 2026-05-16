use std::collections::HashMap;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

use crate::http::{bearer_headers, build_http_client, extract_error_message, join_api_path};
use crate::protocol::speech::{build_openai_stt_form, parse_openai_stt_text};
use crate::{AudioInputLoader, SpeechToTextProvider, SpeechToTextRequest, SpeechToTextResponse};

#[derive(Clone, Debug)]
pub struct OpenAiSpeechToTextConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
}

impl OpenAiSpeechToTextConfig {
    pub fn new(api_base: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: None,
            model: model.into(),
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
        }
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
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

    fn transcriptions_url(&self) -> String {
        join_api_path(&self.api_base, "audio/transcriptions")
    }
}

#[derive(Clone, Debug)]
pub struct OpenAiSpeechToTextProvider {
    config: OpenAiSpeechToTextConfig,
    client: reqwest::Client,
    audio_loader: AudioInputLoader,
}

impl OpenAiSpeechToTextProvider {
    pub fn new(config: OpenAiSpeechToTextConfig) -> Result<Self> {
        let client = build_http_client(
            config.timeout,
            bearer_headers(
                config.api_key.as_deref(),
                &config.custom_headers,
                "invalid OpenAI STT API key header",
            )?,
        )?;
        let audio_loader = AudioInputLoader::new(config.timeout)?;

        Ok(Self {
            config,
            client,
            audio_loader,
        })
    }
}

#[async_trait]
impl SpeechToTextProvider for OpenAiSpeechToTextProvider {
    async fn transcribe(&self, request: SpeechToTextRequest) -> Result<SpeechToTextResponse> {
        let audio = self
            .audio_loader
            .load(&request.audio_url, "OpenAI STT")
            .await?;
        let response = self
            .client
            .post(self.config.transcriptions_url())
            .multipart(build_openai_stt_form(audio, &self.config.model)?)
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("OpenAI STT request failed: {err}")))?;

        let status = response.status();
        let body = response.text().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;

        if !status.is_success() {
            return Err(AstrbotError::Provider(format!(
                "OpenAI STT provider returned {status}: {}",
                extract_error_message(&body)
            )));
        }

        Ok(SpeechToTextResponse::new(parse_openai_stt_text(&body)?))
    }
}
