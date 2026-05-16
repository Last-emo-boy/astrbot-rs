use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use base64::Engine as _;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::media::{GeneratedMediaArtifactWriter, default_tts_output_dir};
use crate::{TextToSpeechProvider, TextToSpeechRequest, TextToSpeechResponse};

const ERROR_TEXT_MAX_CHARS: usize = 4096;

#[derive(Clone, Debug)]
pub struct GeminiTextToSpeechConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub model: String,
    pub voice: String,
    pub prompt_prefix: Option<String>,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub output_dir: PathBuf,
}

impl GeminiTextToSpeechConfig {
    pub fn new(api_base: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: None,
            model: model.into(),
            voice: "Leda".to_string(),
            prompt_prefix: None,
            timeout: Duration::from_secs(20),
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

    pub fn with_prompt_prefix(mut self, prompt_prefix: impl Into<String>) -> Self {
        let prompt_prefix = prompt_prefix.into();
        self.prompt_prefix = (!prompt_prefix.trim().is_empty()).then_some(prompt_prefix);
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

    fn generate_content_url(&self) -> String {
        let api_base = self.api_base.trim_end_matches('/');
        let model = self.model.trim_start_matches("models/");
        if api_base.ends_with("/v1beta") {
            format!("{api_base}/models/{model}:generateContent")
        } else {
            format!("{api_base}/v1beta/models/{model}:generateContent")
        }
    }

    fn artifact_writer(&self) -> GeneratedMediaArtifactWriter {
        GeneratedMediaArtifactWriter::new(self.output_dir.clone(), "gemini_tts", "wav")
    }
}

#[derive(Clone, Debug)]
pub struct GeminiTextToSpeechProvider {
    config: GeminiTextToSpeechConfig,
    client: reqwest::Client,
}

impl GeminiTextToSpeechProvider {
    pub fn new(config: GeminiTextToSpeechConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .default_headers(build_headers(&config)?)
            .build()
            .map_err(|err| AstrbotError::Provider(format!("failed to build HTTP client: {err}")))?;

        Ok(Self { config, client })
    }

    fn build_payload(&self, request: &TextToSpeechRequest) -> Result<GeminiTtsRequest> {
        if request.text.trim().is_empty() {
            return Err(AstrbotError::Provider(
                "text-to-speech request must contain text".to_string(),
            ));
        }

        let prompt = match self.config.prompt_prefix.as_deref() {
            Some(prefix) => format!("{prefix}: {}", request.text),
            None => request.text.clone(),
        };

        Ok(GeminiTtsRequest {
            contents: vec![GeminiContent {
                parts: vec![GeminiPart { text: prompt }],
            }],
            generation_config: GeminiGenerationConfig {
                response_modalities: vec!["AUDIO"],
                speech_config: GeminiSpeechConfig {
                    voice_config: GeminiVoiceConfig {
                        prebuilt_voice_config: GeminiPrebuiltVoiceConfig {
                            voice_name: self.config.voice.clone(),
                        },
                    },
                },
            },
        })
    }

    fn write_wav_audio(&self, pcm_audio: &[u8]) -> Result<String> {
        if pcm_audio.is_empty() {
            return Err(AstrbotError::Provider(
                "Gemini TTS provider returned empty audio".to_string(),
            ));
        }

        self.config.artifact_writer().write_audio(
            &wav_bytes(pcm_audio)?,
            "Gemini TTS provider returned empty audio",
        )
    }
}

#[async_trait]
impl TextToSpeechProvider for GeminiTextToSpeechProvider {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        let response = self
            .client
            .post(self.config.generate_content_url())
            .json(&self.build_payload(&request)?)
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("Gemini TTS request failed: {err}")))?;

        let status = response.status();
        let body = response.text().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;

        if !status.is_success() {
            return Err(AstrbotError::Provider(format!(
                "Gemini TTS provider returned {status}: {}",
                extract_error_message(&body)
            )));
        }

        let payload: GeminiTtsResponse = serde_json::from_str(&body).map_err(|err| {
            AstrbotError::Provider(format!("failed to parse provider response JSON: {err}"))
        })?;
        let audio = payload.into_audio()?;
        Ok(TextToSpeechResponse::new(self.write_wav_audio(&audio)?))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiTtsRequest {
    contents: Vec<GeminiContent>,
    generation_config: GeminiGenerationConfig,
}

#[derive(Debug, Serialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiGenerationConfig {
    response_modalities: Vec<&'static str>,
    speech_config: GeminiSpeechConfig,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiSpeechConfig {
    voice_config: GeminiVoiceConfig,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiVoiceConfig {
    prebuilt_voice_config: GeminiPrebuiltVoiceConfig,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiPrebuiltVoiceConfig {
    voice_name: String,
}

#[derive(Debug, Deserialize)]
struct GeminiTtsResponse {
    #[serde(default)]
    candidates: Vec<GeminiTtsCandidate>,
}

impl GeminiTtsResponse {
    fn into_audio(self) -> Result<Vec<u8>> {
        let inline_data = self
            .candidates
            .into_iter()
            .find_map(|candidate| {
                candidate
                    .content
                    .and_then(|content| content.parts.into_iter().find_map(|part| part.inline_data))
            })
            .ok_or_else(|| {
                AstrbotError::Provider("No audio content returned from Gemini TTS API".to_string())
            })?;

        base64::engine::general_purpose::STANDARD
            .decode(inline_data.data.trim())
            .map_err(|err| AstrbotError::Provider(format!("invalid Gemini TTS audio data: {err}")))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiTtsCandidate {
    content: Option<GeminiTtsContent>,
}

#[derive(Debug, Deserialize)]
struct GeminiTtsContent {
    #[serde(default)]
    parts: Vec<GeminiTtsPart>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiTtsPart {
    inline_data: Option<GeminiInlineData>,
}

#[derive(Debug, Deserialize)]
struct GeminiInlineData {
    data: String,
}

fn build_headers(config: &GeminiTextToSpeechConfig) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    if let Some(api_key) = config
        .api_key
        .as_deref()
        .filter(|api_key| !api_key.trim().is_empty())
    {
        let value = HeaderValue::from_str(api_key)
            .map_err(|_| AstrbotError::Provider("invalid Gemini TTS API key header".to_string()))?;
        headers.insert("x-goog-api-key", value);
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

fn wav_bytes(pcm_audio: &[u8]) -> Result<Vec<u8>> {
    let data_len = u32::try_from(pcm_audio.len()).map_err(|_| {
        AstrbotError::Provider("Gemini TTS audio is too large to write as WAV".to_string())
    })?;
    let riff_len = 36_u32
        .checked_add(data_len)
        .ok_or_else(|| AstrbotError::Provider("Gemini TTS WAV size overflow".to_string()))?;
    let sample_rate = 24_000_u32;
    let channels = 1_u16;
    let bits_per_sample = 16_u16;
    let block_align = channels * bits_per_sample / 8;
    let byte_rate = sample_rate * u32::from(block_align);

    let mut wav = Vec::with_capacity(44 + pcm_audio.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_len.to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(pcm_audio);

    Ok(wav)
}

fn extract_error_message(body: &str) -> String {
    let fallback = truncate(body.trim());
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return fallback;
    };

    let extracted = value
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
        .or_else(|| value.get("message").and_then(Value::as_str))
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
