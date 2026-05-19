use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use base64::Engine as _;
use reqwest::header::{
    ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, USER_AGENT,
};
use serde_json::{Value, json};

use crate::http::{extract_error_message, join_api_path};
use crate::media::{GeneratedMediaArtifactWriter, default_tts_output_dir};
use crate::{TextToSpeechProvider, TextToSpeechRequest, TextToSpeechResponse};

const AZURE_KEY_32: usize = 32;
const AZURE_KEY_84: usize = 84;
const DEFAULT_ASTRBOT_USER_AGENT: &str = "AstrBot/0.1.0";
const ERROR_TEXT_MAX_CHARS: usize = 4096;

static NEXT_NONCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub struct AzureTextToSpeechConfig {
    pub subscription_key: String,
    pub region: String,
    pub voice: String,
    pub style: String,
    pub role: String,
    pub rate: String,
    pub volume: String,
    pub endpoint_override: Option<String>,
    pub token_url_override: Option<String>,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub proxy: Option<String>,
    pub output_dir: PathBuf,
}

impl AzureTextToSpeechConfig {
    pub fn new(subscription_key: impl Into<String>) -> Self {
        Self {
            subscription_key: subscription_key.into(),
            region: "eastus".to_string(),
            voice: "zh-CN-YunxiaNeural".to_string(),
            style: "cheerful".to_string(),
            role: "Boy".to_string(),
            rate: "1".to_string(),
            volume: "100".to_string(),
            endpoint_override: None,
            token_url_override: None,
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            proxy: None,
            output_dir: default_tts_output_dir(),
        }
    }

    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        let region = region.into();
        if !region.trim().is_empty() {
            self.region = region;
        }
        self
    }

    pub fn with_voice(mut self, voice: impl Into<String>) -> Self {
        let voice = voice.into();
        if !voice.trim().is_empty() {
            self.voice = voice;
        }
        self
    }

    pub fn with_style(mut self, style: impl Into<String>) -> Self {
        let style = style.into();
        if !style.trim().is_empty() {
            self.style = style;
        }
        self
    }

    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        let role = role.into();
        if !role.trim().is_empty() {
            self.role = role;
        }
        self
    }

    pub fn with_rate(mut self, rate: impl Into<String>) -> Self {
        let rate = rate.into();
        if !rate.trim().is_empty() {
            self.rate = rate;
        }
        self
    }

    pub fn with_volume(mut self, volume: impl Into<String>) -> Self {
        let volume = volume.into();
        if !volume.trim().is_empty() {
            self.volume = volume;
        }
        self
    }

    pub fn with_endpoint_override(mut self, endpoint: impl Into<String>) -> Self {
        let endpoint = endpoint.into();
        self.endpoint_override = (!endpoint.trim().is_empty()).then_some(endpoint);
        self
    }

    pub fn with_token_url_override(mut self, token_url: impl Into<String>) -> Self {
        let token_url = token_url.into();
        self.token_url_override = (!token_url.trim().is_empty()).then_some(token_url);
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

    fn token_url(&self) -> String {
        self.token_url_override.clone().unwrap_or_else(|| {
            format!(
                "https://{}.api.cognitive.microsoft.com/sts/v1.0/issuetoken",
                self.region
            )
        })
    }

    fn tts_url(&self) -> String {
        self.endpoint_override.clone().unwrap_or_else(|| {
            format!(
                "https://{}.tts.speech.microsoft.com/cognitiveservices/v1",
                self.region
            )
        })
    }

    fn artifact_writer(&self) -> GeneratedMediaArtifactWriter {
        GeneratedMediaArtifactWriter::new(self.output_dir.clone(), "azure_tts", "wav")
    }
}

#[derive(Clone, Debug)]
pub struct AzureTextToSpeechProvider {
    config: AzureTextToSpeechConfig,
    client: reqwest::Client,
}

impl AzureTextToSpeechProvider {
    pub fn new(config: AzureTextToSpeechConfig) -> Result<Self> {
        validate_azure_subscription_key(&config.subscription_key)?;
        let client = build_client(
            config.timeout,
            &config.custom_headers,
            config.proxy.as_deref(),
        )?;
        Ok(Self { config, client })
    }

