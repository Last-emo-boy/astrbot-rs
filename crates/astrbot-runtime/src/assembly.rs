use std::sync::Arc;

use astrbot_core::{MessageEvent, MessageEventResult, Result};
use astrbot_platform::{PlatformBuildContext, PlatformConfig, PlatformManager, PlatformRegistry};
use astrbot_plugin::{
    CommandFilter, HandlerMetadata, PluginControl, PluginEventType, PluginHandler, PluginRegistry,
    RegisteredHandler,
};
use astrbot_provider::{
    ChatProviderConfig, EmbeddingProviderConfig, ProviderManager, ProviderManagerConfigSet,
    ProviderRegistry, RerankProviderConfig, SpeechToTextProviderConfig, TextToSpeechProviderConfig,
};
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::RuntimeConfig;
pub(crate) fn build_platform_manager(
    config: &RuntimeConfig,
    event_tx: mpsc::Sender<MessageEvent>,
) -> Result<PlatformManager> {
    let registry = PlatformRegistry::with_builtin_platforms();
    let platform_configs = config
        .platforms
        .clone()
        .into_iter()
        .map(PlatformConfig::from);

    PlatformManager::from_configs(
        &registry,
        platform_configs,
        PlatformBuildContext::new(event_tx),
    )
}

pub(crate) fn build_provider_manager(config: &RuntimeConfig) -> Result<ProviderManager> {
    let registry = ProviderRegistry::with_builtin_providers();

    ProviderManager::from_configs(
        &registry,
        ProviderManagerConfigSet {
            chat_providers: config
                .chat_providers
                .clone()
                .into_iter()
                .map(ChatProviderConfig::from)
                .collect(),
            default_chat_provider_id: Some(config.default_chat_provider_id.clone()),
            speech_to_text_providers: config
                .speech_to_text_providers
                .clone()
                .into_iter()
                .map(SpeechToTextProviderConfig::from)
                .collect(),
            default_speech_to_text_provider_id: config.default_speech_to_text_provider_id.clone(),
            text_to_speech_providers: config
                .text_to_speech_providers
                .clone()
                .into_iter()
                .map(TextToSpeechProviderConfig::from)
                .collect(),
            default_text_to_speech_provider_id: config.default_text_to_speech_provider_id.clone(),
            embedding_providers: config
                .embedding_providers
                .clone()
                .into_iter()
                .map(EmbeddingProviderConfig::from)
                .collect(),
            default_embedding_provider_id: config.default_embedding_provider_id.clone(),
            rerank_providers: config
                .rerank_providers
                .clone()
                .into_iter()
                .map(RerankProviderConfig::from)
                .collect(),
            default_rerank_provider_id: config.default_rerank_provider_id.clone(),
        },
    )
}

pub(crate) fn build_plugin_registry(config: &RuntimeConfig) -> Arc<PluginRegistry> {
    let mut registry = PluginRegistry::new();
    for command in config
        .command_plugins
        .iter()
        .filter(|command| command.enabled)
    {
        registry.register_handler(
            RegisteredHandler::new(
                HandlerMetadata::new(
                    command.plugin_name.clone(),
                    command.handler_name.clone(),
                    PluginEventType::AdapterMessage,
                )
                .with_priority(command.priority),
                Arc::new(StaticReplyHandler {
                    response: command.response.clone(),
                }),
            )
            .with_filter(CommandFilter::new(command.command.clone())),
        );
    }
    Arc::new(registry)
}

struct StaticReplyHandler {
    response: String,
}

#[async_trait]
impl PluginHandler for StaticReplyHandler {
    async fn handle(&self, event: &mut MessageEvent) -> Result<PluginControl> {
        event.set_result(MessageEventResult::general(self.response.clone()));
        Ok(PluginControl::Continue)
    }
}
