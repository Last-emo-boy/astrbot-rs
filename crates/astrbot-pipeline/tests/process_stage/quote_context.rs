use std::sync::Arc;

use astrbot_core::{EventExecutor, MessageChain, MessageComponent, ProviderContentPart};
use astrbot_pipeline::{
    PipelineContext, PipelineScheduler,
    stages::{ProcessStage, RespondStage},
};
use astrbot_platform::RecordingSink;
use astrbot_plugin::{
    CommandFilter, HandlerMetadata, PluginEventType, PluginRegistry, RegisteredHandler,
};

use crate::support::{CapturingProvider, ProviderRequestHandler, event_with_chain};

#[tokio::test]
async fn process_stage_injects_reply_selected_text_into_provider_request() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(PipelineContext::with_chat_provider(provider.clone()))
        .with_stage(ProcessStage)
        .with_stage(RespondStage);

    scheduler
        .execute(event_with_chain(
            MessageChain::new(vec![
                MessageComponent::reply("message-1", "previous answer"),
                MessageComponent::plain("continue"),
            ]),
            sink.clone(),
        ))
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].prompt, "continue");
    assert_eq!(
        requests[0].extra_user_content_parts,
        vec![ProviderContentPart::text(
            "<Quoted Message>\nprevious answer\n</Quoted Message>"
        )]
    );
}

#[tokio::test]
async fn process_stage_ignores_blank_reply_selected_text() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(PipelineContext::with_chat_provider(provider.clone()))
        .with_stage(ProcessStage)
        .with_stage(RespondStage);

    scheduler
        .execute(event_with_chain(
            MessageChain::new(vec![
                MessageComponent::reply("message-1", " "),
                MessageComponent::plain("continue"),
            ]),
            sink.clone(),
        ))
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert!(requests[0].extra_user_content_parts.is_empty());
}

#[tokio::test]
async fn process_stage_reply_only_message_does_not_trigger_provider_fallback() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(PipelineContext::with_chat_provider(provider.clone()))
        .with_stage(ProcessStage)
        .with_stage(RespondStage);

    scheduler
        .execute(event_with_chain(
            MessageChain::new(vec![MessageComponent::reply(
                "message-1",
                "previous answer",
            )]),
            sink.clone(),
        ))
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert!(sink.messages().await.is_empty());
}

#[tokio::test]
async fn process_stage_adds_quote_context_to_plugin_provider_request() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let mut plugins = PluginRegistry::new();
    plugins.register_handler(
        RegisteredHandler::new(
            HandlerMetadata::new("builtin", "ask", PluginEventType::AdapterMessage),
            Arc::new(ProviderRequestHandler),
        )
        .with_filter(CommandFilter::new("ask")),
    );

    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_plugin_registry(Arc::new(plugins)),
    )
    .with_stage(ProcessStage)
    .with_stage(RespondStage);

    scheduler
        .execute(event_with_chain(
            MessageChain::new(vec![
                MessageComponent::reply("message-1", "previous answer"),
                MessageComponent::plain("/ask ignored fallback"),
            ]),
            sink.clone(),
        ))
        .await
        .expect("scheduler should execute");

    let requests = provider.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].extra_user_content_parts,
        vec![
            ProviderContentPart::text("<Quoted Message>\nprevious answer\n</Quoted Message>"),
            ProviderContentPart::text("extra instruction"),
        ]
    );
}