    async fn issue_token(&self) -> Result<String> {
        let response = self
            .client
            .post(self.config.token_url())
            .header(
                "Ocp-Apim-Subscription-Key",
                self.config.subscription_key.as_str(),
            )
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("Azure TTS token request failed: {err}"))
            })?;
        let status = response.status();
        let body = response.text().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;
        if !status.is_success() {
            return Err(AstrbotError::Provider(format!(
                "Azure TTS token endpoint returned {status}: {}",
                extract_error_message(&body)
            )));
        }
        let token = body.trim();
        if token.is_empty() {
            return Err(AstrbotError::Provider(
                "Azure TTS token endpoint returned empty token".to_string(),
            ));
        }
        Ok(token.to_string())
    }

    fn build_ssml(&self, text: &str) -> String {
        format!(
            "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xmlns:mstts='http://www.w3.org/2001/mstts' xml:lang='zh-CN'><voice name='{}'><mstts:express-as style='{}' role='{}'><prosody rate='{}' volume='{}'>{}</prosody></mstts:express-as></voice></speak>",
            escape_xml(&self.config.voice),
            escape_xml(&self.config.style),
            escape_xml(&self.config.role),
            escape_xml(&self.config.rate),
            escape_xml(&self.config.volume),
            escape_xml(text)
        )
    }

    fn write_audio(&self, audio: &[u8]) -> Result<String> {
        self.config
            .artifact_writer()
            .write_audio(audio, "Azure TTS provider returned empty audio")
    }
}

#[async_trait]
impl TextToSpeechProvider for AzureTextToSpeechProvider {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        ensure_tts_text(&request)?;
        let token = self.issue_token().await?;
        let response = self
            .client
            .post(self.config.tts_url())
            .header(
                CONTENT_TYPE,
                HeaderValue::from_static("application/ssml+xml"),
            )
            .header(
                "X-Microsoft-OutputFormat",
                HeaderValue::from_static("riff-48khz-16bit-mono-pcm"),
            )
            .header(
                USER_AGENT,
                HeaderValue::from_static(DEFAULT_ASTRBOT_USER_AGENT),
            )
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .body(self.build_ssml(&request.text))
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("Azure TTS request failed: {err}")))?;
        let status = response.status();
        let body = response.bytes().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&body);
            return Err(AstrbotError::Provider(format!(
                "Azure TTS provider returned {status}: {}",
                extract_error_message(&body)
            )));
        }

        Ok(TextToSpeechResponse::new(self.write_audio(&body)?))
    }
}

#[derive(Clone, Debug)]
pub struct AzureOttsTextToSpeechConfig {
    pub skey: String,
    pub api_url: String,
    pub auth_time_url: String,
    pub voice: String,
    pub style: String,
    pub role: String,
    pub rate: String,
    pub volume: String,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub proxy: Option<String>,
    pub output_dir: PathBuf,
}

impl AzureOttsTextToSpeechConfig {
    pub fn new(
        skey: impl Into<String>,
        api_url: impl Into<String>,
        auth_time_url: impl Into<String>,
    ) -> Self {
        Self {
            skey: skey.into(),
            api_url: api_url.into(),
            auth_time_url: auth_time_url.into(),
            voice: "zh-CN-YunxiaNeural".to_string(),
            style: "cheerful".to_string(),
            role: "Boy".to_string(),
            rate: "1".to_string(),
            volume: "100".to_string(),
            timeout: Duration::from_secs(10),
            custom_headers: HashMap::new(),
            proxy: None,
            output_dir: default_tts_output_dir(),
        }
    }

    pub fn with_voice(mut self, voice: impl Into<String>) -> Self {
        let voice = voice.into();
        if !voice.trim().is_empty() {
            self.voice = voice;
        }
        self
    }

