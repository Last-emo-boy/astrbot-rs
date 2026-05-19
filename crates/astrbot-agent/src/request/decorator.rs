use std::sync::Arc;

use astrbot_core::{MessageEvent, ProviderRequest, Result};
use async_trait::async_trait;

#[async_trait]
pub trait ProviderRequestDecorator: Send + Sync {
    async fn decorate(&self, event: &MessageEvent, request: &mut ProviderRequest) -> Result<()>;
}

pub struct NoopProviderRequestDecorator;

#[async_trait]
impl ProviderRequestDecorator for NoopProviderRequestDecorator {
    async fn decorate(&self, _event: &MessageEvent, _request: &mut ProviderRequest) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
pub trait ProviderRequestHook: Send + Sync {
    async fn before_request(
        &self,
        event: &MessageEvent,
        request: &mut ProviderRequest,
        explicit: bool,
    ) -> Result<bool>;
}

pub struct NoopProviderRequestHook;

#[async_trait]
impl ProviderRequestHook for NoopProviderRequestHook {
    async fn before_request(
        &self,
        _event: &MessageEvent,
        _request: &mut ProviderRequest,
        _explicit: bool,
    ) -> Result<bool> {
        Ok(false)
    }
}

#[derive(Default)]
pub struct CompositeProviderRequestDecorator {
    decorators: Vec<Arc<dyn ProviderRequestDecorator>>,
}

impl CompositeProviderRequestDecorator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_decorator(mut self, decorator: Arc<dyn ProviderRequestDecorator>) -> Self {
        self.decorators.push(decorator);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.decorators.is_empty()
    }
}

#[async_trait]
impl ProviderRequestDecorator for CompositeProviderRequestDecorator {
    async fn decorate(&self, event: &MessageEvent, request: &mut ProviderRequest) -> Result<()> {
        for decorator in &self.decorators {
            decorator.decorate(event, request).await?;
        }
        Ok(())
    }
}
