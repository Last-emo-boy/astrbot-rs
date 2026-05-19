use std::sync::Arc;

use astrbot_core::{MessageEvent, Result};
use astrbot_tool::{ToolCatalog, WebSearchSessionConfig, WebSearchToolSelection};
use async_trait::async_trait;

use crate::AgentToolCatalogFilter;

#[async_trait]
pub trait WebSearchSessionConfigPort: Send + Sync {
    async fn web_search_config_for_event(
        &self,
        event: &MessageEvent,
    ) -> Result<WebSearchSessionConfig>;
}

pub struct WebSearchToolCatalogFilter {
    config: Arc<dyn WebSearchSessionConfigPort>,
}

impl WebSearchToolCatalogFilter {
    pub fn new(config: Arc<dyn WebSearchSessionConfigPort>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl AgentToolCatalogFilter for WebSearchToolCatalogFilter {
    async fn catalog_for_event(
        &self,
        event: &MessageEvent,
        catalog: &ToolCatalog,
    ) -> Result<ToolCatalog> {
        let config = self.config.web_search_config_for_event(event).await?;
        Ok(WebSearchToolSelection::from_config(&config).apply_to_catalog(catalog))
    }
}