    pub fn with_style(mut self, style: impl Into<String>) -> Self {
        let style = style.into();
        if !style.trim().is_empty() {
            self.style = style;
        }
        self
    }

    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        let role = role.into();
        if !role.trim().is_empty() {
            self.role = role;
        }
        self
    }

    pub fn with_rate(mut self, rate: impl Into<String>) -> Self {
        let rate = rate.into();
        if !rate.trim().is_empty() {
            self.rate = rate;
        }
        self
    }

    pub fn with_volume(mut self, volume: impl Into<String>) -> Self {
        let volume = volume.into();
        if !volume.trim().is_empty() {
            self.volume = volume;
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

    fn artifact_writer(&self) -> GeneratedMediaArtifactWriter {
        GeneratedMediaArtifactWriter::new(self.output_dir.clone(), "otts_tts", "wav")
    }
}

#[derive(Clone, Debug)]
pub struct AzureOttsTextToSpeechProvider {
    config: AzureOttsTextToSpeechConfig,
    client: reqwest::Client,
}

impl AzureOttsTextToSpeechProvider {
    pub fn new(config: AzureOttsTextToSpeechConfig) -> Result<Self> {
        require_non_empty(&config.skey, "Azure OTTS_SKEY is required")?;
        require_non_empty(&config.api_url, "Azure OTTS_URL is required")?;
        require_non_empty(&config.auth_time_url, "Azure OTTS_AUTH_TIME is required")?;
        let client = build_client(
            config.timeout,
            &config.custom_headers,
            config.proxy.as_deref(),
        )?;
        Ok(Self { config, client })
    }

    async fn server_timestamp(&self) -> Result<i64> {
        let response = self
            .client
            .get(&self.config.auth_time_url)
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("Azure OTTS time sync failed: {err}")))?;
        let status = response.status();
        let body = response.text().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;
        if !status.is_success() {
            return Err(AstrbotError::Provider(format!(
                "Azure OTTS time sync returned {status}: {}",
                extract_error_message(&body)
            )));
        }
        serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|value| value.get("timestamp").and_then(Value::as_i64))
            .ok_or_else(|| {
                AstrbotError::Provider(
                    "Azure OTTS time sync response missing timestamp".to_string(),
                )
            })
    }

    async fn signed_url(&self) -> Result<String> {
        let timestamp = self.server_timestamp().await?;
        let nonce = next_nonce();
        let path = http_path(&self.config.api_url);
        let digest_input = format!("{path}-{timestamp}-{nonce}-0-{}", self.config.skey);
        let digest = format!("{:x}", md5::compute(digest_input.as_bytes()));
        let signature = format!("{timestamp}-{nonce}-0-{digest}");
        Ok(format!(
            "{}?sign={}",
            self.config.api_url,
            percent_encode(&signature)
        ))
    }

    fn write_audio(&self, audio: &[u8]) -> Result<String> {
        self.config
            .artifact_writer()
            .write_audio(audio, "Azure OTTS provider returned empty audio")
    }
}

#[async_trait]
impl TextToSpeechProvider for AzureOttsTextToSpeechProvider {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        ensure_tts_text(&request)?;
        let response = self
            .client
            .post(self.signed_url().await?)
            .header(
                USER_AGENT,
                HeaderValue::from_static(DEFAULT_ASTRBOT_USER_AGENT),
            )
            .header("UAK", HeaderValue::from_static("AstrBot/AzureTTS"))
            .form(&[
                ("text", request.text.as_str()),
                ("voice", self.config.voice.as_str()),
                ("style", self.config.style.as_str()),
                ("role", self.config.role.as_str()),
                ("rate", self.config.rate.as_str()),
                ("volume", self.config.volume.as_str()),
            ])
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("Azure OTTS request failed: {err}")))?;
        let status = response.status();
        let body = response.bytes().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&body);
            return Err(AstrbotError::Provider(format!(
                "Azure OTTS provider returned {status}: {}",
                extract_error_message(&body)
            )));
        }

        Ok(TextToSpeechResponse::new(self.write_audio(&body)?))
    }
}

#[derive(Clone, Debug)]
pub enum AzureCommercialTextToSpeechProvider {
    Native(AzureTextToSpeechProvider),
    Otts(AzureOttsTextToSpeechProvider),
}

impl AzureCommercialTextToSpeechProvider {
    pub fn from_configs(
        native_config: AzureTextToSpeechConfig,
        otts_config: Option<AzureOttsTextToSpeechConfig>,
    ) -> Result<Self> {
        if let Some(otts_config) = otts_config {
            return Ok(Self::Otts(AzureOttsTextToSpeechProvider::new(otts_config)?));
        }
        Ok(Self::Native(AzureTextToSpeechProvider::new(native_config)?))
    }
}

#[async_trait]
impl TextToSpeechProvider for AzureCommercialTextToSpeechProvider {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        match self {
            Self::Native(provider) => provider.synthesize(request).await,
            Self::Otts(provider) => provider.synthesize(request).await,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EdgeTextToSpeechConfig {
    pub api_base: String,
    pub voice: String,
    pub rate: Option<String>,
    pub volume: Option<String>,
    pub pitch: Option<String>,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub proxy: Option<String>,
    pub output_dir: PathBuf,
}

impl EdgeTextToSpeechConfig {
    pub fn new(api_base: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            voice: "zh-CN-XiaoxiaoNeural".to_string(),
            rate: None,
            volume: None,
            pitch: None,
            timeout: Duration::from_secs(30),
            custom_headers: HashMap::new(),
            proxy: None,
            output_dir: default_tts_output_dir(),
        }
    }

