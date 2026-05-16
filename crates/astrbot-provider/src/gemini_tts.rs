use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};

use crate::media::{GeneratedMediaArtifactWriter, default_tts_output_dir};
use crate::protocol::tts::{
    build_gemini_tts_request, extract_gemini_tts_error_message, gemini_tts_generate_content_url,
    gemini_tts_wav_bytes, parse_gemini_tts_audio,
};
use crate::{TextToSpeechProvider, TextToSpeechRequest, TextToSpeechResponse};

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
        gemini_tts_generate_content_url(&self.api_base, &self.model)
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

    fn build_payload(
        &self,
        request: &TextToSpeechRequest,
    ) -> Result<impl serde::Serialize + use<>> {
        build_gemini_tts_request(
            request,
            &self.config.voice,
            self.config.prompt_prefix.as_deref(),
        )
    }

    fn write_wav_audio(&self, pcm_audio: &[u8]) -> Result<String> {
        if pcm_audio.is_empty() {
            return Err(AstrbotError::Provider(
                "Gemini TTS provider returned empty audio".to_string(),
            ));
        }

        self.config.artifact_writer().write_audio(
            &gemini_tts_wav_bytes(pcm_audio)?,
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
                extract_gemini_tts_error_message(&body)
            )));
        }

        let audio = parse_gemini_tts_audio(&body)?;
        Ok(TextToSpeechResponse::new(self.write_wav_audio(&audio)?))
    }
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
