use astrbot_core::{MessageEvent, Result};
use async_trait::async_trait;

use crate::PipelineContext;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineControl {
    Continue,
    Stop,
}

#[async_trait]
pub trait PipelineStage: Send + Sync {
    fn name(&self) -> &str;

    fn initialize(&self, _ctx: &PipelineContext) -> Result<()> {
        Ok(())
    }

    async fn handle(
        &self,
        event: &mut MessageEvent,
        ctx: &PipelineContext,
    ) -> Result<PipelineControl>;
}
