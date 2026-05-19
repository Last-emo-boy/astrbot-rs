use astrbot_core::{
    MessageChain, MessageComponent, MessageEvent, MessageEventResult, Result, ResultContentType,
};
use astrbot_provider::TextToSpeechRequest;
use astrbot_render::{
    RenderArtifact, RenderArtifactKind, RenderMode, RenderOptions, T2iRenderRequest,
};
use async_trait::async_trait;
use serde_json::json;

use crate::{ContentSafetyConfig, PipelineContext, PipelineControl, PipelineStage};

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
        let Some(result) = event.result() else {
            return Ok(PipelineControl::Continue);
        };

        if result.chain.is_empty() || result.content_type == ResultContentType::Streaming {
            return Ok(PipelineControl::Continue);
        }
        let is_streaming_finish = result.content_type == ResultContentType::StreamingFinish;
        if is_streaming_finish {
            return Ok(PipelineControl::Continue);
        }

        let mut result = event
            .take_result()
            .expect("result was checked before take_result");

        let config = ctx.result_decorate();
        if config.only_llm_result && result.content_type != ResultContentType::Llm {
            event.set_result(result);
            return Ok(PipelineControl::Continue);
        }

        if result.content_type == ResultContentType::Llm
            && !content_safety_allows(&result.chain, ctx.content_safety()).await?
        {
            event.set_result(MessageEventResult::general(
                ctx.content_safety().rejection_message.clone(),
            ));
            return Ok(PipelineControl::Continue);
        }

        if let Some(reply_prefix) = config.reply_prefix.as_deref() {
            result.chain.prefix_first_plain(reply_prefix);
        }

        if config.tts.enabled
            && result.content_type == ResultContentType::Llm
            && let Some(tts_provider) = ctx.text_to_speech_provider()
        {
            result.chain = synthesize_plain_components_to_record(
                result.chain,
                config.tts.provider_id.as_deref(),
                config.tts.dual_output,
                config.tts.use_file_service,
                ctx,
                tts_provider.as_ref(),
            )
            .await?;
        } else if config.t2i.enabled {
            result.chain = render_long_plain_prefix_to_image(result.chain, ctx).await?;
        }

        if config.content_safety_after_transform
            && result.content_type == ResultContentType::Llm
            && !content_safety_allows(&result.chain, ctx.content_safety()).await?
        {
            event.set_result(MessageEventResult::general(
                ctx.content_safety().rejection_message.clone(),
            ));
            return Ok(PipelineControl::Continue);
        }

        event.set_result(result);
        Ok(PipelineControl::Continue)
    }
}

async fn synthesize_plain_components_to_record(
    chain: MessageChain,
    provider_id: Option<&str>,
    dual_output: bool,
    use_file_service: bool,
    ctx: &PipelineContext,
    tts_provider: &dyn astrbot_provider::TextToSpeechProvider,
) -> Result<MessageChain> {
    let mut new_chain = MessageChain::default();
    for component in chain.components() {
        let MessageComponent::Plain { text } = component else {
            new_chain.push(component.clone());
            continue;
        };
        if text.chars().count() <= 1 {
            new_chain.push(component.clone());
            continue;
        }

        let mut request = TextToSpeechRequest::new(text.clone());
        if let Some(provider_id) = provider_id {
            request = request.with_provider_id(provider_id);
        }
        match tts_provider.synthesize(request).await {
            Ok(response) if !response.audio_path.trim().is_empty() => {
                let mut url = response.audio_path;
                if use_file_service {
                    let artifact = RenderArtifact::file(&url, astrbot_render::RenderFormat::Png);
                    if let Some(public_url) =
                        ctx.result_file_service().public_url(&artifact).await?
                    {
                        url = public_url;
                    }
                }
                new_chain.push(MessageComponent::record(url));
                if dual_output {
                    new_chain.push(component.clone());
                }
            }
            _ => new_chain.push(component.clone()),
        }
    }
    Ok(new_chain)
}

async fn render_long_plain_prefix_to_image(
    chain: MessageChain,
    ctx: &PipelineContext,
) -> Result<MessageChain> {
    let config = &ctx.result_decorate().t2i;
    let Some(renderer) = ctx.t2i_renderer() else {
        return Ok(chain);
    };

    let mut plain = String::new();
    for component in chain.components() {
        let MessageComponent::Plain { text } = component else {
            break;
        };
        plain.push_str("\n\n");
        plain.push_str(text);
    }

    if plain.trim().is_empty() || plain.chars().count() <= config.word_threshold {
        return Ok(chain);
    }

    let options = RenderOptions {
        strategy: config.strategy,
        mode: config.mode,
        template_name: config.active_template.clone(),
        ..RenderOptions::default()
    };
    let request = T2iRenderRequest::from_text(plain.clone())
        .with_options(options)
        .with_template_data("plain_text", json!(plain.trim()));
    let Ok(rendered) = renderer.render(request).await else {
        return Ok(chain);
    };

    let mut url = rendered.artifact.value.clone();
    if config.use_file_service && rendered.artifact.kind == RenderArtifactKind::File {
        if let Some(public_url) = ctx
            .result_file_service()
            .public_url(&rendered.artifact)
            .await?
        {
            url = public_url;
        }
    }

    if url.trim().is_empty() {
        return Ok(chain);
    }

    let image_url = match rendered.artifact.kind {
        RenderArtifactKind::Url => url,
        RenderArtifactKind::File if matches!(config.mode, RenderMode::Url) => url,
        RenderArtifactKind::File => url,
    };
    Ok(MessageChain::new(vec![MessageComponent::image(image_url)]))
}

async fn content_safety_allows(
    chain: &MessageChain,
    content_safety: &ContentSafetyConfig,
) -> Result<bool> {
    if !content_safety.is_enabled() {
        return Ok(true);
    }

    let text = chain.plain_text();
    if text.trim().is_empty() {
        return Ok(true);
    }

    Ok(content_safety.check_text(&text).await?.allowed)
}
