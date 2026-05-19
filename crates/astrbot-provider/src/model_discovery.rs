use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;

use crate::capability::{ProviderModelDiscoverySupport, ProviderModelInfo};
use crate::constants::{
    ANTHROPIC_CHAT_PROVIDER_TYPE, GOOGLE_GENAI_CHAT_PROVIDER_TYPE, MOCK_CHAT_PROVIDER_TYPE,
    OPENAI_COMPATIBLE_CHAT_PROVIDER_TYPES, XINFERENCE_RERANK_PROVIDER_TYPE,
    XINFERENCE_SPEECH_TO_TEXT_PROVIDER_TYPE,
};
use crate::http::{
    build_http_client, extract_error_message, insert_custom_headers, join_api_path,
    json_api_key_headers, json_bearer_headers,
};

#[derive(Clone, Debug)]
pub struct ProviderModelDiscoveryConfig {
    pub provider_type: String,
    pub api_base: Option<String>,
    pub api_key: Option<String>,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
}

impl ProviderModelDiscoveryConfig {
    pub fn new(provider_type: impl Into<String>) -> Self {
        Self {
            provider_type: provider_type.into(),
            api_base: None,
            api_key: None,
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
        }
    }

    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = Some(api_base.into());
        self
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderModelDiscoveryResult {
    pub provider_type: String,
    pub models: Vec<ProviderModelInfo>,
    pub support: ProviderModelDiscoverySupport,
    pub source: String,
    pub dynamic: bool,
    pub unsupported: bool,
    pub error_kind: Option<String>,
    pub message: Option<String>,
}

impl ProviderModelDiscoveryResult {
    pub fn unsupported(provider_type: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            provider_type: provider_type.into(),
            models: Vec::new(),
            support: ProviderModelDiscoverySupport::Unsupported,
            source: "unsupported".to_string(),
            dynamic: false,
            unsupported: true,
            error_kind: None,
            message: Some(message.into()),
        }
    }
}

pub async fn discover_provider_models(
    config: ProviderModelDiscoveryConfig,
) -> Result<ProviderModelDiscoveryResult> {
    let provider_type = config.provider_type.trim().to_string();
    if provider_type == MOCK_CHAT_PROVIDER_TYPE {
        return Ok(ProviderModelDiscoveryResult {
            provider_type,
            models: default_model_candidates(MOCK_CHAT_PROVIDER_TYPE),
            support: ProviderModelDiscoverySupport::Unsupported,
            source: "static-suggestion".to_string(),
            dynamic: false,
            unsupported: true,
            error_kind: None,
            message: Some("mock provider does not expose a remote model list".to_string()),
        });
    }
    if provider_type == ANTHROPIC_CHAT_PROVIDER_TYPE {
        return Ok(ProviderModelDiscoveryResult::unsupported(
            provider_type,
            "Anthropic does not expose a stable get_models endpoint in this adapter",
        ));
    }

    let api_base = config
        .api_base
        .as_deref()
        .and_then(non_empty_string)
        .ok_or_else(|| {
            AstrbotError::Provider("provider api_base is required for model discovery".to_string())
        })?;
    let started = std::time::Instant::now();
    let models = if is_openai_compatible_provider(&provider_type) {
        discover_openai_compatible_models(&config, &api_base).await?
    } else if provider_type == GOOGLE_GENAI_CHAT_PROVIDER_TYPE {
        discover_gemini_models(&config, &api_base).await?
    } else if provider_type == XINFERENCE_SPEECH_TO_TEXT_PROVIDER_TYPE
        || provider_type == XINFERENCE_RERANK_PROVIDER_TYPE
    {
        discover_xinference_models(&config, &api_base).await?
    } else {
        return Ok(ProviderModelDiscoveryResult::unsupported(
            provider_type,
            "provider does not declare dynamic model discovery support",
        ));
    };

    let elapsed = started.elapsed().as_millis();
    let message = if models.is_empty() {
        "provider source returned no models".to_string()
    } else {
        format!("provider model discovery completed in {elapsed}ms")
    };

    Ok(ProviderModelDiscoveryResult {
        provider_type,
        models,
        support: ProviderModelDiscoverySupport::Supported,
        source: "runtime-model-list".to_string(),
        dynamic: true,
        unsupported: false,
        error_kind: None,
        message: Some(message),
    })
}

pub fn is_openai_compatible_provider(provider_type: &str) -> bool {
    OPENAI_COMPATIBLE_CHAT_PROVIDER_TYPES
        .iter()
        .any(|candidate| *candidate == provider_type)
}

