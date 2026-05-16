use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};

use crate::media::{GeneratedMediaArtifactWriter, default_tts_output_dir};
use crate::protocol::tts::{
    VolcengineTtsRequestOptions, build_volcengine_tts_request,
    extract_volcengine_tts_error_message, parse_volcengine_tts_audio,
};
use crate::{TextToSpeechProvider, TextToSpeechRequest, TextToSpeechResponse};

static NEXT_AUDIO_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub struct VolcengineTextToSpeechConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub appid: String,
    pub cluster: String,
    pub voice_type: String,
    pub speed_ratio: f32,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub output_dir: PathBuf,
}

impl VolcengineTextToSpeechConfig {
    pub fn new(api_base: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: None,
            appid: String::new(),
            cluster: String::new(),
            voice_type: String::new(),
            speed_ratio: 1.0,
            timeout: Duration::from_secs(20),
            custom_headers: HashMap::new(),
            output_dir: default_tts_output_dir(),
        }
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_appid(mut self, appid: impl Into<String>) -> Self {
        self.appid = appid.into();
        self
    }

    pub fn with_cluster(mut self, cluster: impl Into<String>) -> Self {
        self.cluster = cluster.into();
        self
    }

    pub fn with_voice_type(mut self, voice_type: impl Into<String>) -> Self {
        self.voice_type = voice_type.into();
        self
    }

    pub fn with_speed_ratio(mut self, speed_ratio: f32) -> Self {
        self.speed_ratio = speed_ratio;
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
        self.api_base.trim_end_matches('/').to_string()
    }

    fn artifact_writer(&self) -> GeneratedMediaArtifactWriter {
        GeneratedMediaArtifactWriter::new(self.output_dir.clone(), "volcengine_tts", "mp3")
    }
}

#[derive(Clone, Debug)]
pub struct VolcengineTextToSpeechProvider {
    config: VolcengineTextToSpeechConfig,
    client: reqwest::Client,
}

impl VolcengineTextToSpeechProvider {
    pub fn new(config: VolcengineTextToSpeechConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .default_headers(build_headers(&config)?)
            .build()
            .map_err(|err| AstrbotError::Provider(format!("failed to build HTTP client: {err}")))?;

        Ok(Self { config, client })
    }

    fn build_payload(
        &self,
        request: &TextToSpeechRequest,
    ) -> Result<impl serde::Serialize + use<>> {
        let token = self.config.api_key.clone().unwrap_or_default();
        build_volcengine_tts_request(
            request,
            VolcengineTtsRequestOptions {
                appid: &self.config.appid,
                token: &token,
                cluster: &self.config.cluster,
                voice_type: &self.config.voice_type,
                speed_ratio: self.config.speed_ratio,
                uid: next_audio_id(),
                reqid: next_audio_id(),
            },
        )
    }

    fn write_audio(&self, audio: &[u8]) -> Result<String> {
        self.config
            .artifact_writer()
            .write_audio(audio, "Volcengine TTS provider returned empty audio")
    }
}

#[async_trait]
impl TextToSpeechProvider for VolcengineTextToSpeechProvider {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        let response = self
            .client
            .post(self.config.speech_url())
            .json(&self.build_payload(&request)?)
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("Volcengine TTS request failed: {err}"))
            })?;

        let status = response.status();
        let body = response.text().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;

        if !status.is_success() {
            return Err(AstrbotError::Provider(format!(
                "Volcengine TTS provider returned {status}: {}",
                extract_volcengine_tts_error_message(&body)
            )));
        }

        let audio = parse_volcengine_tts_audio(&body)?;
        Ok(TextToSpeechResponse::new(self.write_audio(&audio)?))
    }
}

fn build_headers(config: &VolcengineTextToSpeechConfig) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    if let Some(api_key) = config
        .api_key
        .as_deref()
        .filter(|api_key| !api_key.trim().is_empty())
    {
        let bearer = format!("Bearer; {api_key}");
        let value = HeaderValue::from_str(&bearer).map_err(|_| {
            AstrbotError::Provider("invalid Volcengine TTS API key header".to_string())
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

fn next_audio_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = NEXT_AUDIO_ID.fetch_add(1, Ordering::Relaxed);
    format!("{timestamp}_{sequence}")
}
