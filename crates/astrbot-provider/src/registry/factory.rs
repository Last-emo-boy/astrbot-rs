use std::sync::Arc;

use astrbot_core::Result;

use crate::config::{
    ChatProviderConfig, EmbeddingProviderConfig, RerankProviderConfig, SpeechToTextProviderConfig,
    TextToSpeechProviderConfig,
};
use crate::{
    ChatProvider, EmbeddingProvider, RerankProvider, SpeechToTextProvider, TextToSpeechProvider,
};

pub(super) type ChatProviderFactory =
    Arc<dyn Fn(&ChatProviderConfig) -> Result<Arc<dyn ChatProvider>> + Send + Sync>;
pub(super) type SpeechToTextProviderFactory =
    Arc<dyn Fn(&SpeechToTextProviderConfig) -> Result<Arc<dyn SpeechToTextProvider>> + Send + Sync>;
pub(super) type TextToSpeechProviderFactory =
    Arc<dyn Fn(&TextToSpeechProviderConfig) -> Result<Arc<dyn TextToSpeechProvider>> + Send + Sync>;
pub(super) type EmbeddingProviderFactory =
    Arc<dyn Fn(&EmbeddingProviderConfig) -> Result<Arc<dyn EmbeddingProvider>> + Send + Sync>;
pub(super) type RerankProviderFactory =
    Arc<dyn Fn(&RerankProviderConfig) -> Result<Arc<dyn RerankProvider>> + Send + Sync>;
