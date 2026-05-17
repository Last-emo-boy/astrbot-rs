use astrbot_core::{MessageComponent, MessageEvent, Result};
use async_trait::async_trait;

use crate::{PipelineContext, PipelineControl, PipelineStage, strip_file_scheme};

#[derive(Default)]
pub struct PreprocessStage;

#[async_trait]
impl PipelineStage for PreprocessStage {
    fn name(&self) -> &str {
        "preprocess"
    }

    async fn handle(
        &self,
        event: &mut MessageEvent,
        ctx: &PipelineContext,
    ) -> Result<PipelineControl> {
        maybe_send_pre_ack(event, ctx).await?;
        apply_path_mapping(event, ctx);
        apply_speech_to_text(event, ctx).await?;
        Ok(PipelineControl::Continue)
    }
}

async fn maybe_send_pre_ack(event: &MessageEvent, ctx: &PipelineContext) -> Result<()> {
    let config = ctx.preprocess();
    let pre_ack = &config.pre_ack;
    if !pre_ack.enabled || !event.is_at_or_wake_command() {
        return Ok(());
    }
    if !pre_ack.supports_platform(&event.platform_id, &event.platform_name) {
        return Ok(());
    }
    let Some(reaction) = pre_ack.first_reaction() else {
        return Ok(());
    };

    config.pre_ack_sink().react(event, reaction).await
}

fn apply_path_mapping(event: &mut MessageEvent, ctx: &PipelineContext) {
    let config = ctx.preprocess();
    if !config.path_mapping_enabled {
        return;
    }

    let mapper = config.path_mapper();
    for component in event.message.components_mut() {
        let url = match component {
            MessageComponent::Image { url } | MessageComponent::Record { url } => url,
            _ => continue,
        };
        if let Some(mapped) = mapper.map_path(url) {
            *url = mapped;
        }
    }
}

async fn apply_speech_to_text(event: &mut MessageEvent, ctx: &PipelineContext) -> Result<()> {
    let config = ctx.preprocess();
    if !config.speech_to_text.enabled {
        return Ok(());
    }
    let Some(provider) = config.speech_to_text_provider() else {
        return Ok(());
    };

    for component in event.message.components_mut() {
        let MessageComponent::Record { url } = component else {
            continue;
        };
        if url.trim().is_empty() {
            continue;
        }

        let audio_url = strip_file_scheme(url).to_string();
        let attempts = config.speech_to_text.retry_attempts.max(1);
        let mut transcribed = None;
        for _ in 0..attempts {
            let response = provider
                .transcribe(config.speech_to_text.request_for(audio_url.clone()))
                .await?;
            if !response.text.trim().is_empty() {
                transcribed = Some(response.text);
                break;
            }
        }

        if let Some(text) = transcribed {
            *component = MessageComponent::plain(text);
        }
    }

    Ok(())
}
