use std::collections::HashMap;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

use crate::http::{build_http_client, extract_error_message, join_api_path, json_bearer_headers};
use crate::protocol::rerank::{build_vllm_rerank_request, parse_vllm_rerank_response};
use crate::{RerankProvider, RerankRequest, RerankResponse};

#[derive(Clone, Debug)]
pub struct VllmRerankConfig {
    pub api_base: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout: Duration,
    pub custom_headers: HashMap<String, String>,
}

impl VllmRerankConfig {
    pub fn new(api_base: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: None,
            model: model.into(),
            timeout: Duration::from_secs(120),
            custom_headers: HashMap::new(),
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

    fn rerank_url(&self) -> String {
        join_api_path(&self.api_base, "v1/rerank")
    }
}

#[derive(Clone, Debug)]
pub struct VllmRerankProvider {
    config: VllmRerankConfig,
    client: reqwest::Client,
}

impl VllmRerankProvider {
    pub fn new(config: VllmRerankConfig) -> Result<Self> {
        let client = build_http_client(
            config.timeout,
            json_bearer_headers(
                config.api_key.as_deref(),
                &config.custom_headers,
                "invalid VLLM rerank API key header",
            )?,
        )?;

        Ok(Self { config, client })
    }

    fn build_payload(&self, request: &RerankRequest) -> Result<impl serde::Serialize + use<>> {
        build_vllm_rerank_request(request, &self.config.model)
    }
}

#[async_trait]
impl RerankProvider for VllmRerankProvider {
    async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse> {
        let response = self
            .client
            .post(self.config.rerank_url())
            .json(&self.build_payload(&request)?)
            .send()
            .await
            .map_err(|err| AstrbotError::Provider(format!("VLLM rerank request failed: {err}")))?;

        let status = response.status();
        let body = response.text().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read provider response: {err}"))
        })?;

        if !status.is_success() {
            return Err(AstrbotError::Provider(format!(
                "VLLM rerank provider returned {status}: {}",
                extract_error_message(&body)
            )));
        }

        Ok(RerankResponse::new(parse_vllm_rerank_response(&body)?))
    }
}