    pub fn with_voice(mut self, voice: impl Into<String>) -> Self {
        let voice = voice.into();
        if !voice.trim().is_empty() {
            self.voice = voice;
        }
        self
    }

    pub fn with_rate(mut self, rate: impl Into<String>) -> Self {
        self.rate = non_empty_option(rate);
        self
    }

    pub fn with_volume(mut self, volume: impl Into<String>) -> Self {
        self.volume = non_empty_option(volume);
        self
    }

    pub fn with_pitch(mut self, pitch: impl Into<String>) -> Self {
        self.pitch = non_empty_option(pitch);
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

    fn tts_url(&self) -> String {
        join_api_path(&self.api_base, "tts")
    }

    fn artifact_writer(&self) -> GeneratedMediaArtifactWriter {
        GeneratedMediaArtifactWriter::new(self.output_dir.clone(), "edge_tts", "wav")
    }
}

#[derive(Clone, Debug)]
pub struct EdgeTextToSpeechProvider {
    config: EdgeTextToSpeechConfig,
    client: reqwest::Client,
}

impl EdgeTextToSpeechProvider {
    pub fn new(config: EdgeTextToSpeechConfig) -> Result<Self> {
        require_non_empty(
            &config.api_base,
            "Edge TTS HTTP adapter api_base is required",
        )?;
        let client = build_client(
            config.timeout,
            &config.custom_headers,
            config.proxy.as_deref(),
        )?;
        Ok(Self { config, client })
    }

    fn build_payload(&self, request: &TextToSpeechRequest) -> Result<Value> {
        ensure_tts_text(request)?;
        let mut payload = json!({
            "text": request.text,
            "voice": self.config.voice,
        });
        if let Some(rate) = self.config.rate.as_deref() {
            payload["rate"] = json!(rate);
        }
        if let Some(volume) = self.config.volume.as_deref() {
            payload["volume"] = json!(volume);
        }
        if let Some(pitch) = self.config.pitch.as_deref() {
            payload["pitch"] = json!(pitch);
        }
        Ok(payload)
    }

    fn write_audio(&self, audio: &[u8]) -> Result<String> {
        self.config
            .artifact_writer()
            .write_audio(audio, "Edge TTS provider returned empty audio")
    }
}

#[async_trait]
impl TextToSpeechProvider for EdgeTextToSpeechProvider {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        let response = self
            .client
            .post(self.config.tts_url())
            .json(&self.build_payload(&request)?)
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("Edge TTS request failed: {err}")))?;
        let status = response.status();
        let body = response.bytes().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&body);
            return Err(AstrbotError::Provider(format!(
                "Edge TTS provider returned {status}: {}",
                extract_error_message(&body)
            )));
        }

        Ok(TextToSpeechResponse::new(self.write_audio(&body)?))
    }
}

#[derive(Clone, Debug)]
pub struct DashscopeTextToSpeechConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub model: String,
    pub voice: String,
    pub mode: DashscopeTtsMode,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub output_dir: PathBuf,
}

impl DashscopeTextToSpeechConfig {
    pub fn new(api_base: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: None,
            model: model.into(),
            voice: "loongstella".to_string(),
            mode: DashscopeTtsMode::Auto,
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
        let voice = voice.into();
        if !voice.trim().is_empty() {
            self.voice = voice;
        }
        self
    }

    pub fn with_mode(mut self, mode: DashscopeTtsMode) -> Self {
        self.mode = mode;
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

    fn tts_url(&self) -> String {
        match self.mode.resolved_for_model(&self.model) {
            DashscopeTtsMode::Qwen => join_api_path(&self.api_base, "qwen/tts"),
            DashscopeTtsMode::Cosyvoice | DashscopeTtsMode::Auto => {
                join_api_path(&self.api_base, "cosyvoice/tts")
            }
        }
    }

    fn artifact_writer(&self) -> GeneratedMediaArtifactWriter {
        GeneratedMediaArtifactWriter::new(self.output_dir.clone(), "dashscope_tts", "wav")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DashscopeTtsMode {
    Auto,
    Qwen,
    Cosyvoice,
}

impl DashscopeTtsMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "qwen" | "qwen_tts" | "qwen-tts" => Some(Self::Qwen),
            "cosyvoice" | "cosy" => Some(Self::Cosyvoice),
            _ => None,
        }
    }

