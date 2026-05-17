mod assembly;
mod config;
mod config_io;
mod config_route;
mod config_service;
mod dashboard_assets;
mod defaults;
mod handle;
mod path_config;
mod platform_config;
mod policy_config;
mod ports;
mod provider_config;
mod provider_selection;

pub use config::{
    ConfigFieldSchema, ConfigUiControl, ConfigUiField, ConfigUiGroup, ConfigUiMetadata,
    ConfigValueType, REDACTED_SECRET, RUNTIME_CONFIG_SCHEMA_VERSION, RuntimeConfig,
    RuntimeConfigMigrationPlan, RuntimeConfigSchema, RuntimeEnvConfigSource, SecretValue,
    redact_optional_secret, runtime_config_from_env, runtime_config_migration_plan,
    runtime_config_schema, runtime_config_ui_metadata,
};
pub use config_route::{UmopConfigRoute, UmopConfigRoutePattern, UmopConfigRouter};
pub use config_service::{
    RuntimeConfigMutationPlan, RuntimeConfigReloadAction, RuntimeConfigService,
    RuntimeConfigUpdatePreview, validate_runtime_config_value,
};
pub use dashboard_assets::{
    DASHBOARD_INDEX_ROUTES, DashboardAssetPolicy, DashboardAssetSelection, DashboardAssetSource,
    is_dashboard_index_route,
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
    RuntimeChatProviderConfig, RuntimeEmbeddingProviderConfig, RuntimeExternalAgentConfig,
    RuntimeRerankProviderConfig, RuntimeSpeechToTextProviderConfig,
    RuntimeTextToSpeechProviderConfig,
};
pub use provider_selection::RuntimeProviderSelectionSnapshot;

#[cfg(test)]
mod tests;
