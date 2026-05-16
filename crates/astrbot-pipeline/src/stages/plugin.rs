use astrbot_core::{MessageEvent, Result};
use astrbot_plugin::{PluginControl, PluginEventType};
use async_trait::async_trait;

use crate::{PipelineContext, PipelineControl, PipelineStage};

#[derive(Default)]
pub struct PluginStage;

#[async_trait]
impl PipelineStage for PluginStage {
    fn name(&self) -> &str {
        "plugin"
    }

    async fn handle(
        &self,
        event: &mut MessageEvent,
        ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        run_plugin_handlers(event, ctx).await
    }
}

pub(super) async fn run_plugin_handlers(
    event: &mut MessageEvent,
    ctx: &PipelineContext,
) -> Result<PipelineControl> {
    if event.result().is_some() || event.is_stopped() {
        return Ok(PipelineControl::Continue);
    }

    let Some(registry) = ctx.plugin_registry() else {
        return Ok(PipelineControl::Continue);
    };

    match registry
        .handle_event(PluginEventType::AdapterMessage, event)
        .await?
    {
        PluginControl::Continue => Ok(PipelineControl::Continue),
        PluginControl::Stop => Ok(PipelineControl::Stop),
    }
}
