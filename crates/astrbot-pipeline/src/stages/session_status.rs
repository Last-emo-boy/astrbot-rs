use astrbot_core::{MessageEvent, Result};
use async_trait::async_trait;

use crate::{PipelineContext, PipelineControl, PipelineStage};

#[derive(Default)]
pub struct SessionStatusCheckStage;

#[async_trait]
impl PipelineStage for SessionStatusCheckStage {
    fn name(&self) -> &str {
        "session_status"
    }

    async fn handle(
        &self,
        event: &mut MessageEvent,
        ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        if ctx.session_status().is_session_enabled(event).await? {
            return Ok(PipelineControl::Continue);
        }

        event.stop();
        Ok(PipelineControl::Stop)
    }
}
