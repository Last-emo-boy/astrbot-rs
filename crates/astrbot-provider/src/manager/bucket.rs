use std::collections::HashMap;
use std::sync::Arc;

use astrbot_core::Result;

use crate::config::{
    ChatProviderConfig, EmbeddingProviderConfig, RerankProviderConfig, SpeechToTextProviderConfig,
    TextToSpeechProviderConfig,
};

pub(super) struct ProviderBucket<P: ?Sized> {
    providers: HashMap<String, Arc<P>>,
}

impl<P: ?Sized> Clone for ProviderBucket<P> {
    fn clone(&self) -> Self {
        Self {
            providers: self.providers.clone(),
        }
    }
}

impl<P: ?Sized> ProviderBucket<P> {
    pub(super) fn from_configs<C, F>(
        configs: impl IntoIterator<Item = C>,
        configured_default_id: Option<String>,
        mut build_provider: F,
    ) -> Result<ProviderBucketInit<P>>
    where
        C: ProviderConfigEntry,
        F: FnMut(&C) -> Result<Arc<P>>,
    {
        let mut bucket = Self::default();
        let mut first_provider_id = None;

        for config in configs {
            if !config.enabled() {
                continue;
            }

            let provider = build_provider(&config)?;
            let provider_id = config.id().to_string();
            first_provider_id.get_or_insert_with(|| provider_id.clone());
            bucket.insert(provider_id, provider);
        }

        let default_provider_id = configured_default_id
            .filter(|provider_id| bucket.contains_key(provider_id))
            .or(first_provider_id);

        Ok(ProviderBucketInit {
            bucket,
            default_provider_id,
        })
    }

    pub(super) fn get(&self, provider_id: &str) -> Option<Arc<P>> {
        self.providers.get(provider_id).cloned()
    }

    pub(super) fn selected(&self, provider_id: Option<&str>) -> Option<Arc<P>> {
        provider_id.and_then(|provider_id| self.get(provider_id))
    }

    pub(super) fn contains_key(&self, provider_id: &str) -> bool {
        self.providers.contains_key(provider_id)
    }

    pub(super) fn len(&self) -> usize {
        self.providers.len()
    }

    pub(super) fn insert(&mut self, provider_id: String, provider: Arc<P>) -> Option<Arc<P>> {
        self.providers.insert(provider_id, provider)
    }
}

impl<P: ?Sized> Default for ProviderBucket<P> {
    fn default() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }
}

impl<'a, P: ?Sized> IntoIterator for &'a ProviderBucket<P> {
    type Item = (&'a String, &'a Arc<P>);
    type IntoIter = std::collections::hash_map::Iter<'a, String, Arc<P>>;

    fn into_iter(self) -> Self::IntoIter {
        self.providers.iter()
    }
}

pub(super) struct ProviderBucketInit<P: ?Sized> {
    pub(super) bucket: ProviderBucket<P>,
    pub(super) default_provider_id: Option<String>,
}

pub(super) trait ProviderConfigEntry {
    fn id(&self) -> &str;

    fn enabled(&self) -> bool;
}

macro_rules! impl_provider_config_entry {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl ProviderConfigEntry for $ty {
                fn id(&self) -> &str {
                    &self.id
                }

                fn enabled(&self) -> bool {
                    self.enabled
                }
            }
        )+
    };
}

impl_provider_config_entry!(
    ChatProviderConfig,
    SpeechToTextProviderConfig,
    TextToSpeechProviderConfig,
    EmbeddingProviderConfig,
    RerankProviderConfig,
);
