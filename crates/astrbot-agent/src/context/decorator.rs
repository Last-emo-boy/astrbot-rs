use std::sync::Arc;

use astrbot_core::{MessageEvent, ProviderRequest, Result};
use async_trait::async_trait;

use crate::ProviderRequestDecorator;

use super::ContextWindowManager;

pub struct ContextWindowRequestDecorator {
    manager: Arc<ContextWindowManager>,
}

impl ContextWindowRequestDecorator {
    pub fn new(manager: Arc<ContextWindowManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl ProviderRequestDecorator for ContextWindowRequestDecorator {
    async fn decorate(&self, _event: &MessageEvent, request: &mut ProviderRequest) -> Result<()> {
        if request.contexts.is_empty() {
            return Ok(());
        }

        let contexts = std::mem::take(&mut request.contexts);
        request.contexts = self.manager.prepare_messages(contexts).await?;
        Ok(())
    }
}
