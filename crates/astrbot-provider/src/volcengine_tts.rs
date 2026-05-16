use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use base64::Engine as _;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::media::{GeneratedMediaArtifactWriter, default_tts_output_dir};
use crate::{TextToSpeechProvider, TextToSpeechRequest, TextToSpeechResponse};

const ERROR_TEXT_MAX_CHARS: usize = 4096;
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

    fn build_payload(&self, request: &TextToSpeechRequest) -> Result<VolcengineTtsRequest> {
        if request.text.trim().is_empty() {
            return Err(AstrbotError::Provider(
                "text-to-speech request must contain text".to_string(),
            ));
        }

        let token = self.config.api_key.clone().unwrap_or_default();
        Ok(VolcengineTtsRequest {
            app: VolcengineApp {
                appid: self.config.appid.clone(),
                token,
                cluster: self.config.cluster.clone(),
            },
            user: VolcengineUser {
                uid: next_audio_id(),
            },
            audio: VolcengineAudio {
                voice_type: self.config.voice_type.clone(),
                encoding: "mp3",
                speed_ratio: self.config.speed_ratio,
                volume_ratio: 1.0,
                pitch_ratio: 1.0,
            },
            request: VolcengineRequest {
                reqid: next_audio_id(),
                text: request.text.clone(),
                text_type: "plain",
                operation: "query",
                with_frontend: 1,
                frontend_type: "unitTson",
            },
        })
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
                extract_error_message(&body)
            )));
        }

        let payload: VolcengineTtsResponse = serde_json::from_str(&body).map_err(|err| {
            AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
        })?;
        let data = payload
            .data
            .as_deref()
            .map(str::trim)
            .filter(|data| !data.is_empty())
            .ok_or_else(|| {
                AstrbotError::Provider(format!(
                    "Volcengine TTS provider returned no audio data: {}",
                    payload
                        .message
                        .unwrap_or_else(|| "missing data".to_string())
                ))
            })?;
        let audio = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|err| {
                AstrbotError::Provider(format!("invalid Volcengine TTS audio data: {err}"))
            })?;
        Ok(TextToSpeechResponse::new(self.write_audio(&audio)?))
    }
}

#[derive(Debug, Serialize)]
struct VolcengineTtsRequest {
    app: VolcengineApp,
    user: VolcengineUser,
    audio: VolcengineAudio,
    request: VolcengineRequest,
}

#[derive(Debug, Serialize)]
struct VolcengineApp {
    appid: String,
    token: String,
    cluster: String,
}

#[derive(Debug, Serialize)]
struct VolcengineUser {
    uid: String,
}

#[derive(Debug, Serialize)]
struct VolcengineAudio {
    voice_type: String,
    encoding: &'static str,
    speed_ratio: f32,
    volume_ratio: f32,
    pitch_ratio: f32,
}

#[derive(Debug, Serialize)]
struct VolcengineRequest {
    reqid: String,
    text: String,
    text_type: &'static str,
    operation: &'static str,
    with_frontend: u8,
    frontend_type: &'static str,
}

#[derive(Debug, Deserialize)]
struct VolcengineTtsResponse {
    data: Option<String>,
    message: Option<String>,
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

fn extract_error_message(body: &str) -> String {
    let fallback = truncate(body.trim());
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return fallback;
    };

    let extracted = value
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .map(str::to_string);

    extracted.unwrap_or(fallback)
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= ERROR_TEXT_MAX_CHARS {
        return text.to_string();
    }

    text.chars().take(ERROR_TEXT_MAX_CHARS).collect()
}

fn next_audio_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = NEXT_AUDIO_ID.fetch_add(1, Ordering::Relaxed);
    format!("{timestamp}_{sequence}")
}
