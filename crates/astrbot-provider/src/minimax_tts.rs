use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde_json::{Value, json};

use crate::media::{GeneratedMediaArtifactWriter, default_tts_output_dir};
use crate::protocol::minimax_tts::{
    MiniMaxTtsRequest, MiniMaxTtsRequestOptions, build_minimax_tts_request,
    collect_minimax_sse_audio, extract_minimax_error_message,
};
use crate::{TextToSpeechProvider, TextToSpeechRequest, TextToSpeechResponse};

#[derive(Clone, Debug)]
pub struct MiniMaxTextToSpeechConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub group_id: String,
    pub model: String,
    pub language_boost: String,
    pub is_timber_weight: bool,
    pub timber_weights: Value,
    pub voice_speed: f32,
    pub voice_volume: f32,
    pub voice_pitch: f32,
    pub voice_id: String,
    pub voice_emotion: Option<String>,
    pub voice_latex_read: bool,
    pub voice_english_normalization: bool,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub output_dir: PathBuf,
}

impl MiniMaxTextToSpeechConfig {
    pub fn new(api_base: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: None,
            group_id: String::new(),
            model: model.into(),
            language_boost: "auto".to_string(),
            is_timber_weight: false,
            timber_weights: json!([{"voice_id": "Chinese (Mandarin)_Warm_Girl", "weight": 1}]),
            voice_speed: 1.0,
            voice_volume: 1.0,
            voice_pitch: 0.0,
            voice_id: String::new(),
            voice_emotion: None,
            voice_latex_read: false,
            voice_english_normalization: false,
            timeout: Duration::from_secs(60),
            custom_headers: HashMap::new(),
            output_dir: default_tts_output_dir(),
        }
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_group_id(mut self, group_id: impl Into<String>) -> Self {
        self.group_id = group_id.into();
        self
    }

    pub fn with_language_boost(mut self, language_boost: impl Into<String>) -> Self {
        self.language_boost = language_boost.into();
        self
    }

    pub fn with_timber_weight_enabled(mut self, enabled: bool) -> Self {
        self.is_timber_weight = enabled;
        self
    }

    pub fn with_timber_weights(mut self, timber_weights: Value) -> Self {
        self.timber_weights = timber_weights;
        self
    }

    pub fn with_voice_speed(mut self, speed: f32) -> Self {
        self.voice_speed = speed;
        self
    }

    pub fn with_voice_volume(mut self, volume: f32) -> Self {
        self.voice_volume = volume;
        self
    }

    pub fn with_voice_pitch(mut self, pitch: f32) -> Self {
        self.voice_pitch = pitch;
        self
    }

    pub fn with_voice_id(mut self, voice_id: impl Into<String>) -> Self {
        self.voice_id = voice_id.into();
        self
    }

    pub fn with_voice_emotion(mut self, emotion: impl Into<String>) -> Self {
        let emotion = emotion.into();
        self.voice_emotion = (!emotion.trim().is_empty() && emotion != "auto").then_some(emotion);
        self
    }

    pub fn with_voice_latex_read(mut self, enabled: bool) -> Self {
        self.voice_latex_read = enabled;
        self
    }

    pub fn with_voice_english_normalization(mut self, enabled: bool) -> Self {
        self.voice_english_normalization = enabled;
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
        format!(
            "{}?GroupId={}",
            self.api_base.trim_end_matches('/'),
            self.group_id
        )
    }

    fn artifact_writer(&self) -> GeneratedMediaArtifactWriter {
        GeneratedMediaArtifactWriter::new(self.output_dir.clone(), "minimax_tts_api", "mp3")
    }
}

#[derive(Clone, Debug)]
pub struct MiniMaxTextToSpeechProvider {
    config: MiniMaxTextToSpeechConfig,
    client: reqwest::Client,
}

impl MiniMaxTextToSpeechProvider {
    pub fn new(config: MiniMaxTextToSpeechConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .default_headers(build_headers(&config)?)
            .build()
            .map_err(|err| AstrbotError::Provider(format!("failed to build HTTP client: {err}")))?;

        Ok(Self { config, client })
    }

    fn build_payload(&self, request: &TextToSpeechRequest) -> Result<MiniMaxTtsRequest> {
        build_minimax_tts_request(
            request,
            MiniMaxTtsRequestOptions {
                model: &self.config.model,
                language_boost: &self.config.language_boost,
                is_timber_weight: self.config.is_timber_weight,
                timber_weights: &self.config.timber_weights,
                voice_speed: self.config.voice_speed,
                voice_volume: self.config.voice_volume,
                voice_pitch: self.config.voice_pitch,
                voice_id: &self.config.voice_id,
                voice_emotion: self.config.voice_emotion.as_deref(),
                voice_latex_read: self.config.voice_latex_read,
                voice_english_normalization: self.config.voice_english_normalization,
            },
        )
    }

    fn write_audio(&self, audio: &[u8]) -> Result<String> {
        self.config.artifact_writer().write_audio(
            audio,
            "MiniMax TTS API returned empty audio data. Please verify the group_id and voice configuration.",
        )
    }
}

#[async_trait]
impl TextToSpeechProvider for MiniMaxTextToSpeechProvider {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        let response = self
            .client
            .post(self.config.speech_url())
            .json(&self.build_payload(&request)?)
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("MiniMax TTS request failed: {err}")))?;

        let status = response.status();
        let body = response.text().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;

        if !status.is_success() {
            return Err(AstrbotError::Provider(format!(
                "MiniMax TTS provider returned {status}: {}",
                extract_minimax_error_message(&body)
            )));
        }

        let audio = collect_minimax_sse_audio(&body)?;
        Ok(TextToSpeechResponse::new(self.write_audio(&audio)?))
    }
}

fn build_headers(config: &MiniMaxTextToSpeechConfig) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/plain, */*"),
    );

    if let Some(api_key) = config
        .api_key
        .as_deref()
        .filter(|api_key| !api_key.trim().is_empty())
    {
        let bearer = format!("Bearer {api_key}");
        let value = HeaderValue::from_str(&bearer).map_err(|_| {
            AstrbotError::Provider("invalid MiniMax TTS API key header".to_string())
        })?;
        headers.insert(AUTHORIZATION, value);
    }

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
