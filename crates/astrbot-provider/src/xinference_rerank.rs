use std::collections::HashMap;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

use crate::http::{build_http_client, extract_error_message, join_api_path, json_bearer_headers};
use crate::model_resolver::{XinferenceModelResolver, XinferenceModelType};
use crate::protocol::xinference::{XinferenceRerankRequest, parse_xinference_rerank_response};
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

    fn rerank_url(&self) -> String {
        join_api_path(&self.api_base, "v1/rerank")
    }
}

#[derive(Clone, Debug)]
pub struct XinferenceRerankProvider {
    config: XinferenceRerankConfig,
    client: reqwest::Client,
    model_resolver: XinferenceModelResolver,
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
        let model_resolver = XinferenceModelResolver::new(
            client.clone(),
            &config.api_base,
            config.model.clone(),
            XinferenceModelType::Rerank,
            config.launch_model_if_not_running,
        );

        Ok(Self {
            config,
            client,
            model_resolver,
        })
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
        let model_uid = self.model_resolver.resolve_model_uid().await?;
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
