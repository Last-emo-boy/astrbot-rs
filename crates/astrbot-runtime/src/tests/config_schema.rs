use std::collections::HashMap;

use crate::{
    ConfigUiControl, ConfigValueType, REDACTED_SECRET, RuntimeConfig, RuntimeEnvConfigSource,
    SecretValue, redact_optional_secret, runtime_config_migration_plan,
};

#[test]
fn env_config_source_builds_openai_provider_from_lookup() {
    let values = HashMap::from([
        ("ASTRBOT_OPENAI_API_KEY", "sk-test"),
        ("ASTRBOT_OPENAI_API_BASE", "https://openai.example/v1"),
        ("ASTRBOT_OPENAI_MODEL", "gpt-test"),
        ("ASTRBOT_OPENAI_TIMEOUT_SECS", "42"),
    ]);
    let source = RuntimeEnvConfigSource::default();

    let config = source.load_from(|key| values.get(key).map(|value| value.to_string()));

    assert_eq!(config.default_chat_provider_id, "env-openai");
    assert_eq!(config.chat_providers.len(), 1);
    let provider = &config.chat_providers[0];
    assert_eq!(provider.api_key.as_deref(), Some("sk-test"));
    assert_eq!(
        provider.api_base.as_deref(),
        Some("https://openai.example/v1")
    );
    assert_eq!(provider.model.as_deref(), Some("gpt-test"));
    assert_eq!(provider.timeout_secs, 42);
}

#[test]
fn env_config_source_uses_defaults_and_ignores_empty_key() {
    let source = RuntimeEnvConfigSource::default();

    let empty_key_config =
        source.load_from(|key| (key == "ASTRBOT_OPENAI_API_KEY").then(|| "   ".to_string()));
    assert_eq!(empty_key_config, RuntimeConfig::default());

    let defaulted_config = source
        .load_from(|key| (key == "ASTRBOT_OPENAI_API_KEY").then(|| "sk-defaults".to_string()));
    let provider = &defaulted_config.chat_providers[0];
    assert_eq!(
        provider.api_base.as_deref(),
        Some("https://api.openai.com/v1")
    );
    assert_eq!(provider.model.as_deref(), Some("gpt-4o-mini"));
    assert_eq!(provider.timeout_secs, 120);
}

#[test]
fn secret_values_redact_debug_and_optional_display() {
    let secret = SecretValue::new("sk-sensitive");

    assert_eq!(secret.expose_secret(), "sk-sensitive");
    assert_eq!(secret.redacted(), REDACTED_SECRET);
    assert!(!format!("{secret:?}").contains("sk-sensitive"));
    assert_eq!(
        redact_optional_secret(Some("sk-sensitive")),
        Some(REDACTED_SECRET)
    );
    assert_eq!(redact_optional_secret(Some("")), None);
    assert_eq!(redact_optional_secret(None), None);
}

#[test]
fn migration_plan_reports_missing_top_level_and_nested_defaults() {
    let plan = runtime_config_migration_plan(r#"{"webchat_server":{"enabled":true}}"#)
        .expect("migration plan should parse");

    assert!(
        plan.missing_default_keys
            .contains(&"chat_providers".to_string())
    );
    assert!(
        plan.missing_default_keys
            .contains(&"webchat_server.platform_id".to_string())
    );
    assert!(
        plan.missing_default_keys
            .contains(&"webchat_server.host".to_string())
    );
    assert!(
        plan.missing_default_keys
            .contains(&"webchat_server.port".to_string())
    );
}

#[test]
fn runtime_config_schema_marks_secret_and_common_defaults() {
    let schema = RuntimeConfig::schema();

    let queue = schema
        .field("event_queue_capacity")
        .expect("queue field should exist");
    assert_eq!(queue.value_type, ConfigValueType::Integer);
    assert_eq!(queue.default_value, serde_json::json!(8));

    let paths = schema.field("paths").expect("paths field should exist");
    assert_eq!(paths.value_type, ConfigValueType::Object);

    let temp_dir = schema
        .field("paths.temp_dir")
        .expect("paths temp field should exist");
    assert_eq!(temp_dir.value_type, ConfigValueType::OptionalString);

    let api_key = schema
        .field("chat_providers[].api_key")
        .expect("provider api key field should exist");
    assert!(api_key.secret);
    assert_eq!(api_key.value_type, ConfigValueType::OptionalString);
}

#[test]
fn runtime_config_ui_metadata_groups_controls_without_runtime_internals() {
    let metadata = RuntimeConfig::ui_metadata();

    assert_eq!(
        metadata
            .field("chat_providers[].api_key")
            .expect("api key metadata should exist")
            .control,
        ConfigUiControl::Password
    );
    assert_eq!(
        metadata
            .field("webchat_server.enabled")
            .expect("webchat enabled metadata should exist")
            .control,
        ConfigUiControl::Toggle
    );
    assert_eq!(
        metadata
            .field("webchat_server.port")
            .expect("webchat port metadata should exist")
            .control,
        ConfigUiControl::Number
    );
    assert_eq!(
        metadata
            .field("paths.temp_dir")
            .expect("paths temp metadata should exist")
            .control,
        ConfigUiControl::Text
    );
}
