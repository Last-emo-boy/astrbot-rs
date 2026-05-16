use std::collections::HashMap;

use astrbot_core::{AstrbotError, Result};

use super::lifecycle::PluginLifecycleState;
use super::metadata::PluginMetadata;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginRecord {
    pub metadata: PluginMetadata,
    pub state: PluginLifecycleState,
}

impl PluginRecord {
    pub fn new(metadata: PluginMetadata, state: PluginLifecycleState) -> Self {
        Self { metadata, state }
    }

    pub fn plugin_id(&self) -> &str {
        self.metadata.plugin_id()
    }

    pub fn is_active(&self) -> bool {
        self.state == PluginLifecycleState::Active
    }
}

pub trait PluginStateStore {
    fn upsert(&mut self, record: PluginRecord);
    fn get(&self, plugin_id: &str) -> Option<&PluginRecord>;
    fn set_state(&mut self, plugin_id: &str, state: PluginLifecycleState) -> Result<()>;
    fn remove(&mut self, plugin_id: &str) -> Option<PluginRecord>;
    fn records(&self) -> Vec<PluginRecord>;
}

#[derive(Default)]
pub struct InMemoryPluginStore {
    records: HashMap<String, PluginRecord>,
}

impl InMemoryPluginStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PluginStateStore for InMemoryPluginStore {
    fn upsert(&mut self, record: PluginRecord) {
        self.records.insert(record.plugin_id().to_string(), record);
    }

    fn get(&self, plugin_id: &str) -> Option<&PluginRecord> {
        self.records.get(plugin_id)
    }

    fn set_state(&mut self, plugin_id: &str, state: PluginLifecycleState) -> Result<()> {
        let record = self.records.get_mut(plugin_id).ok_or_else(|| {
            AstrbotError::Pipeline(format!("plugin {plugin_id} is not discovered"))
        })?;
        record.state = state;
        Ok(())
    }

    fn remove(&mut self, plugin_id: &str) -> Option<PluginRecord> {
        self.records.remove(plugin_id)
    }

    fn records(&self) -> Vec<PluginRecord> {
        self.records.values().cloned().collect()
    }
}
