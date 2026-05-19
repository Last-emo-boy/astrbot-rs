use std::collections::BTreeMap;
use std::sync::Arc;

use astrbot_computer::{
    BooterSession, ComputerToolInvocation, ComputerUseRuntime, ComputerUseSession,
};
use astrbot_core::{MessageEvent, Result};
use astrbot_tool::ToolCatalog;
use async_trait::async_trait;

use crate::{
    AgentToolCatalogFilter, AgentToolExecutionRequest, AgentToolExecutionResult, AgentToolExecutor,
};

#[async_trait]
pub trait ComputerUseSessionPort: Send + Sync {
    async fn computer_use_session_for_event(
        &self,
        event: &MessageEvent,
    ) -> Result<ComputerUseSession>;

    async fn booter_session_for_event(
        &self,
        _event: &MessageEvent,
    ) -> Result<Option<BooterSession>> {
        Ok(None)
    }
}

#[async_trait]
pub trait ComputerUseExecutionSessionPort: Send + Sync {
    async fn computer_use_session_for_request(
        &self,
        request: &AgentToolExecutionRequest,
    ) -> Result<ComputerUseSession>;

    async fn booter_session_for_request(
        &self,
        _request: &AgentToolExecutionRequest,
    ) -> Result<Option<BooterSession>> {
        Ok(None)
    }
}

pub struct ComputerUseToolCatalogFilter {
    runtime: Arc<ComputerUseRuntime>,
    session_port: Arc<dyn ComputerUseSessionPort>,
}

impl ComputerUseToolCatalogFilter {
    pub fn new(
        runtime: Arc<ComputerUseRuntime>,
        session_port: Arc<dyn ComputerUseSessionPort>,
    ) -> Self {
        Self {
            runtime,
            session_port,
        }
    }
}

#[async_trait]
impl AgentToolCatalogFilter for ComputerUseToolCatalogFilter {
    async fn catalog_for_event(
        &self,
        event: &MessageEvent,
        catalog: &ToolCatalog,
    ) -> Result<ToolCatalog> {
        let session = self
            .session_port
            .computer_use_session_for_event(event)
            .await?;
        let booter = self.session_port.booter_session_for_event(event).await?;
        Ok(self
            .runtime
            .catalog_for_session(catalog, &session, booter.as_ref()))
    }
}

pub struct ComputerUseToolExecutor {
    runtime: Arc<ComputerUseRuntime>,
    session_port: Arc<dyn ComputerUseExecutionSessionPort>,
}

impl ComputerUseToolExecutor {
    pub fn new(
        runtime: Arc<ComputerUseRuntime>,
        session_port: Arc<dyn ComputerUseExecutionSessionPort>,
    ) -> Self {
        Self {
            runtime,
            session_port,
        }
    }
}

#[async_trait]
impl AgentToolExecutor for ComputerUseToolExecutor {
    async fn execute(
        &self,
        request: AgentToolExecutionRequest,
    ) -> Result<AgentToolExecutionResult> {
        let session = self
            .session_port
            .computer_use_session_for_request(&request)
            .await?;
        let booter = self
            .session_port
            .booter_session_for_request(&request)
            .await?;
        let invocation = ComputerToolInvocation::new(
            request.descriptor.name.clone(),
            request.session_id.clone(),
            request.arguments.clone(),
        );
        let execution = self.runtime.execute(&session, booter, invocation).await?;
        Ok(AgentToolExecutionResult::completed(execution.output_text()))
    }
}

#[derive(Clone, Debug)]
pub struct StaticComputerUseSessionPort {
    session: ComputerUseSession,
    booter: Option<BooterSession>,
}

impl StaticComputerUseSessionPort {
    pub fn new(session: ComputerUseSession) -> Self {
        Self {
            session,
            booter: None,
        }
    }

    pub fn with_booter_session(mut self, booter: BooterSession) -> Self {
        self.booter = Some(booter);
        self
    }
}

#[async_trait]
impl ComputerUseSessionPort for StaticComputerUseSessionPort {
    async fn computer_use_session_for_event(
        &self,
        _event: &MessageEvent,
    ) -> Result<ComputerUseSession> {
        Ok(self.session.clone())
    }

    async fn booter_session_for_event(
        &self,
        _event: &MessageEvent,
    ) -> Result<Option<BooterSession>> {
        Ok(self.booter.clone())
    }
}

#[async_trait]
impl ComputerUseExecutionSessionPort for StaticComputerUseSessionPort {
    async fn computer_use_session_for_request(
        &self,
        request: &AgentToolExecutionRequest,
    ) -> Result<ComputerUseSession> {
        let mut session = self.session.clone();
        if session.session_id.trim().is_empty() {
            session.session_id = request.session_id.clone();
        }
        Ok(session)
    }

    async fn booter_session_for_request(
        &self,
        _request: &AgentToolExecutionRequest,
    ) -> Result<Option<BooterSession>> {
        Ok(self.booter.clone())
    }
}

pub fn arguments_from_json(value: &serde_json::Value) -> BTreeMap<String, serde_json::Value> {
    value
        .as_object()
        .map(|object| {
            object
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default()
}
