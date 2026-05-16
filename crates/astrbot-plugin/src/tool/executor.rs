use std::collections::BTreeMap;

use async_trait::async_trait;

use astrbot_core::{AstrbotError, Result};

use crate::sdk::PluginContext;

use super::capability::ToolCapabilityDecision;
use super::declaration::PluginToolDeclaration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolExecutionRequest {
    pub declaration: PluginToolDeclaration,
    pub context: PluginContext,
    arguments: BTreeMap<String, String>,
}

impl ToolExecutionRequest {
    pub fn new(declaration: PluginToolDeclaration, context: PluginContext) -> Self {
        Self {
            declaration,
            context,
            arguments: BTreeMap::new(),
        }
    }

    pub fn with_argument(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        if !key.trim().is_empty() {
            self.arguments.insert(key, value.into());
        }
        self
    }

    pub fn argument(&self, key: &str) -> Option<&str> {
        self.arguments.get(key).map(String::as_str)
    }

    pub fn arguments(&self) -> &BTreeMap<String, String> {
        &self.arguments
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolExecutionStatus {
    Completed,
    AcceptedBackground,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolExecutionResult {
    pub status: ToolExecutionStatus,
    pub content: Option<String>,
    pub wake_main_agent: bool,
}

impl ToolExecutionResult {
    pub fn completed(content: impl Into<String>) -> Self {
        let content = content.into();
        Self {
            status: ToolExecutionStatus::Completed,
            content: (!content.trim().is_empty()).then_some(content),
            wake_main_agent: false,
        }
    }

    pub fn accepted_background(content: impl Into<String>, wake_main_agent: bool) -> Self {
        let content = content.into();
        Self {
            status: ToolExecutionStatus::AcceptedBackground,
            content: (!content.trim().is_empty()).then_some(content),
            wake_main_agent,
        }
    }

    pub fn rejected(reason: impl Into<String>) -> Self {
        Self {
            status: ToolExecutionStatus::Rejected,
            content: Some(reason.into()),
            wake_main_agent: false,
        }
    }
}

#[async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(&self, request: ToolExecutionRequest) -> Result<ToolExecutionResult>;
}

pub struct SandboxedToolExecutor<E> {
    inner: E,
}

impl<E> SandboxedToolExecutor<E> {
    pub fn new(inner: E) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &E {
        &self.inner
    }
}

#[async_trait]
impl<E> ToolExecutor for SandboxedToolExecutor<E>
where
    E: ToolExecutor,
{
    async fn execute(&self, request: ToolExecutionRequest) -> Result<ToolExecutionResult> {
        let decision =
            ToolCapabilityDecision::check(&request.declaration, request.context.sandbox_profile());
        if let Some(reason) = decision.rejection_message(&request.declaration.name) {
            return Err(AstrbotError::Pipeline(reason));
        }

        self.inner.execute(request).await
    }
}
