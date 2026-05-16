use astrbot_core::{MessageEvent, Result};
use async_trait::async_trait;

use crate::{PipelineContext, PipelineControl, PipelineStage};

#[derive(Default)]
pub struct RespondStage;

#[async_trait]
impl PipelineStage for RespondStage {
    fn name(&self) -> &str {
        "respond"
    }

    async fn handle(
        &self,
        event: &mut MessageEvent,
        _ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        if event.is_streaming_finished() {
            return Ok(PipelineControl::Continue);
        }

        let Some(result) = event.take_result() else {
            return Ok(PipelineControl::Continue);
        };

        let stopped = result.is_stopped();
        if result.is_streaming_finish() {
            event.mark_streaming_finished();
            return Ok(control_for(stopped));
        }

        if result.is_streaming() {
            if let Some(stream) = result.stream
                && !stream.is_empty()
            {
                event.send_streaming(stream).await?;
            }
            return Ok(control_for(stopped));
        }

        if let Some(chain) = result.chain.into_sendable() {
            event.send(chain).await?;
        }

        Ok(control_for(stopped))
    }
}

fn control_for(stopped: bool) -> PipelineControl {
    if stopped {
        PipelineControl::Stop
    } else {
        PipelineControl::Continue
    }
}
