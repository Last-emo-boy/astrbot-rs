use astrbot_core::{MessageEvent, Result, ResultContentType};
use async_trait::async_trait;

use crate::{PipelineContext, PipelineControl, PipelineStage};

#[derive(Default)]
pub struct ResultDecorateStage;

#[async_trait]
impl PipelineStage for ResultDecorateStage {
    fn name(&self) -> &str {
        "result_decorate"
    }

    async fn handle(
        &self,
        event: &mut MessageEvent,
        ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        let Some(result) = event.result_mut() else {
            return Ok(PipelineControl::Continue);
        };

        if result.chain.is_empty() || result.content_type == ResultContentType::Streaming {
            return Ok(PipelineControl::Continue);
        }

        let config = ctx.result_decorate();
        let Some(reply_prefix) = config.reply_prefix.as_deref() else {
            return Ok(PipelineControl::Continue);
        };

        if config.only_llm_result && result.content_type != ResultContentType::Llm {
            return Ok(PipelineControl::Continue);
        }

        result.chain.prefix_first_plain(reply_prefix);
        Ok(PipelineControl::Continue)
    }
}
