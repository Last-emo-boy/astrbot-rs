use astrbot_core::{MessageEvent, MessageEventResult, Result};
use async_trait::async_trait;

use crate::{ContentSafetyVerdict, PipelineContext, PipelineControl, PipelineStage};

#[derive(Default)]
pub struct ContentSafetyCheckStage;

#[async_trait]
impl PipelineStage for ContentSafetyCheckStage {
    fn name(&self) -> &str {
        "content_safety"
    }

    async fn handle(
        &self,
        event: &mut MessageEvent,
        ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        let config = ctx.content_safety();
        if !config.is_enabled() {
            return Ok(PipelineControl::Continue);
        }

        let content = event.message.plain_text();
        if content.trim().is_empty() {
            return Ok(PipelineControl::Continue);
        }

        let verdict = config.check_text(&content).await?;
        if !verdict.allowed {
            return block_event(event, config.rejection_message.clone(), verdict);
        }

        Ok(PipelineControl::Continue)
    }
}

fn block_event(
    event: &mut MessageEvent,
    rejection_message: String,
    _verdict: ContentSafetyVerdict,
) -> Result<PipelineControl> {
    if event.is_at_or_wake_command() {
        event.set_result(MessageEventResult::general(rejection_message));
        return Ok(PipelineControl::Continue);
    }

    event.stop();
    Ok(PipelineControl::Stop)
}
