use std::collections::HashMap;

use astrbot_core::Result;

use crate::capability::{ProviderAdapterMetadata, ProviderCapability};

use super::errors::duplicate_provider_type;

#[derive(Clone, Default)]
pub(super) struct ProviderMetadataIndex {
    adapters: HashMap<String, ProviderAdapterMetadata>,
}

impl ProviderMetadataIndex {
    pub(super) fn register(
        &mut self,
        provider_type: impl Into<String>,
        capability: ProviderCapability,
    ) -> Result<()> {
        let provider_type = provider_type.into();
        if self.adapters.contains_key(&provider_type) {
            return Err(duplicate_provider_type(&provider_type));
        }
        self.adapters.insert(
            provider_type.clone(),
            ProviderAdapterMetadata::new(provider_type, capability),
        );
        Ok(())
    }

    pub(super) fn get(&self, provider_type: &str) -> Option<&ProviderAdapterMetadata> {
        self.adapters.get(provider_type)
    }

    pub(super) fn contains(&self, provider_type: &str) -> bool {
        self.adapters.contains_key(provider_type)
    }

    pub(super) fn types_by_capability(&self, capability: ProviderCapability) -> Vec<String> {
        let mut provider_types = self
            .adapters
            .values()
            .filter(|metadata| metadata.capability == capability)
            .map(|metadata| metadata.provider_type.clone())
            .collect::<Vec<_>>();
        provider_types.sort();
        provider_types
    }
}
