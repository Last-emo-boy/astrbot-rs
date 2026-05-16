mod assembly;
mod config;
mod config_io;
mod defaults;
mod handle;
mod path_config;
mod platform_config;
mod policy_config;
mod ports;
mod provider_config;

pub use config::{
    ConfigFieldSchema, ConfigUiControl, ConfigUiField, ConfigUiGroup, ConfigUiMetadata,
    ConfigValueType, REDACTED_SECRET, RUNTIME_CONFIG_SCHEMA_VERSION, RuntimeConfig,
    RuntimeConfigMigrationPlan, RuntimeConfigSchema, RuntimeEnvConfigSource, SecretValue,
    redact_optional_secret, runtime_config_from_env, runtime_config_migration_plan,
    runtime_config_schema, runtime_config_ui_metadata,
};
pub use handle::{AstrbotRuntime, RuntimeHandle};
pub use path_config::{RuntimePathConfig, RuntimePathLayout};
pub use platform_config::{
    RuntimeCommandPluginConfig, RuntimePlatformConfig, RuntimeWebChatServerConfig,
};
pub use policy_config::{
    RuntimeContentSafetyConfig, RuntimeKeywordContentSafetyConfig, RuntimeProviderFallbackConfig,
    RuntimeRateLimitConfig, RuntimeRateLimitStrategy, RuntimeResultDecorateConfig,
    RuntimeSessionStatusConfig, RuntimeStatePolicyConfig, RuntimeWakeCheckConfig,
    RuntimeWhitelistPolicyConfig,
};
pub use provider_config::{
    RuntimeChatProviderConfig, RuntimeEmbeddingProviderConfig, RuntimeRerankProviderConfig,
    RuntimeSpeechToTextProviderConfig, RuntimeTextToSpeechProviderConfig,
};

#[cfg(test)]
mod tests;
