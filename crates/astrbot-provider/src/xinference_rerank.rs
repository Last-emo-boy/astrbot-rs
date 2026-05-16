use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

use crate::http::{build_http_client, extract_error_message, join_api_path, json_bearer_headers};
use crate::protocol::xinference::{
    XinferenceLaunchModelRequest, XinferenceRerankRequest, parse_launch_model_uid,
    parse_running_model_uid, parse_xinference_rerank_response,
};
use crate::{RerankProvider, RerankRequest, RerankResponse};

#[derive(Clone, Debug)]
pub struct XinferenceRerankConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
    pub launch_model_if_not_running: bool,
}

impl XinferenceRerankConfig {
    pub fn new(api_base: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: None,
            model: model.into(),
            timeout: Duration::from_secs(120),
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

    fn rerank_url(&self) -> String {
        join_api_path(&self.api_base, "v1/rerank")
    }
}

#[derive(Clone, Debug)]
pub struct XinferenceRerankProvider {
    config: XinferenceRerankConfig,
    client: reqwest::Client,
    model_uid: Arc<Mutex<Option<String>>>,
}

impl XinferenceRerankProvider {
    pub fn new(config: XinferenceRerankConfig) -> Result<Self> {
        let client = build_http_client(
            config.timeout,
            json_bearer_headers(
                config.api_key.as_deref(),
                &config.custom_headers,
                "invalid Xinference rerank API key header",
            )?,
        )?;

        Ok(Self {
            config,
            client,
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
            "Xinference rerank model {} is not running and auto-launch is disabled",
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
                model_type: "rerank",
            })
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("Xinference launch model request failed: {err}"))
            })?;

        let body = response_body_or_error(response, "Xinference launch model").await?;
        parse_launch_model_uid(&body)
    }

    fn build_payload(
        &self,
        request: &RerankRequest,
        model_uid: String,
    ) -> Result<XinferenceRerankRequest> {
        if request.documents.is_empty() {
            return Err(AstrbotError::Provider(
                "rerank request must contain at least one document".to_string(),
            ));
        }

        Ok(XinferenceRerankRequest {
            model: request.model.clone().unwrap_or(model_uid),
            documents: request.documents.clone(),
            query: request.query.clone(),
            top_n: request.top_n,
        })
    }
}

#[async_trait]
impl RerankProvider for XinferenceRerankProvider {
    async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse> {
        let model_uid = self.resolve_model_uid().await?;
        let response = self
            .client
            .post(self.config.rerank_url())
            .json(&self.build_payload(&request, model_uid)?)
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("Xinference rerank request failed: {err}"))
            })?;

        let body = response_body_or_error(response, "Xinference rerank provider").await?;
        Ok(RerankResponse::new(parse_xinference_rerank_response(
            &body,
        )?))
    }
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