    fn resolved_for_model(self, model: &str) -> Self {
        if self != Self::Auto {
            return self;
        }
        let model = model.to_ascii_lowercase();
        if model.starts_with("qwen") && model.contains("tts") {
            Self::Qwen
        } else {
            Self::Cosyvoice
        }
    }
}

#[derive(Clone, Debug)]
pub struct DashscopeTextToSpeechProvider {
    config: DashscopeTextToSpeechConfig,
    client: reqwest::Client,
}

impl DashscopeTextToSpeechProvider {
    pub fn new(config: DashscopeTextToSpeechConfig) -> Result<Self> {
        require_non_empty(
            config.api_key.as_deref().unwrap_or_default(),
            "Dashscope TTS api_key is required",
        )?;
        require_non_empty(&config.model, "Dashscope TTS model is required")?;
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .default_headers(json_auth_headers(
                config.api_key.as_deref(),
                &config.custom_headers,
                "invalid Dashscope TTS API key header",
            )?)
            .build()
            .map_err(|err| AstrbotError::Provider(format!("failed to build HTTP client: {err}")))?;
        Ok(Self { config, client })
    }

    fn build_payload(&self, request: &TextToSpeechRequest) -> Result<Value> {
        ensure_tts_text(request)?;
        Ok(json!({
            "model": self.config.model,
            "voice": self.config.voice,
            "text": request.text,
        }))
    }

    async fn parse_audio(&self, body: &[u8], content_type: Option<&str>) -> Result<Vec<u8>> {
        if content_type
            .is_some_and(|content_type| content_type.to_ascii_lowercase().starts_with("audio/"))
        {
            return Ok(body.to_vec());
        }

        let text = String::from_utf8_lossy(body);
        let value = serde_json::from_str::<Value>(&text).map_err(|err| {
            AstrbotError::Provider(format!("Dashscope TTS response was not valid JSON: {err}"))
        })?;
        if let Some(data) = value
            .pointer("/output/audio/data")
            .and_then(Value::as_str)
            .or_else(|| value.pointer("/audio/data").and_then(Value::as_str))
        {
            return base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|err| {
                    AstrbotError::Provider(format!("invalid Dashscope TTS base64 audio: {err}"))
                });
        }
        if let Some(url) = value
            .pointer("/output/audio/url")
            .and_then(Value::as_str)
            .or_else(|| value.pointer("/audio/url").and_then(Value::as_str))
        {
            return self.download_audio(url).await;
        }
        Err(AstrbotError::Provider(
            "Dashscope TTS response did not include audio data or url".to_string(),
        ))
    }

    async fn download_audio(&self, url: &str) -> Result<Vec<u8>> {
        let response = self.client.get(url).send().await.map_err(|err| {
            AstrbotError::Provider(format!("Dashscope TTS audio download failed: {err}"))
        })?;
        let status = response.status();
        let body = response.bytes().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&body);
            return Err(AstrbotError::Provider(format!(
                "Dashscope TTS audio download returned {status}: {}",
                extract_error_message(&body)
            )));
        }
        Ok(body.to_vec())
    }

    fn write_audio(&self, audio: &[u8]) -> Result<String> {
        self.config
            .artifact_writer()
            .write_audio(audio, "Dashscope TTS provider returned empty audio")
    }
}

#[async_trait]
impl TextToSpeechProvider for DashscopeTextToSpeechProvider {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        let response = self
            .client
            .post(self.config.tts_url())
            .json(&self.build_payload(&request)?)
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("Dashscope TTS request failed: {err}"))
            })?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let body = response.bytes().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&body);
            return Err(AstrbotError::Provider(format!(
                "Dashscope TTS provider returned {status}: {}",
                extract_error_message(&body)
            )));
        }

        let audio = self.parse_audio(&body, content_type.as_deref()).await?;
        Ok(TextToSpeechResponse::new(self.write_audio(&audio)?))
    }
}

#[derive(Clone, Debug)]
pub struct FishAudioTextToSpeechConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub reference_id: Option<String>,
    pub character: String,
    pub model: Option<String>,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub proxy: Option<String>,
    pub output_dir: PathBuf,
}

