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
mod tool_assembly;

pub use config::{
    ConfigFieldSchema, ConfigUiControl, ConfigUiField, ConfigUiGroup, ConfigUiMetadata,
    ConfigValueType, REDACTED_SECRET, RUNTIME_CONFIG_SCHEMA_VERSION, RuntimeConfig,
    RuntimeConfigMigrationPlan, RuntimeConfigSchema, RuntimeDashboardAuthConfig,
    RuntimeEnvConfigSource, SecretValue, redact_optional_secret, runtime_config_from_env,
    runtime_config_migration_plan, runtime_config_schema, runtime_config_ui_metadata,
};
pub use config_route::{
    UmopConfigRoute, UmopConfigRoutePattern, UmopConfigRouteStore, UmopConfigRouter,
};
pub use config_service::{
    DEFAULT_ABCONF_ID, RuntimeAbconfDescriptor, RuntimeAbconfRecord, RuntimeConfigMutationPlan,
    RuntimeConfigReloadAction, RuntimeConfigService, RuntimeConfigUpdatePreview,
    validate_runtime_config_value,
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
    RuntimeBaiduAipContentSafetyConfig, RuntimeContentSafetyConfig,
    RuntimeKeywordContentSafetyConfig, RuntimeProviderFallbackConfig, RuntimeRateLimitConfig,
    RuntimeRateLimitStrategy, RuntimeResultDecorateConfig, RuntimeSessionStatusConfig,
    RuntimeStatePolicyConfig, RuntimeWakeCheckConfig, RuntimeWhitelistPolicyConfig,
};
pub use provider_config::{
    RuntimeChatProviderConfig, RuntimeEmbeddingProviderConfig, RuntimeExternalAgentConfig,
    RuntimeProviderSourceConfig, RuntimeRerankProviderConfig, RuntimeSpeechToTextProviderConfig,
    RuntimeTextToSpeechProviderConfig,
};
pub use provider_selection::RuntimeProviderSelectionSnapshot;
pub use tool_assembly::{
    RuntimeInternalToolAssembly, runtime_internal_tool_catalog, runtime_internal_tool_registrations,
};

#[cfg(test)]
mod tests;
