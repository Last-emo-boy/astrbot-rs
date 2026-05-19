use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::media::{GeneratedMediaArtifactWriter, default_tts_output_dir};
use crate::{TextToSpeechProvider, TextToSpeechRequest, TextToSpeechResponse};

const ERROR_TEXT_MAX_CHARS: usize = 4096;

#[derive(Clone, Debug)]
pub struct GsvSelfhostTextToSpeechConfig {
    pub api_base: String,
    pub gpt_weights_path: Option<String>,
    pub sovits_weights_path: Option<String>,
    pub default_params: HashMap<String, String>,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub proxy: Option<String>,
    pub output_dir: PathBuf,
}

impl GsvSelfhostTextToSpeechConfig {
    pub fn new(api_base: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            gpt_weights_path: None,
            sovits_weights_path: None,
            default_params: HashMap::new(),
            timeout: Duration::from_secs(60),
            custom_headers: HashMap::new(),
            proxy: None,
            output_dir: default_tts_output_dir(),
        }
    }

    pub fn with_gpt_weights_path(mut self, path: impl Into<String>) -> Self {
        let path = path.into();
        self.gpt_weights_path = (!path.trim().is_empty()).then_some(path);
        self
    }

    pub fn with_sovits_weights_path(mut self, path: impl Into<String>) -> Self {
        let path = path.into();
        self.sovits_weights_path = (!path.trim().is_empty()).then_some(path);
        self
    }

    pub fn with_default_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        if !key.trim().is_empty() {
            self.default_params
                .insert(strip_gsv_prefix(&key), value.into());
        }
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

    pub fn with_proxy(mut self, proxy: impl Into<String>) -> Self {
        let proxy = proxy.into();
        self.proxy = (!proxy.trim().is_empty()).then_some(proxy);
        self
    }

    pub fn with_output_dir(mut self, output_dir: impl Into<PathBuf>) -> Self {
        self.output_dir = output_dir.into();
        self
    }

    fn endpoint_url(&self, endpoint: &str, params: &[(&str, &str)]) -> String {
        let mut url = format!(
            "{}/{}",
            self.api_base.trim_end_matches('/'),
            endpoint.trim_start_matches('/')
        );
        if !params.is_empty() {
            let query = params
                .iter()
                .map(|(key, value)| format!("{key}={}", percent_encode(value)))
                .collect::<Vec<_>>()
                .join("&");
            url.push('?');
            url.push_str(&query);
        }
        url
    }

    fn tts_url(&self, text: &str) -> String {
        let mut params = self
            .default_params
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        params.sort_by(|left, right| left.0.cmp(right.0));
        params.push(("text", text));
        self.endpoint_url("tts", &params)
    }

    fn artifact_writer(&self) -> GeneratedMediaArtifactWriter {
        GeneratedMediaArtifactWriter::new(self.output_dir.clone(), "gsv_tts", "wav")
    }
}

#[derive(Clone, Debug)]
pub struct GsvSelfhostTextToSpeechProvider {
    config: GsvSelfhostTextToSpeechConfig,
    client: reqwest::Client,
    initialized: Arc<AtomicBool>,
}

impl GsvSelfhostTextToSpeechProvider {
    pub fn new(config: GsvSelfhostTextToSpeechConfig) -> Result<Self> {
        let client = build_client(&config)?;
        Ok(Self {
            config,
            client,
            initialized: Arc::new(AtomicBool::new(false)),
        })
    }

    async fn request_bytes(&self, endpoint: &str, params: &[(&str, &str)]) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(self.config.endpoint_url(endpoint, params))
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("GSV TTS request failed: {err}")))?;
        let status = response.status();
        let body = response.bytes().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&body);
            return Err(AstrbotError::Provider(format!(
                "GSV TTS provider returned {status}: {}",
                truncate(body.trim())
            )));
        }
        Ok(body.to_vec())
    }

    pub async fn initialize(&self) -> Result<()> {
        if self.initialized.load(Ordering::SeqCst) {
            return Ok(());
        }
        if let Some(path) = self.config.gpt_weights_path.as_deref() {
            self.request_bytes("set_gpt_weights", &[("weights_path", path)])
                .await?;
        }
        if let Some(path) = self.config.sovits_weights_path.as_deref() {
            self.request_bytes("set_sovits_weights", &[("weights_path", path)])
                .await?;
        }
        self.initialized.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn write_audio(&self, audio: &[u8]) -> Result<String> {
        self.config
            .artifact_writer()
            .write_audio(audio, "GSV TTS provider returned empty audio")
    }
}

#[async_trait]
impl TextToSpeechProvider for GsvSelfhostTextToSpeechProvider {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        if request.text.trim().is_empty() {
            return Err(AstrbotError::Provider(
                "text-to-speech request must contain text".to_string(),
            ));
        }

        self.initialize().await?;
        let response = self
            .client
            .get(self.config.tts_url(&request.text))
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("GSV TTS request failed: {err}")))?;
        let status = response.status();
        let body = response.bytes().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&body);
            return Err(AstrbotError::Provider(format!(
                "GSV TTS provider returned {status}: {}",
                truncate(body.trim())
            )));
        }

        Ok(TextToSpeechResponse::new(self.write_audio(&body)?))
    }

    async fn terminate(&self) -> Result<()> {
        Ok(())
    }
}

fn build_client(config: &GsvSelfhostTextToSpeechConfig) -> Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    for (key, value) in &config.custom_headers {
        let name = HeaderName::from_bytes(key.as_bytes()).map_err(|_| {
            AstrbotError::Provider(format!("invalid custom provider header name: {key}"))
        })?;
        let value = HeaderValue::from_str(value).map_err(|_| {
            AstrbotError::Provider(format!("invalid custom provider header value for: {key}"))
        })?;
        headers.insert(name, value);
    }

    let mut builder = reqwest::Client::builder()
        .timeout(config.timeout)
        .default_headers(headers);
    if let Some(proxy) = config.proxy.as_deref() {
        builder = builder.proxy(
            reqwest::Proxy::all(proxy)
                .map_err(|err| AstrbotError::Provider(format!("invalid GSV TTS proxy: {err}")))?,
        );
    }
    builder
        .build()
        .map_err(|err| AstrbotError::Provider(format!("failed to build HTTP client: {err}")))
}

fn strip_gsv_prefix(key: &str) -> String {
    let key = key.strip_prefix("gsv_default_parms.").unwrap_or(key);
    key.strip_prefix("gsv_").unwrap_or(key).to_string()
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char);
            }
            _ => {
                let _ = write!(&mut encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= ERROR_TEXT_MAX_CHARS {
        return text.to_string();
    }

    text.chars().take(ERROR_TEXT_MAX_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::{percent_encode, strip_gsv_prefix};

    #[test]
    fn gsv_query_helpers_match_source_adapter_shape() {
        assert_eq!(strip_gsv_prefix("gsv_prompt_text"), "prompt_text");
        assert_eq!(strip_gsv_prefix("text_lang"), "text_lang");
        assert_eq!(percent_encode("hello world"), "hello%20world");
        assert_eq!(percent_encode("可莉"), "%E5%8F%AF%E8%8E%89");
    }
}