impl FishAudioTextToSpeechConfig {
    pub fn new(api_base: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: None,
            reference_id: None,
            character: "可莉".to_string(),
            model: None,
            timeout: Duration::from_secs(20),
            custom_headers: HashMap::new(),
            proxy: None,
            output_dir: default_tts_output_dir(),
        }
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_reference_id(mut self, reference_id: impl Into<String>) -> Self {
        let reference_id = reference_id.into();
        self.reference_id = (!reference_id.trim().is_empty()).then_some(reference_id);
        self
    }

    pub fn with_character(mut self, character: impl Into<String>) -> Self {
        let character = character.into();
        if !character.trim().is_empty() {
            self.character = character;
        }
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        let model = model.into();
        self.model = (!model.trim().is_empty()).then_some(model);
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

    fn model_lookup_base(&self) -> String {
        self.api_base
            .trim_end_matches('/')
            .trim_end_matches("/v1")
            .to_string()
    }

    fn tts_url(&self) -> String {
        join_api_path(&self.api_base, "tts")
    }

    fn artifact_writer(&self) -> GeneratedMediaArtifactWriter {
        GeneratedMediaArtifactWriter::new(self.output_dir.clone(), "fishaudio_tts_api", "wav")
    }
}

#[derive(Clone, Debug)]
pub struct FishAudioTextToSpeechProvider {
    config: FishAudioTextToSpeechConfig,
    client: reqwest::Client,
}

impl FishAudioTextToSpeechProvider {
    pub fn new(config: FishAudioTextToSpeechConfig) -> Result<Self> {
        require_non_empty(
            config.api_key.as_deref().unwrap_or_default(),
            "FishAudio TTS api_key is required",
        )?;
        if let Some(reference_id) = config.reference_id.as_deref() {
            validate_fishaudio_reference_id(reference_id)?;
        }
        let client = build_client(
            config.timeout,
            &config.custom_headers,
            config.proxy.as_deref(),
        )?;
        Ok(Self { config, client })
    }

    async fn reference_id(&self) -> Result<Option<String>> {
        if let Some(reference_id) = self.config.reference_id.as_deref() {
            return Ok(Some(reference_id.trim().to_string()));
        }

        for sort_by in ["score", "task_count", "created_at"] {
            let url = format!(
                "{}/model?title={}&sort_by={}",
                self.config.model_lookup_base(),
                percent_encode(&self.config.character),
                sort_by
            );
            let response = self
                .client
                .get(url)
                .headers(self.auth_headers("application/json")?)
                .send()
                .await
                .map_err(|err| {
                    AstrbotError::Provider(format!("FishAudio model lookup failed: {err}"))
                })?;
            let status = response.status();
            let body = response.text().await.map_err(|err| {
                AstrbotError::Provider(format!("failed to read provider response: {err}"))
            })?;
            if !status.is_success() {
                return Err(AstrbotError::Provider(format!(
                    "FishAudio model lookup returned {status}: {}",
                    extract_error_message(&body)
                )));
            }
            if let Some(reference_id) = find_fishaudio_reference_id(&body, &self.config.character)?
            {
                return Ok(Some(reference_id));
            }
        }
        Ok(None)
    }

    fn auth_headers(&self, content_type: &str) -> Result<HeaderMap> {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_str(content_type).map_err(|_| {
                AstrbotError::Provider("invalid FishAudio content-type header".to_string())
            })?,
        );
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
        let api_key = self.config.api_key.as_deref().unwrap_or_default();
        let value = HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|_| AstrbotError::Provider("invalid FishAudio API key header".to_string()))?;
        headers.insert(AUTHORIZATION, value);
        for (key, value) in &self.config.custom_headers {
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

    async fn build_payload(&self, request: &TextToSpeechRequest) -> Result<Value> {
        ensure_tts_text(request)?;
        let reference_id = self.reference_id().await?;
        let mut payload = json!({
            "text": request.text,
            "format": "wav",
            "reference_id": reference_id,
            "normalize": true,
            "latency": "normal",
            "chunk_length": 200,
        });
        if let Some(model) = self.config.model.as_deref() {
            payload["model"] = json!(model);
        }
        Ok(payload)
    }

    fn write_audio(&self, audio: &[u8]) -> Result<String> {
        self.config
            .artifact_writer()
            .write_audio(audio, "FishAudio TTS provider returned empty audio")
    }
}

#[async_trait]
impl TextToSpeechProvider for FishAudioTextToSpeechProvider {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        let payload = self.build_payload(&request).await?;
        let response = self
            .client
            .post(self.config.tts_url())
            .headers(self.auth_headers("application/json")?)
            .json(&payload)
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("FishAudio TTS request failed: {err}"))
            })?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
            .unwrap_or_default();
        let body = response.bytes().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;
        if status.is_success() && content_type.to_ascii_lowercase().starts_with("audio/") {
            return Ok(TextToSpeechResponse::new(self.write_audio(&body)?));
        }

        let body_text = String::from_utf8_lossy(&body);
        Err(AstrbotError::Provider(format!(
            "FishAudio TTS provider returned {status}: {}",
            truncate(body_text.trim())
        )))
    }
}

