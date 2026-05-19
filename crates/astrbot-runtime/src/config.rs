use std::path::Path;

use astrbot_core::Result;
use serde::{Deserialize, Serialize};

pub(crate) mod defaults;
mod env;
pub(crate) mod migration;
mod schema;
mod secrets;
mod ui_metadata;

pub use env::{RuntimeEnvConfigSource, runtime_config_from_env};
pub use migration::{RuntimeConfigMigrationPlan, runtime_config_migration_plan};
pub use schema::{
    ConfigFieldSchema, ConfigValueType, RUNTIME_CONFIG_SCHEMA_VERSION, RuntimeConfigSchema,
    runtime_config_schema,
};
pub use secrets::{REDACTED_SECRET, SecretValue, redact_optional_secret};
pub use ui_metadata::{
    ConfigUiControl, ConfigUiField, ConfigUiGroup, ConfigUiMetadata, runtime_config_ui_metadata,
};

use crate::config_io::read_runtime_config;
use crate::defaults::{
    default_chat_provider_id, default_chat_providers, default_dashboard_jwt_secret,
    default_dashboard_password, default_dashboard_token_ttl_seconds, default_dashboard_username,
    default_event_queue_capacity, default_platforms,
};
use crate::path_config::RuntimePathConfig;
use crate::platform_config::{
    RuntimeCommandPluginConfig, RuntimePlatformConfig, RuntimeWebChatServerConfig,
};
use crate::policy_config::{
    RuntimeContentSafetyConfig, RuntimeProviderFallbackConfig, RuntimeRateLimitConfig,
    RuntimeResultDecorateConfig, RuntimeSessionStatusConfig, RuntimeStatePolicyConfig,
    RuntimeWakeCheckConfig, RuntimeWhitelistPolicyConfig,
};
use crate::provider_config::{
    RuntimeChatProviderConfig, RuntimeEmbeddingProviderConfig, RuntimeExternalAgentConfig,
    RuntimeProviderSourceConfig, RuntimeRerankProviderConfig, RuntimeSpeechToTextProviderConfig,
    RuntimeTextToSpeechProviderConfig,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default = "default_event_queue_capacity")]
    pub event_queue_capacity: usize,
    #[serde(default)]
    pub paths: RuntimePathConfig,
    #[serde(default = "default_chat_provider_id")]
    pub default_chat_provider_id: String,
    #[serde(default = "default_chat_providers")]
    pub chat_providers: Vec<RuntimeChatProviderConfig>,
    #[serde(default)]
    pub provider_sources: Vec<RuntimeProviderSourceConfig>,
    #[serde(default)]
    pub default_speech_to_text_provider_id: Option<String>,
    #[serde(default)]
    pub speech_to_text_providers: Vec<RuntimeSpeechToTextProviderConfig>,
    #[serde(default)]
    pub default_text_to_speech_provider_id: Option<String>,
    #[serde(default)]
    pub text_to_speech_providers: Vec<RuntimeTextToSpeechProviderConfig>,
    #[serde(default)]
    pub default_embedding_provider_id: Option<String>,
    #[serde(default)]
    pub embedding_providers: Vec<RuntimeEmbeddingProviderConfig>,
    #[serde(default)]
    pub default_rerank_provider_id: Option<String>,
    #[serde(default)]
    pub rerank_providers: Vec<RuntimeRerankProviderConfig>,
    #[serde(default)]
    pub external_agent_runners: Vec<RuntimeExternalAgentConfig>,
    #[serde(default = "default_platforms")]
    pub platforms: Vec<RuntimePlatformConfig>,
    #[serde(default)]
    pub wake_check: RuntimeWakeCheckConfig,
    #[serde(default)]
    pub whitelist_policy: RuntimeWhitelistPolicyConfig,
    #[serde(default)]
    pub session_status: RuntimeSessionStatusConfig,
    #[serde(default)]
    pub rate_limit: RuntimeRateLimitConfig,
    #[serde(default)]
    pub content_safety: RuntimeContentSafetyConfig,
    #[serde(default)]
    pub provider_fallback: RuntimeProviderFallbackConfig,
    #[serde(default)]
    pub result_decorate: RuntimeResultDecorateConfig,
    #[serde(default)]
    pub state_policy: RuntimeStatePolicyConfig,
    #[serde(default)]
    pub dashboard_auth: RuntimeDashboardAuthConfig,
    #[serde(default)]
    pub webchat_server: RuntimeWebChatServerConfig,
    #[serde(default)]
    pub command_plugins: Vec<RuntimeCommandPluginConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeDashboardAuthConfig {
    #[serde(default = "default_dashboard_username")]
    pub username: String,
    #[serde(default = "default_dashboard_password")]
    pub password: String,
    #[serde(default = "default_dashboard_jwt_secret")]
    pub jwt_secret: String,
    #[serde(default = "default_dashboard_token_ttl_seconds")]
    pub token_ttl_seconds: u64,
}

impl Default for RuntimeDashboardAuthConfig {
    fn default() -> Self {
        Self {
            username: default_dashboard_username(),
            password: default_dashboard_password(),
            jwt_secret: default_dashboard_jwt_secret(),
            token_ttl_seconds: default_dashboard_token_ttl_seconds(),
        }
    }
}

impl RuntimeDashboardAuthConfig {
    pub fn is_default_credential(&self) -> bool {
        self.username == default_dashboard_username()
            && self.password == default_dashboard_password()
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            event_queue_capacity: default_event_queue_capacity(),
            paths: RuntimePathConfig::default(),
            default_chat_provider_id: default_chat_provider_id(),
            chat_providers: default_chat_providers(),
            provider_sources: Vec::new(),
            default_speech_to_text_provider_id: None,
            speech_to_text_providers: Vec::new(),
            default_text_to_speech_provider_id: None,
            text_to_speech_providers: Vec::new(),
            default_embedding_provider_id: None,
            embedding_providers: Vec::new(),
            default_rerank_provider_id: None,
            rerank_providers: Vec::new(),
            external_agent_runners: Vec::new(),
            platforms: default_platforms(),
            wake_check: RuntimeWakeCheckConfig::default(),
            whitelist_policy: RuntimeWhitelistPolicyConfig::default(),
            session_status: RuntimeSessionStatusConfig::default(),
            rate_limit: RuntimeRateLimitConfig::default(),
            content_safety: RuntimeContentSafetyConfig::default(),
            provider_fallback: RuntimeProviderFallbackConfig::default(),
            result_decorate: RuntimeResultDecorateConfig::default(),
            state_policy: RuntimeStatePolicyConfig::default(),
            dashboard_auth: RuntimeDashboardAuthConfig::default(),
            webchat_server: RuntimeWebChatServerConfig::default(),
            command_plugins: Vec::new(),
        }
    }
}

impl RuntimeConfig {
    pub fn new(chat_providers: Vec<RuntimeChatProviderConfig>) -> Self {
        Self {
            chat_providers,
            ..Self::default()
        }
    }

    pub fn with_default_chat_provider_id(mut self, id: impl Into<String>) -> Self {
        self.default_chat_provider_id = id.into();
        self
    }

    pub fn from_json_file(path: impl AsRef<Path>) -> Result<Self> {
        read_runtime_config(path.as_ref())
    }

    pub fn from_env() -> Self {
        runtime_config_from_env()
    }

    pub fn schema() -> RuntimeConfigSchema {
        runtime_config_schema()
    }

    pub fn ui_metadata() -> ConfigUiMetadata {
        runtime_config_ui_metadata()
    }
}
