use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue};
use reqwest::multipart;

use crate::http::{build_http_client, extract_error_message, insert_custom_headers, join_api_path};
use crate::protocol::xinference::{
    XinferenceLaunchModelRequest, parse_launch_model_uid, parse_running_model_uid,
    parse_xinference_stt_text,
};
use crate::{AudioInputLoader, SpeechToTextProvider, SpeechToTextRequest, SpeechToTextResponse};

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

    fn models_url(&self) -> String {
        join_api_path(&self.api_base, "v1/models")
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
    model_uid: Arc<Mutex<Option<String>>>,
}

impl XinferenceSpeechToTextProvider {
    pub fn new(config: XinferenceSpeechToTextConfig) -> Result<Self> {
        let client = build_http_client(config.timeout, build_headers(&config)?)?;
        let audio_loader = AudioInputLoader::new(config.timeout)?;

        Ok(Self {
            config,
            client,
            audio_loader,
            model_uid: Arc::new(Mutex::new(None)),
        })
    }

    fn cached_model_uid(&self) -> Result<Option<String>> {
        self.model_uid
            .lock()
            .map(|model_uid| model_uid.clone())
            .map_err(|_| AstrbotError::Provider("Xinference model UID cache poisoned".to_string()))
    }

    fn cache_model_uid(&self, model_uid: String) -> Result<String> {
        let mut cached = self.model_uid.lock().map_err(|_| {
            AstrbotError::Provider("Xinference model UID cache poisoned".to_string())
        })?;
        *cached = Some(model_uid.clone());
        Ok(model_uid)
    }

    async fn resolve_model_uid(&self) -> Result<String> {
        if let Some(model_uid) = self.cached_model_uid()? {
            return Ok(model_uid);
        }

        if let Some(model_uid) = self.find_running_model_uid().await? {
            return self.cache_model_uid(model_uid);
        }

        if self.config.launch_model_if_not_running {
            let model_uid = self.launch_model().await?;
            return self.cache_model_uid(model_uid);
        }

        Err(AstrbotError::Provider(format!(
            "Xinference STT model {} is not running and auto-launch is disabled",
            self.config.model
        )))
    }

    async fn find_running_model_uid(&self) -> Result<Option<String>> {
        let response = self
            .client
            .get(self.config.models_url())
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("Xinference list models request failed: {err}"))
            })?;

        let body = response_body_or_error(response, "Xinference list models").await?;
        parse_running_model_uid(&body, &self.config.model)
    }

    async fn launch_model(&self) -> Result<String> {
        let response = self
            .client
            .post(self.config.models_url())
            .json(&XinferenceLaunchModelRequest {
                model_name: self.config.model.clone(),
                model_type: "audio",
            })
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("Xinference launch model request failed: {err}"))
            })?;

        let body = response_body_or_error(response, "Xinference launch model").await?;
        parse_launch_model_uid(&body)
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
        let model_uid = self.resolve_model_uid().await?;
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