#[derive(Clone, Debug)]
pub struct GenieTextToSpeechConfig {
    pub api_base: String,
    pub character_name: String,
    pub language: String,
    pub onnx_model_dir: Option<String>,
    pub refer_audio_path: Option<String>,
    pub refer_text: Option<String>,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub output_dir: PathBuf,
}

impl GenieTextToSpeechConfig {
    pub fn new(api_base: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            character_name: "mika".to_string(),
            language: "Japanese".to_string(),
            onnx_model_dir: None,
            refer_audio_path: None,
            refer_text: None,
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
            output_dir: default_tts_output_dir(),
        }
    }

    pub fn with_character_name(mut self, character_name: impl Into<String>) -> Self {
        let character_name = character_name.into();
        if !character_name.trim().is_empty() {
            self.character_name = character_name;
        }
        self
    }

    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        let language = language.into();
        if !language.trim().is_empty() {
            self.language = language;
        }
        self
    }

    pub fn with_onnx_model_dir(mut self, model_dir: impl Into<String>) -> Self {
        self.onnx_model_dir = non_empty_option(model_dir);
        self
    }

    pub fn with_refer_audio_path(mut self, refer_audio_path: impl Into<String>) -> Self {
        self.refer_audio_path = non_empty_option(refer_audio_path);
        self
    }

    pub fn with_refer_text(mut self, refer_text: impl Into<String>) -> Self {
        self.refer_text = non_empty_option(refer_text);
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

    fn tts_url(&self) -> String {
        join_api_path(&self.api_base, "tts")
    }

    fn artifact_writer(&self) -> GeneratedMediaArtifactWriter {
        GeneratedMediaArtifactWriter::new(self.output_dir.clone(), "genie_tts", "wav")
    }
}

#[derive(Clone, Debug)]
pub struct GenieTextToSpeechProvider {
    config: GenieTextToSpeechConfig,
    client: reqwest::Client,
}

impl GenieTextToSpeechProvider {
    pub fn new(config: GenieTextToSpeechConfig) -> Result<Self> {
        require_non_empty(
            &config.api_base,
            "Genie TTS HTTP adapter api_base is required",
        )?;
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .default_headers(json_auth_headers(
                None,
                &config.custom_headers,
                "invalid Genie TTS header",
            )?)
            .build()
            .map_err(|err| AstrbotError::Provider(format!("failed to build HTTP client: {err}")))?;
        Ok(Self { config, client })
    }

    fn build_payload(&self, request: &TextToSpeechRequest) -> Result<Value> {
        ensure_tts_text(request)?;
        Ok(json!({
            "text": request.text,
            "character_name": self.config.character_name,
            "language": self.config.language,
            "onnx_model_dir": self.config.onnx_model_dir,
            "refer_audio_path": self.config.refer_audio_path,
            "refer_text": self.config.refer_text,
        }))
    }

    fn write_audio(&self, audio: &[u8]) -> Result<String> {
        self.config
            .artifact_writer()
            .write_audio(audio, "Genie TTS provider returned empty audio")
    }
}

#[async_trait]
impl TextToSpeechProvider for GenieTextToSpeechProvider {
    async fn synthesize(&self, request: TextToSpeechRequest) -> Result<TextToSpeechResponse> {
        let response = self
            .client
            .post(self.config.tts_url())
            .json(&self.build_payload(&request)?)
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("Genie TTS request failed: {err}")))?;
        let status = response.status();
        let body = response.bytes().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&body);
            return Err(AstrbotError::Provider(format!(
                "Genie TTS provider returned {status}: {}",
                extract_error_message(&body)
            )));
        }

        Ok(TextToSpeechResponse::new(self.write_audio(&body)?))
    }

    fn supports_streaming(&self) -> bool {
        true
    }
}

