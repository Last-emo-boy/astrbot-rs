use std::sync::{Arc, Mutex};

use astrbot_core::{AstrbotError, MessageChain, MessageEvent, MessageSession, MessageSink, Result};
use async_trait::async_trait;

use crate::{PipelineContext, PipelineControl, PipelineStage};

pub(super) struct NamedStage(pub(super) &'static str);

#[async_trait]
impl PipelineStage for NamedStage {
    fn name(&self) -> &str {
        self.0
    }

    async fn handle(
        &self,
        _event: &mut MessageEvent,
        _ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        Ok(PipelineControl::Continue)
    }
}

pub(super) struct InitRecordingStage {
    pub(super) name: &'static str,
    pub(super) calls: Arc<Mutex<Vec<&'static str>>>,
}

#[async_trait]
impl PipelineStage for InitRecordingStage {
    fn name(&self) -> &str {
        self.name
    }

    fn initialize(&self, _ctx: &PipelineContext) -> Result<()> {
        self.calls
            .lock()
            .expect("init calls should lock")
            .push(self.name);
        Ok(())
    }

    async fn handle(
        &self,
        _event: &mut MessageEvent,
        _ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        Ok(PipelineControl::Continue)
    }
}

pub(super) struct FailingInitStage;

#[async_trait]
impl PipelineStage for FailingInitStage {
    fn name(&self) -> &str {
        "failing"
    }

    fn initialize(&self, _ctx: &PipelineContext) -> Result<()> {
        Err(AstrbotError::Pipeline("init failed".to_string()))
    }

    async fn handle(
        &self,
        _event: &mut MessageEvent,
        _ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        Ok(PipelineControl::Continue)
    }
}

pub(super) struct HandleRecordingStage {
    pub(super) name: &'static str,
    pub(super) calls: Arc<Mutex<Vec<&'static str>>>,
    pub(super) control: PipelineControl,
}

#[async_trait]
impl PipelineStage for HandleRecordingStage {
    fn name(&self) -> &str {
        self.name
    }

    async fn handle(
        &self,
        _event: &mut MessageEvent,
        _ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        self.calls
            .lock()
            .expect("handle calls should lock")
            .push(self.name);
        Ok(self.control)
    }
}

pub(super) struct NoopSink;

#[async_trait]
impl MessageSink for NoopSink {
    async fn send(&self, _session: &MessageSession, _chain: MessageChain) -> Result<()> {
        Ok(())
    }
}