pub fn model_discovery_support(provider_type: &str) -> ProviderModelDiscoverySupport {
    if is_openai_compatible_provider(provider_type)
        || provider_type == GOOGLE_GENAI_CHAT_PROVIDER_TYPE
        || provider_type == XINFERENCE_SPEECH_TO_TEXT_PROVIDER_TYPE
        || provider_type == XINFERENCE_RERANK_PROVIDER_TYPE
    {
        ProviderModelDiscoverySupport::Supported
    } else {
        ProviderModelDiscoverySupport::Unsupported
    }
}

pub fn default_model_candidates(provider_type: &str) -> Vec<ProviderModelInfo> {
    match provider_type {
        MOCK_CHAT_PROVIDER_TYPE => vec![ProviderModelInfo::new("mock-response")],
        ANTHROPIC_CHAT_PROVIDER_TYPE => vec![
            ProviderModelInfo::new("claude-3-5-sonnet-latest"),
            ProviderModelInfo::new("claude-3-5-haiku-latest"),
        ],
        GOOGLE_GENAI_CHAT_PROVIDER_TYPE => vec![
            ProviderModelInfo::new("gemini-2.0-flash"),
            ProviderModelInfo::new("gemini-1.5-pro"),
        ],
        XINFERENCE_SPEECH_TO_TEXT_PROVIDER_TYPE | XINFERENCE_RERANK_PROVIDER_TYPE => Vec::new(),
        provider_type if is_openai_compatible_provider(provider_type) => vec![
            ProviderModelInfo::new("gpt-4.1-mini").with_metadata("capability", "chat_completion"),
            ProviderModelInfo::new("gpt-4.1").with_metadata("capability", "chat_completion"),
            ProviderModelInfo::new("o3-mini").with_metadata("reasoning", "true"),
        ],
        "chat_completion" => vec![
            ProviderModelInfo::new("gpt-4.1-mini"),
            ProviderModelInfo::new("gpt-4.1"),
            ProviderModelInfo::new("o3-mini"),
        ],
        _ => Vec::new(),
    }
}

pub fn provider_model_ids(models: &[ProviderModelInfo]) -> Vec<String> {
    models.iter().map(|model| model.id.clone()).collect()
}

pub fn model_metadata_map(
    models: &[ProviderModelInfo],
) -> BTreeMap<String, BTreeMap<String, String>> {
    models
        .iter()
        .filter(|model| model.display_name.is_some() || !model.metadata.is_empty())
        .map(|model| {
            let mut metadata = model.metadata.clone();
            if let Some(display_name) = &model.display_name {
                metadata.insert("display_name".to_string(), display_name.clone());
            }
            (model.id.clone(), metadata)
        })
        .collect()
}

pub fn sanitize_model_discovery_error(error: &AstrbotError, secrets: &[&str]) -> String {
    let mut message = error.to_string();
    for secret in secrets
        .iter()
        .filter_map(|secret| non_empty_string(secret).map(|secret| secret.to_string()))
    {
        message = message.replace(&secret, "<redacted>");
        message = message.replace(&format!("Bearer {secret}"), "Bearer <redacted>");
    }
    message
}

async fn discover_openai_compatible_models(
    config: &ProviderModelDiscoveryConfig,
    api_base: &str,
) -> Result<Vec<ProviderModelInfo>> {
    let client = build_http_client(
        config.timeout,
        json_bearer_headers(
            config.api_key.as_deref(),
            &config.custom_headers,
            "invalid OpenAI-compatible API key header",
        )?,
    )?;
    let body = get_body_or_error(
        client.get(join_api_path(api_base, "models")).send().await,
        "provider model list",
    )
    .await?;
    parse_openai_models(&body)
}

async fn discover_gemini_models(
    config: &ProviderModelDiscoveryConfig,
    api_base: &str,
) -> Result<Vec<ProviderModelInfo>> {
    let client = build_http_client(
        config.timeout,
        json_api_key_headers(
            HeaderName::from_static("x-goog-api-key"),
            config.api_key.as_deref(),
            &config.custom_headers,
            "invalid Gemini API key header",
        )?,
    )?;
    let url = join_api_path(gemini_api_base(api_base).as_str(), "models");
    let body = get_body_or_error(client.get(url).send().await, "Gemini model list").await?;
    parse_gemini_models(&body)
}

