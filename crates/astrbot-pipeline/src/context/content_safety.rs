use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde_json::Value;

use super::policy::normalize_ids;

#[derive(Clone)]
pub struct ContentSafetyConfig {
    enabled: bool,
    strategies: Vec<Arc<dyn ContentSafetyStrategy>>,
    pub rejection_message: String,
}

impl Default for ContentSafetyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            strategies: Vec::new(),
            rejection_message: default_content_safety_rejection_message(),
        }
    }
}

impl ContentSafetyConfig {
    pub fn with_strategy(mut self, strategy: Arc<dyn ContentSafetyStrategy>) -> Self {
        self.strategies.push(strategy);
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_rejection_message(mut self, rejection_message: impl Into<String>) -> Self {
        let rejection_message = rejection_message.into();
        if !rejection_message.trim().is_empty() {
            self.rejection_message = rejection_message;
        }
        self
    }

    pub fn strategies(&self) -> &[Arc<dyn ContentSafetyStrategy>] {
        &self.strategies
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled && !self.strategies.is_empty()
    }

    pub async fn check_text(&self, content: &str) -> Result<ContentSafetyVerdict> {
        if !self.is_enabled() || content.trim().is_empty() {
            return Ok(ContentSafetyVerdict::allowed());
        }

        for strategy in self.strategies() {
            let verdict = strategy.check(content).await?;
            if !verdict.allowed {
                return Ok(verdict);
            }
        }

        Ok(ContentSafetyVerdict::allowed())
    }
}

#[async_trait]
pub trait ContentSafetyStrategy: Send + Sync {
    async fn check(&self, content: &str) -> Result<ContentSafetyVerdict>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentSafetyVerdict {
    pub allowed: bool,
    pub reason: String,
}

impl ContentSafetyVerdict {
    pub fn allowed() -> Self {
        Self {
            allowed: true,
            reason: String::new(),
        }
    }

    pub fn blocked(reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            reason: reason.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeywordContentSafetyStrategy {
    keywords: Vec<String>,
}

impl KeywordContentSafetyStrategy {
    pub fn new<I, S>(keywords: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            keywords: normalize_ids(keywords),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.keywords.is_empty()
    }
}

#[async_trait]
impl ContentSafetyStrategy for KeywordContentSafetyStrategy {
    async fn check(&self, content: &str) -> Result<ContentSafetyVerdict> {
        for keyword in &self.keywords {
            if content.contains(keyword) {
                return Ok(ContentSafetyVerdict::blocked(
                    "content safety check failed: matched keyword",
                ));
            }
        }
        Ok(ContentSafetyVerdict::allowed())
    }
}

#[derive(Clone, Debug)]
pub struct BaiduAipContentSafetyStrategy {
    app_id: String,
    api_key: String,
    secret_key: String,
    token_url: String,
    censor_url: String,
    client: reqwest::Client,
}

impl BaiduAipContentSafetyStrategy {
    pub fn new(
        app_id: impl Into<String>,
        api_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> Self {
        Self {
            app_id: app_id.into(),
            api_key: api_key.into(),
            secret_key: secret_key.into(),
            token_url: "https://aip.baidubce.com/oauth/2.0/token".to_string(),
            censor_url: "https://aip.baidubce.com/rest/2.0/solution/v1/text_censor/v2/user_defined"
                .to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn with_endpoints(
        mut self,
        token_url: impl Into<String>,
        censor_url: impl Into<String>,
    ) -> Self {
        self.token_url = token_url.into();
        self.censor_url = censor_url.into();
        self
    }

    async fn access_token(&self) -> Result<String> {
        if self.app_id.trim().is_empty()
            || self.api_key.trim().is_empty()
            || self.secret_key.trim().is_empty()
        {
            return Err(AstrbotError::Pipeline(
                "Baidu AIP content safety credentials are incomplete".to_string(),
            ));
        }

        let response = self
            .client
            .post(&self.token_url)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", self.api_key.as_str()),
                ("client_secret", self.secret_key.as_str()),
            ])
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Pipeline(format!("Baidu AIP token request failed: {err}"))
            })?;
        let body = response_body_or_error(response, "Baidu AIP token").await?;
        let value: Value = serde_json::from_str(&body).map_err(|err| {
            AstrbotError::Pipeline(format!("Baidu AIP token response is not JSON: {err}"))
        })?;
        value
            .get("access_token")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(str::to_string)
            .ok_or_else(|| {
                AstrbotError::Pipeline("Baidu AIP token response missing access_token".to_string())
            })
    }
}

#[async_trait]
impl ContentSafetyStrategy for BaiduAipContentSafetyStrategy {
    async fn check(&self, content: &str) -> Result<ContentSafetyVerdict> {
        let token = self.access_token().await?;
        let response = self
            .client
            .post(&self.censor_url)
            .query(&[("access_token", token.as_str())])
            .form(&[("text", content)])
            .send()
            .await
            .map_err(|err| {
                AstrbotError::Pipeline(format!("Baidu AIP censor request failed: {err}"))
            })?;
        let body = response_body_or_error(response, "Baidu AIP censor").await?;
        parse_baidu_aip_verdict(&body)
    }
}

fn parse_baidu_aip_verdict(body: &str) -> Result<ContentSafetyVerdict> {
    let value: Value = serde_json::from_str(body).map_err(|err| {
        AstrbotError::Pipeline(format!("Baidu AIP censor response is not JSON: {err}"))
    })?;
    if value.get("conclusionType").and_then(Value::as_i64) == Some(1) {
        return Ok(ContentSafetyVerdict::allowed());
    }

    let conclusion = value
        .get("conclusion")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let messages = value
        .get("data")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("msg").and_then(Value::as_str))
        .filter(|message| !message.trim().is_empty())
        .collect::<Vec<_>>();

    let reason = if messages.is_empty() && conclusion.trim().is_empty() {
        "Baidu AIP content safety check failed".to_string()
    } else {
        format!(
            "百度审核服务发现 {} 处违规：{}\n判断结果：{}",
            messages.len(),
            messages.join("；"),
            conclusion
        )
    };

    Ok(ContentSafetyVerdict::blocked(reason))
}

async fn response_body_or_error(response: reqwest::Response, label: &str) -> Result<String> {
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|err| AstrbotError::Pipeline(format!("failed to read {label} response: {err}")))?;
    if !status.is_success() {
        return Err(AstrbotError::Pipeline(format!(
            "{label} returned {status}: {body}"
        )));
    }
    Ok(body)
}

fn default_content_safety_rejection_message() -> String {
    "你的消息或者大模型的响应中包含不适当的内容，已被屏蔽。".to_string()
}
