use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::multipart;
use serde::Deserialize;

use crate::http::{extract_error_message, insert_custom_headers, join_api_path};
use crate::{
    AudioInputLoader, AudioMediaConverter, SpeechToTextProvider, SpeechToTextRequest,
    SpeechToTextResponse,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelfhostSpeechToTextKind {
    OpenAiWhisper,
    SenseVoice,
}

impl SelfhostSpeechToTextKind {
    fn default_model(self) -> &'static str {
        match self {
            Self::OpenAiWhisper => "tiny",
            Self::SenseVoice => "iic/SenseVoiceSmall",
        }
    }

    fn default_endpoint(self) -> &'static str {
        match self {
            Self::OpenAiWhisper => "audio/transcriptions",
            Self::SenseVoice => "audio/transcriptions",
        }
    }

    fn model_field(self) -> &'static str {
        match self {
            Self::OpenAiWhisper => "model",
            Self::SenseVoice => "stt_model",
        }
    }

    fn provider_label(self) -> &'static str {
        match self {
            Self::OpenAiWhisper => "Whisper selfhost STT",
            Self::SenseVoice => "SenseVoice selfhost STT",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SelfhostSpeechToTextConfig {
    pub api_base: String,
    pub endpoint: String,
    pub model: String,
    pub kind: SelfhostSpeechToTextKind,
    pub timeout: Duration,
    pub api_key: Option<String>,
    pub custom_headers: HashMap<String, String>,
    pub proxy: Option<String>,
    pub is_emotion: bool,
    pub extra_form_fields: HashMap<String, String>,
}

impl SelfhostSpeechToTextConfig {
    pub fn new(
        kind: SelfhostSpeechToTextKind,
        api_base: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            api_base: api_base.into(),
            endpoint: kind.default_endpoint().to_string(),
            model: model.into(),
            kind,
            timeout: Duration::from_secs(120),
            api_key: None,
            custom_headers: HashMap::new(),
            proxy: None,
            is_emotion: false,
            extra_form_fields: HashMap::new(),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_headers.insert(key.into(), value.into());
        self
    }

    pub fn with_proxy(mut self, proxy: impl Into<String>) -> Self {
        let proxy = proxy.into();
        self.proxy = (!proxy.trim().is_empty()).then_some(proxy);
        self
    }

    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        let endpoint = endpoint.into();
        if !endpoint.trim().is_empty() {
            self.endpoint = endpoint;
        }
        self
    }

    pub fn with_emotion(mut self, enabled: bool) -> Self {
        self.is_emotion = enabled;
        self
    }

    pub fn with_form_field(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_form_fields.insert(key.into(), value.into());
        self
    }

    fn transcription_url(&self) -> String {
        join_api_path(&self.api_base, &self.endpoint)
    }
}

impl SelfhostSpeechToTextConfig {
    pub fn with_default_model(kind: SelfhostSpeechToTextKind, api_base: impl Into<String>) -> Self {
        Self::new(kind, api_base, kind.default_model())
    }
}

#[derive(Clone, Debug)]
pub struct SelfhostSpeechToTextProvider {
    config: SelfhostSpeechToTextConfig,
    client: reqwest::Client,
    audio_loader: AudioInputLoader,
}

impl SelfhostSpeechToTextProvider {
    pub fn new(config: SelfhostSpeechToTextConfig) -> Result<Self> {
        let client = build_client(&config)?;
        let audio_loader = AudioInputLoader::new(config.timeout)?;

        Ok(Self {
            config,
            client,
            audio_loader,
        })
    }

    pub fn with_audio_converter(mut self, converter: Arc<dyn AudioMediaConverter>) -> Self {
        self.audio_loader = self.audio_loader.with_converter(converter);
        self
    }

    fn build_form(&self, audio: Vec<u8>) -> Result<multipart::Form> {
        let file = multipart::Part::bytes(audio)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|err| {
                AstrbotError::Provider(format!("failed to build audio multipart field: {err}"))
            })?;

        let mut form = multipart::Form::new()
            .text(self.config.kind.model_field(), self.config.model.clone())
            .part("file", file);
        if self.config.kind == SelfhostSpeechToTextKind::SenseVoice {
            form = form.text("is_emotion", self.config.is_emotion.to_string());
        }
        for (key, value) in &self.config.extra_form_fields {
            form = form.text(key.clone(), value.clone());
        }

        Ok(form)
    }
}

#[async_trait]
impl SpeechToTextProvider for SelfhostSpeechToTextProvider {
    async fn transcribe(&self, request: SpeechToTextRequest) -> Result<SpeechToTextResponse> {
        let audio = self
            .audio_loader
            .load(&request.audio_url, self.config.kind.provider_label())
            .await?;
        let response = self
            .client
            .post(self.config.transcription_url())
            .multipart(self.build_form(audio)?)
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!(
                    "{} request failed: {err}",
                    self.config.kind.provider_label()
                ))
            })?;

        let status = response.status();
        let body = response.text().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;

        if !status.is_success() {
            return Err(AstrbotError::Provider(format!(
                "{} provider returned {status}: {}",
                self.config.kind.provider_label(),
                extract_error_message(&body)
            )));
        }

        Ok(SpeechToTextResponse::new(parse_selfhost_stt_text(&body)?))
    }
}

fn build_client(config: &SelfhostSpeechToTextConfig) -> Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    if let Some(api_key) = config
        .api_key
        .as_deref()
        .filter(|api_key| !api_key.trim().is_empty())
    {
        let value = HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|_| {
            AstrbotError::Provider("invalid selfhost STT API key header".to_string())
        })?;
        headers.insert(reqwest::header::AUTHORIZATION, value);
    }
    insert_custom_headers(&mut headers, &config.custom_headers)?;

    let mut builder = reqwest::Client::builder()
        .timeout(config.timeout)
        .default_headers(headers);
    if let Some(proxy) = config.proxy.as_deref() {
        builder =
            builder.proxy(reqwest::Proxy::all(proxy).map_err(|err| {
                AstrbotError::Provider(format!("invalid selfhost STT proxy: {err}"))
            })?);
    }
    builder
        .build()
        .map_err(|err| AstrbotError::Provider(format!("failed to build HTTP client: {err}")))
}

#[derive(Debug, Deserialize)]
struct TextResponse {
    text: Option<String>,
    result: Option<String>,
    transcription: Option<String>,
}

fn parse_selfhost_stt_text(body: &str) -> Result<String> {
    let payload: TextResponse = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
    })?;
    let text = payload
        .text
        .or(payload.result)
        .or(payload.transcription)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .ok_or_else(|| {
            AstrbotError::Provider(
                "provider response did not contain transcription text".to_string(),
            )
        })?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::parse_selfhost_stt_text;

    #[test]
    fn parser_accepts_common_selfhost_stt_text_fields() {
        assert_eq!(
            parse_selfhost_stt_text(r#"{"text":"hello"}"#).expect("text should parse"),
            "hello"
        );
        assert_eq!(
            parse_selfhost_stt_text(r#"{"result":"hi"}"#).expect("result should parse"),
            "hi"
        );
        assert!(parse_selfhost_stt_text(r#"{"text":" "}"#).is_err());
    }
}