fn build_client(
    timeout: Duration,
    custom_headers: &HashMap<String, String>,
    proxy: Option<&str>,
) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .timeout(timeout)
        .default_headers(base_headers(custom_headers)?);
    if let Some(proxy) = proxy.filter(|proxy| !proxy.trim().is_empty()) {
        builder =
            builder.proxy(reqwest::Proxy::all(proxy).map_err(|err| {
                AstrbotError::Provider(format!("invalid TTS provider proxy: {err}"))
            })?);
    }
    builder
        .build()
        .map_err(|err| AstrbotError::Provider(format!("failed to build HTTP client: {err}")))
}

fn base_headers(custom_headers: &HashMap<String, String>) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    for (key, value) in custom_headers {
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

fn json_auth_headers(
    api_key: Option<&str>,
    custom_headers: &HashMap<String, String>,
    invalid_api_key_message: &str,
) -> Result<HeaderMap> {
    let mut headers = base_headers(custom_headers)?;
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if let Some(api_key) = api_key.filter(|api_key| !api_key.trim().is_empty()) {
        let value = HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|_| AstrbotError::Provider(invalid_api_key_message.to_string()))?;
        headers.insert(AUTHORIZATION, value);
    }
    Ok(headers)
}

fn ensure_tts_text(request: &TextToSpeechRequest) -> Result<()> {
    if request.text.trim().is_empty() {
        return Err(AstrbotError::Provider(
            "text-to-speech request must contain text".to_string(),
        ));
    }
    Ok(())
}

fn require_non_empty(value: &str, message: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(AstrbotError::Provider(message.to_string()))
    } else {
        Ok(())
    }
}

fn validate_azure_subscription_key(subscription_key: &str) -> Result<()> {
    let is_valid_len =
        subscription_key.len() == AZURE_KEY_32 || subscription_key.len() == AZURE_KEY_84;
    if is_valid_len
        && subscription_key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric())
    {
        Ok(())
    } else {
        Err(AstrbotError::Provider(
            "invalid Azure TTS subscription key; expected 32 or 84 alphanumeric characters"
                .to_string(),
        ))
    }
}

fn validate_fishaudio_reference_id(reference_id: &str) -> Result<()> {
    let valid =
        reference_id.len() == 32 && reference_id.bytes().all(|byte| byte.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err(AstrbotError::Provider(format!(
            "invalid FishAudio reference_id: {reference_id}"
        )))
    }
}

fn find_fishaudio_reference_id(body: &str, character: &str) -> Result<Option<String>> {
    let value = serde_json::from_str::<Value>(body).map_err(|err| {
        AstrbotError::Provider(format!(
            "FishAudio model lookup response was not valid JSON: {err}"
        ))
    })?;
    let Some(items) = value.get("items").and_then(Value::as_array) else {
        return Ok(None);
    };
    for item in items {
        let title = item
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if title.contains(character) {
            return Ok(item.get("_id").and_then(Value::as_str).map(str::to_string));
        }
    }
    Ok(None)
}

fn http_path(url: &str) -> String {
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let Some((_, path)) = without_scheme.split_once('/') else {
        return "/".to_string();
    };
    format!("/{path}")
}

fn next_nonce() -> String {
    let sequence = NEXT_NONCE.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{sequence:x}{:x}", nanos % 0xfffff)
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
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

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= ERROR_TEXT_MAX_CHARS {
        return text.to_string();
    }
    text.chars().take(ERROR_TEXT_MAX_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        DashscopeTtsMode, escape_xml, http_path, percent_encode, validate_azure_subscription_key,
        validate_fishaudio_reference_id,
    };

    #[test]
    fn helper_parsing_matches_source_adapter_shapes() {
        assert!(validate_azure_subscription_key(&"a".repeat(32)).is_ok());
        assert!(validate_azure_subscription_key("bad-key").is_err());
        assert!(validate_fishaudio_reference_id("626bb6d3f3364c9cbc3aa6a67300a664").is_ok());
        assert!(validate_fishaudio_reference_id("not-valid").is_err());
        assert_eq!(http_path("https://otts.example/v1/tts"), "/v1/tts");
        assert_eq!(percent_encode("hello world"), "hello%20world");
        assert_eq!(escape_xml("<hello&'\">"), "&lt;hello&amp;&apos;&quot;&gt;");
        assert_eq!(
            DashscopeTtsMode::Auto.resolved_for_model("qwen-tts-latest"),
            DashscopeTtsMode::Qwen
        );
        assert_eq!(
            DashscopeTtsMode::Auto.resolved_for_model("cosyvoice-v1"),
            DashscopeTtsMode::Cosyvoice
        );
    }
}
