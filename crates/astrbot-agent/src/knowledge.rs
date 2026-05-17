use std::sync::Arc;

use astrbot_core::{MessageEvent, ProviderRequest, Result};
use async_trait::async_trait;

use crate::ProviderRequestDecorator;

#[async_trait]
pub trait AgentKnowledgeContextPort: Send + Sync {
    async fn formatted_knowledge_context(&self, event: &MessageEvent) -> Result<Option<String>>;
}

pub struct KnowledgeContextRequestDecorator {
    context: Arc<dyn AgentKnowledgeContextPort>,
}

impl KnowledgeContextRequestDecorator {
    pub fn new(context: Arc<dyn AgentKnowledgeContextPort>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl ProviderRequestDecorator for KnowledgeContextRequestDecorator {
    async fn decorate(&self, event: &MessageEvent, request: &mut ProviderRequest) -> Result<()> {
        let Some(context) = self.context.formatted_knowledge_context(event).await? else {
            return Ok(());
        };
        let context = context.trim();
        if context.is_empty() {
            return Ok(());
        }

        request.system_prompt = Some(match request.system_prompt.take() {
            Some(existing) if !existing.trim().is_empty() => format!("{existing}\n\n{context}"),
            _ => context.to_string(),
        });
        Ok(())
    }
}
