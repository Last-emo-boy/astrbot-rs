use std::sync::Arc;

use astrbot_core::Result;
use async_trait::async_trait;

use super::policy::normalize_ids;

#[derive(Clone)]
pub struct ContentSafetyConfig {
    strategies: Vec<Arc<dyn ContentSafetyStrategy>>,
    pub rejection_message: String,
}

impl Default for ContentSafetyConfig {
    fn default() -> Self {
        Self {
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
        !self.strategies.is_empty()
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

fn default_content_safety_rejection_message() -> String {
    "你的消息或者大模型的响应中包含不适当的内容，已被屏蔽。".to_string()
}