async fn discover_xinference_models(
    config: &ProviderModelDiscoveryConfig,
    api_base: &str,
) -> Result<Vec<ProviderModelInfo>> {
    let client = build_http_client(config.timeout, xinference_headers(config)?)?;
    let body = get_body_or_error(
        client
            .get(join_api_path(api_base, "v1/models"))
            .send()
            .await,
        "Xinference model list",
    )
    .await?;
    parse_xinference_models(&body)
}

async fn get_body_or_error(
    response: std::result::Result<reqwest::Response, reqwest::Error>,
    label: &str,
) -> Result<String> {
    let response =
        response.map_err(|err| AstrbotError::Provider(format!("{label} request failed: {err}")))?;
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        AstrbotError::Provider(format!(
            "failed to read provider model list response: {err}"
        ))
    })?;

    if !status.is_success() {
        return Err(AstrbotError::Provider(format!(
            "{label} returned {status}: {}",
            extract_error_message(&body)
        )));
    }

    Ok(body)
}

fn parse_openai_models(body: &str) -> Result<Vec<ProviderModelInfo>> {
    let value = parse_json_body(body)?;
    let models = value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("id").and_then(Value::as_str))
        .filter_map(non_empty_string)
        .map(ProviderModelInfo::new)
        .collect::<Vec<_>>();
    Ok(models)
}

fn parse_gemini_models(body: &str) -> Result<Vec<ProviderModelInfo>> {
    let value = parse_json_body(body)?;
    let models = value
        .get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|item| {
            item.get("supportedGenerationMethods")
                .and_then(Value::as_array)
                .map_or(true, |methods| {
                    methods
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|method| method == "generateContent")
                })
        })
        .filter_map(|item| {
            item.get("name")
                .and_then(Value::as_str)
                .and_then(non_empty_string)
                .map(|name| {
                    let id = name.strip_prefix("models/").unwrap_or(&name).to_string();
                    let mut model = ProviderModelInfo::new(id);
                    if let Some(display_name) = item
                        .get("displayName")
                        .and_then(Value::as_str)
                        .and_then(non_empty_string)
                    {
                        model = model.with_display_name(display_name);
                    }
                    model
                })
        })
        .collect::<Vec<_>>();
    Ok(models)
}

fn parse_xinference_models(body: &str) -> Result<Vec<ProviderModelInfo>> {
    let value = parse_json_body(body)?;
    let candidates = if let Some(data) = value.get("data").and_then(Value::as_array) {
        data.iter().cloned().collect::<Vec<_>>()
    } else if let Some(object) = value.as_object() {
        object
            .iter()
            .map(|(uid, item)| {
                let mut item = item.clone();
                if let Some(object) = item.as_object_mut() {
                    object
                        .entry("id".to_string())
                        .or_insert_with(|| Value::String(uid.clone()));
                }
                item
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let models = candidates
        .iter()
        .filter_map(|item| {
            item.get("id")
                .and_then(Value::as_str)
                .or_else(|| item.get("model_uid").and_then(Value::as_str))
                .or_else(|| item.get("model_name").and_then(Value::as_str))
                .and_then(non_empty_string)
                .map(|id| {
                    let mut model = ProviderModelInfo::new(id);
                    if let Some(display_name) = item
                        .get("model_name")
                        .and_then(Value::as_str)
                        .and_then(non_empty_string)
                    {
                        model = model.with_display_name(display_name);
                    }
                    if let Some(model_type) = item
                        .get("model_type")
                        .and_then(Value::as_str)
                        .and_then(non_empty_string)
                    {
                        model = model.with_metadata("model_type", model_type);
                    }
                    model
                })
        })
        .collect::<Vec<_>>();
    Ok(models)
}

fn parse_json_body(body: &str) -> Result<Value> {
    serde_json::from_str(body).map_err(|err| {
        AstrbotError::Provider(format!("provider model list response is not JSON: {err}"))
    })
}

fn xinference_headers(config: &ProviderModelDiscoveryConfig) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    if let Some(api_key) = config.api_key.as_deref().and_then(non_empty_string) {
        let value = HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|_| AstrbotError::Provider("invalid Xinference API key header".to_string()))?;
        headers.insert(AUTHORIZATION, value);
    }
    insert_custom_headers(&mut headers, &config.custom_headers)?;
    Ok(headers)
}

fn gemini_api_base(api_base: &str) -> String {
    let api_base = api_base.trim_end_matches('/');
    if api_base.ends_with("/v1beta") {
        api_base.to_string()
    } else {
        format!("{api_base}/v1beta")
    }
}

fn non_empty_string(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty()).then_some(value)
}
