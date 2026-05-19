use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCapability {
    ChatCompletion,
    SpeechToText,
    TextToSpeech,
}

impl ProviderCapability {
    pub fn preference_key(self) -> &'static str {
        match self {
            Self::ChatCompletion => "provider_perf_chat_completion",
            Self::SpeechToText => "provider_perf_speech_to_text",
            Self::TextToSpeech => "provider_perf_text_to_speech",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionRuleValue {
    Service(SessionServiceRule),
    Plugin(SessionPluginRule),
    KnowledgeBase(SessionKnowledgeBaseRule),
    Provider(SessionProviderPreference),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRule {
    pub umo: String,
    pub key: SessionRuleKey,
    pub value: SessionRuleValue,
}

impl SessionRule {
    pub fn new(
        umo: impl Into<String>,
        key: SessionRuleKey,
        value: SessionRuleValue,
    ) -> Option<Self> {
        let umo = normalize(umo);
        (!umo.is_empty()).then_some(Self { umo, key, value })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "capability", rename_all = "snake_case")]
pub enum SessionRuleKey {
    Service,
    Plugin,
    KnowledgeBase,
    Provider(ProviderCapability),
}

impl SessionRuleKey {
    pub fn storage_key(&self) -> &'static str {
        match self {
            Self::Service => "session_service_config",
            Self::Plugin => "session_plugin_config",
            Self::KnowledgeBase => "kb_config",
            Self::Provider(capability) => capability.preference_key(),
        }
    }

    pub fn available_keys() -> Vec<Self> {
        vec![
            Self::Service,
            Self::Plugin,
            Self::KnowledgeBase,
            Self::Provider(ProviderCapability::ChatCompletion),
            Self::Provider(ProviderCapability::SpeechToText),
            Self::Provider(ProviderCapability::TextToSpeech),
        ]
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionServiceRule {
    pub session_enabled: Option<bool>,
    pub llm_enabled: Option<bool>,
    pub tts_enabled: Option<bool>,
    pub custom_name: Option<String>,
    pub persona_id: Option<String>,
}

impl SessionServiceRule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_session_enabled(mut self, enabled: bool) -> Self {
        self.session_enabled = Some(enabled);
        self
    }

    pub fn with_llm_enabled(mut self, enabled: bool) -> Self {
        self.llm_enabled = Some(enabled);
        self
    }

    pub fn with_tts_enabled(mut self, enabled: bool) -> Self {
        self.tts_enabled = Some(enabled);
        self
    }

    pub fn with_custom_name(mut self, custom_name: impl Into<String>) -> Self {
        self.custom_name = non_empty(custom_name);
        self
    }

    pub fn with_persona_id(mut self, persona_id: impl Into<String>) -> Self {
        self.persona_id = non_empty(persona_id);
        self
    }

    pub fn merge_patch(&mut self, patch: SessionServiceRulePatch) {
        if let Some(value) = patch.session_enabled {
            self.session_enabled = Some(value);
        }
        if let Some(value) = patch.llm_enabled {
            self.llm_enabled = Some(value);
        }
        if let Some(value) = patch.tts_enabled {
            self.tts_enabled = Some(value);
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionServiceRulePatch {
    pub session_enabled: Option<bool>,
    pub llm_enabled: Option<bool>,
    pub tts_enabled: Option<bool>,
}

impl SessionServiceRulePatch {
    pub fn has_changes(&self) -> bool {
        self.session_enabled.is_some() || self.llm_enabled.is_some() || self.tts_enabled.is_some()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionPluginRule {
    pub enabled_plugins: Vec<String>,
    pub disabled_plugins: Vec<String>,
}

impl SessionPluginRule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_enabled_plugin(mut self, plugin: impl Into<String>) -> Self {
        push_unique(&mut self.enabled_plugins, plugin);
        self
    }

    pub fn with_disabled_plugin(mut self, plugin: impl Into<String>) -> Self {
        push_unique(&mut self.disabled_plugins, plugin);
        self
    }

    pub fn is_plugin_enabled(&self, plugin: &str) -> bool {
        let plugin = plugin.trim();
        !self.disabled_plugins.iter().any(|known| known == plugin)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionKnowledgeBaseRule {
    pub kb_ids: Vec<String>,
    pub top_k: Option<u32>,
    pub enable_rerank: Option<bool>,
}

impl SessionKnowledgeBaseRule {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_kb_id(mut self, kb_id: impl Into<String>) -> Self {
        push_unique(&mut self.kb_ids, kb_id);
        self
    }

    pub fn with_top_k(mut self, top_k: u32) -> Self {
        self.top_k = Some(top_k);
        self
    }

    pub fn with_enable_rerank(mut self, enable_rerank: bool) -> Self {
        self.enable_rerank = Some(enable_rerank);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionProviderPreference {
    pub capability: ProviderCapability,
    pub provider_id: String,
}

impl SessionProviderPreference {
    pub fn new(capability: ProviderCapability, provider_id: impl Into<String>) -> Option<Self> {
        let provider_id = normalize(provider_id);
        (!provider_id.is_empty()).then_some(Self {
            capability,
            provider_id,
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRuleSet {
    pub umo: String,
    pub service: Option<SessionServiceRule>,
    pub plugin: Option<SessionPluginRule>,
    pub knowledge_base: Option<SessionKnowledgeBaseRule>,
    pub provider_preferences: Vec<SessionProviderPreference>,
}

impl SessionRuleSet {
    pub fn new(umo: impl Into<String>) -> Option<Self> {
        let umo = normalize(umo);
        (!umo.is_empty()).then_some(Self {
            umo,
            ..Self::default()
        })
    }

    pub fn with_rule(mut self, rule: SessionRule) -> Self {
        match rule.value {
            SessionRuleValue::Service(service) => self.service = Some(service),
            SessionRuleValue::Plugin(plugin) => self.plugin = Some(plugin),
            SessionRuleValue::KnowledgeBase(knowledge_base) => {
                self.knowledge_base = Some(knowledge_base);
            }
            SessionRuleValue::Provider(provider) => {
                self.provider_preferences
                    .retain(|current| current.capability != provider.capability);
                self.provider_preferences.push(provider);
                self.provider_preferences
                    .sort_by_key(|current| current.capability);
            }
        }
        self
    }

    pub fn provider_for(&self, capability: ProviderCapability) -> Option<&str> {
        self.provider_preferences
            .iter()
            .find(|preference| preference.capability == capability)
            .map(|preference| preference.provider_id.as_str())
    }

    pub fn has_any_rule(&self) -> bool {
        self.service.is_some()
            || self.plugin.is_some()
            || self.knowledge_base.is_some()
            || !self.provider_preferences.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionBatchScope {
    Explicit(Vec<String>),
    All,
    Group,
    Private,
    CustomGroup(String),
}

impl SessionBatchScope {
    pub fn explicit<I, S>(umos: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::Explicit(normalized_unique(umos))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBatchServiceUpdate {
    pub scope: SessionBatchScope,
    pub patch: SessionServiceRulePatch,
}

impl SessionBatchServiceUpdate {
    pub fn new(scope: SessionBatchScope, patch: SessionServiceRulePatch) -> Option<Self> {
        patch.has_changes().then_some(Self { scope, patch })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBatchProviderUpdate {
    pub scope: SessionBatchScope,
    pub preference: SessionProviderPreference,
}

pub fn filter_umos_by_scope<I, S>(scope: &SessionBatchScope, all_umos: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    match scope {
        SessionBatchScope::Explicit(umos) => normalized_unique(umos.clone()),
        SessionBatchScope::All => normalized_unique(all_umos),
        SessionBatchScope::Group => normalized_unique(all_umos)
            .into_iter()
            .filter(|umo| is_group_umo(umo))
            .collect(),
        SessionBatchScope::Private => normalized_unique(all_umos)
            .into_iter()
            .filter(|umo| is_private_umo(umo))
            .collect(),
        SessionBatchScope::CustomGroup(_) => Vec::new(),
    }
}

pub fn is_group_umo(umo: &str) -> bool {
    let normalized = umo.to_ascii_lowercase();
    normalized.contains(":group:") || normalized.contains(":groupmessage:")
}

pub fn is_private_umo(umo: &str) -> bool {
    let normalized = umo.to_ascii_lowercase();
    normalized.contains(":private:")
        || normalized.contains(":friend:")
        || normalized.contains(":friendmessage:")
        || normalized.contains(":direct:")
}

fn normalized_unique<I, S>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut set = BTreeSet::new();
    for value in values {
        let value = normalize(value);
        if !value.is_empty() {
            set.insert(value);
        }
    }
    set.into_iter().collect()
}

fn push_unique(values: &mut Vec<String>, value: impl Into<String>) {
    let value = normalize(value);
    if !value.is_empty() && !values.iter().any(|known| known == &value) {
        values.push(value);
    }
}

fn non_empty(value: impl Into<String>) -> Option<String> {
    let value = normalize(value);
    (!value.is_empty()).then_some(value)
}

fn normalize(value: impl Into<String>) -> String {
    value.into().trim().to_string()
}
