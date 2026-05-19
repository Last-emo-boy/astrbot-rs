use std::collections::HashMap;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue};
use reqwest::multipart;

use crate::http::{build_http_client, extract_error_message, insert_custom_headers, join_api_path};
use crate::model_resolver::{XinferenceModelResolver, XinferenceModelType};
use crate::protocol::xinference::parse_xinference_stt_text;
use crate::{
    AudioInputLoader, AudioMediaConverter, SpeechToTextProvider, SpeechToTextRequest,
    SpeechToTextResponse,
};

#[derive(Clone, Debug)]
pub struct XinferenceSpeechToTextConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub launch_model_if_not_running: bool,
}

impl XinferenceSpeechToTextConfig {
    pub fn new(api_base: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: None,
            model: model.into(),
            timeout: Duration::from_secs(180),
            custom_headers: HashMap::new(),
            launch_model_if_not_running: false,
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

    pub fn with_launch_model_if_not_running(mut self, launch_model_if_not_running: bool) -> Self {
        self.launch_model_if_not_running = launch_model_if_not_running;
        self
    }

    fn transcriptions_url(&self) -> String {
        join_api_path(&self.api_base, "v1/audio/transcriptions")
    }
}

#[derive(Clone, Debug)]
pub struct XinferenceSpeechToTextProvider {
    config: XinferenceSpeechToTextConfig,
    client: reqwest::Client,
    audio_loader: AudioInputLoader,
    model_resolver: XinferenceModelResolver,
}

impl XinferenceSpeechToTextProvider {
    pub fn new(config: XinferenceSpeechToTextConfig) -> Result<Self> {
        let client = build_http_client(config.timeout, build_headers(&config)?)?;
        let audio_loader = AudioInputLoader::new(config.timeout)?;
        let model_resolver = XinferenceModelResolver::new(
            client.clone(),
            &config.api_base,
            config.model.clone(),
            XinferenceModelType::Audio,
            config.launch_model_if_not_running,
        );

        Ok(Self {
            config,
            client,
            audio_loader,
            model_resolver,
        })
    }

    pub fn with_audio_converter(
        mut self,
        converter: std::sync::Arc<dyn AudioMediaConverter>,
    ) -> Self {
        self.audio_loader = self.audio_loader.with_converter(converter);
        self
    }

    fn build_form(&self, audio: Vec<u8>, model_uid: String) -> Result<multipart::Form> {
        let file = multipart::Part::bytes(audio)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|err| {
                AstrbotError::Provider(format!("failed to build audio multipart field: {err}"))
            })?;

        Ok(multipart::Form::new()
            .text("model", model_uid)
            .part("file", file))
    }
}

#[async_trait]
impl SpeechToTextProvider for XinferenceSpeechToTextProvider {
    async fn transcribe(&self, request: SpeechToTextRequest) -> Result<SpeechToTextResponse> {
        let audio = self
            .audio_loader
            .load(&request.audio_url, "Xinference STT")
            .await?;
        let model_uid = self.model_resolver.resolve_model_uid().await?;
        let response = self
            .client
            .post(self.config.transcriptions_url())
            .multipart(self.build_form(audio, model_uid)?)
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("Xinference STT request failed: {err}"))
            })?;

        let body = response_body_or_error(response, "Xinference STT provider").await?;
        Ok(SpeechToTextResponse::new(parse_xinference_stt_text(&body)?))
    }
}

fn build_headers(config: &XinferenceSpeechToTextConfig) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

    if let Some(api_key) = config
        .api_key
        .as_deref()
        .filter(|api_key| !api_key.trim().is_empty())
    {
        let bearer = format!("Bearer {api_key}");
        let value = HeaderValue::from_str(&bearer).map_err(|_| {
            AstrbotError::Provider("invalid Xinference STT API key header".to_string())
        })?;
        headers.insert(AUTHORIZATION, value);
    }

    insert_custom_headers(&mut headers, &config.custom_headers)?;

    Ok(headers)
}

async fn response_body_or_error(response: reqwest::Response, label: &str) -> Result<String> {
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        AstrbotError::Provider(format!("failed to read provider response: {err}"))
    })?;

    if !status.is_success() {
        return Err(AstrbotError::Provider(format!(
            "{label} returned {status}: {}",
            extract_error_message(&body)
        )));
    }

    Ok(body)
}
