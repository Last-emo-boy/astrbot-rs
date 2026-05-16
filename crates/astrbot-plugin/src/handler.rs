use std::sync::Arc;

use astrbot_core::{MessageEvent, Result};
use async_trait::async_trait;

use crate::event::{PluginControl, PluginEventType};
use crate::filter::EventFilter;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandlerMetadata {
    pub plugin_name: String,
    pub handler_name: String,
    pub event_type: PluginEventType,
    pub priority: i32,
    pub enabled: bool,
    pub description: Option<String>,
}

impl HandlerMetadata {
    pub fn new(
        plugin_name: impl Into<String>,
        handler_name: impl Into<String>,
        event_type: PluginEventType,
    ) -> Self {
        Self {
            plugin_name: plugin_name.into(),
            handler_name: handler_name.into(),
            event_type,
            priority: 0,
            enabled: true,
            description: None,
        }
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        let description = description.into();
        self.description = (!description.trim().is_empty()).then_some(description);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

#[async_trait]
pub trait PluginHandler: Send + Sync {
    async fn handle(&self, event: &mut MessageEvent) -> Result<PluginControl>;

    async fn terminate(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct RegisteredHandler {
    metadata: HandlerMetadata,
    filters: Vec<Arc<dyn EventFilter>>,
    handler: Arc<dyn PluginHandler>,
}

impl RegisteredHandler {
    pub fn new(metadata: HandlerMetadata, handler: Arc<dyn PluginHandler>) -> Self {
        Self {
            metadata,
            filters: Vec::new(),
            handler,
        }
    }

    pub fn with_filter(mut self, filter: impl EventFilter + 'static) -> Self {
        self.filters.push(Arc::new(filter));
        self
    }

    pub fn metadata(&self) -> &HandlerMetadata {
        &self.metadata
    }

    pub fn filters(&self) -> &[Arc<dyn EventFilter>] {
        &self.filters
    }

    pub(crate) fn matches(&self, event_type: PluginEventType, event: &MessageEvent) -> bool {
        self.metadata.enabled
            && self.metadata.event_type == event_type
            && (self.filters.is_empty() || self.filters.iter().all(|filter| filter.matches(event)))
    }

    pub(crate) async fn handle(&self, event: &mut MessageEvent) -> Result<PluginControl> {
        self.handler.handle(event).await
    }

    pub(crate) async fn terminate(&self) -> Result<()> {
        self.handler.terminate().await
    }
}
