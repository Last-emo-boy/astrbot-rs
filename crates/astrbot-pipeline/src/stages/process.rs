use astrbot_core::{MessageEvent, Result};
use async_trait::async_trait;

use crate::{PipelineContext, PipelineControl, PipelineStage};

use super::plugin::run_plugin_handlers;
use super::provider::run_provider_fallback;

#[derive(Default)]
pub struct ProcessStage;

#[async_trait]
impl PipelineStage for ProcessStage {
    fn name(&self) -> &str {
        "process"
    }

    async fn handle(
        &self,
        event: &mut MessageEvent,
        ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        let plugin_control = run_plugin_handlers(event, ctx).await?;
        if plugin_control == PipelineControl::Stop || event.is_stopped() {
            return Ok(PipelineControl::Stop);
        }

        run_provider_fallback(event, ctx).await
    }
}
