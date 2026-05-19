use std::collections::HashMap;

use crate::{
    ConfigUiControl, ConfigValueType, REDACTED_SECRET, RuntimeConfig, RuntimeEnvConfigSource,
    RuntimePlatformConfig, SecretValue, redact_optional_secret, runtime_config_migration_plan,
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
fn runtime_platform_config_deserializes_options_and_redacts_secrets() {
    let platform: RuntimePlatformConfig = serde_json::from_value(serde_json::json!({
        "id": "telegram",
        "type": "telegram",
        "options": {
            "telegram_api_base_url": "https://api.telegram.org/bot"
        },
        "secrets": {
            "telegram_token": "secret-token"
        }
    }))
    .expect("platform config should deserialize");

    assert_eq!(platform.platform_type, "telegram");
    assert_eq!(
        platform
            .options
            .get("telegram_api_base_url")
            .and_then(serde_json::Value::as_str),
        Some("https://api.telegram.org/bot")
    );
    assert_eq!(
        platform.secrets.get("telegram_token").map(String::as_str),
        Some("secret-token")
    );

    let debug = format!("{platform:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("secret-token"));
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

    let source_api_key = schema
        .field("provider_sources[].api_key")
        .expect("provider source api key field should exist");
    assert!(source_api_key.secret);
    assert_eq!(source_api_key.value_type, ConfigValueType::OptionalString);

    let provider_wake_prefix = schema
        .field("provider_fallback.wake_prefix")
        .expect("provider wake prefix field should exist");
    assert_eq!(provider_wake_prefix.value_type, ConfigValueType::String);
    assert_eq!(provider_wake_prefix.default_value, serde_json::json!(""));

    let platform_secrets = schema
        .field("platforms[].secrets")
        .expect("platform secrets field should exist");
    assert!(platform_secrets.secret);
    assert_eq!(platform_secrets.value_type, ConfigValueType::Object);

    let telegram_token = schema
        .field("platforms[].secrets.telegram_token")
        .expect("telegram token field should exist");
    assert!(telegram_token.secret);
    assert_eq!(telegram_token.value_type, ConfigValueType::OptionalString);

    let dingtalk_client_secret = schema
        .field("platforms[].secrets.client_secret")
        .expect("dingtalk client secret field should exist");
    assert!(dingtalk_client_secret.secret);
    assert_eq!(
        dingtalk_client_secret.value_type,
        ConfigValueType::OptionalString
    );

    let dashboard_password = schema
        .field("dashboard_auth.password")
        .expect("dashboard password field should exist");
    assert!(dashboard_password.secret);
    assert_eq!(dashboard_password.value_type, ConfigValueType::String);

    let active_template = schema
        .field("result_decorate.t2i_active_template")
        .expect("t2i active template field should exist");
    assert_eq!(active_template.value_type, ConfigValueType::String);
    assert_eq!(active_template.default_value, serde_json::json!("base"));

    let tts_enabled = schema
        .field("result_decorate.tts_enabled")
        .expect("tts enabled field should exist");
    assert_eq!(tts_enabled.value_type, ConfigValueType::Bool);
    assert_eq!(tts_enabled.default_value, serde_json::json!(false));

    let tts_provider_id = schema
        .field("result_decorate.tts_provider_id")
        .expect("tts provider field should exist");
    assert_eq!(tts_provider_id.value_type, ConfigValueType::OptionalString);
    assert_eq!(tts_provider_id.default_value, serde_json::json!(null));

    let content_safety_after_transform = schema
        .field("result_decorate.content_safety_after_transform")
        .expect("post-transform content safety field should exist");
    assert_eq!(
        content_safety_after_transform.value_type,
        ConfigValueType::Bool
    );
    assert_eq!(
        content_safety_after_transform.default_value,
        serde_json::json!(false)
    );
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
            .field("provider_sources[].api_key")
            .expect("provider source api key metadata should exist")
            .control,
        ConfigUiControl::Password
    );
    assert_eq!(
        metadata
            .field("provider_fallback.wake_prefix")
            .expect("provider wake prefix metadata should exist")
            .control,
        ConfigUiControl::Text
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
            .field("platforms[].secrets.telegram_token")
            .expect("telegram token metadata should exist")
            .control,
        ConfigUiControl::Password
    );
    assert_eq!(
        metadata
            .field("platforms[].secrets.client_secret")
            .expect("dingtalk client secret metadata should exist")
            .control,
        ConfigUiControl::Password
    );
    assert_eq!(
        metadata
            .field("dashboard_auth.jwt_secret")
            .expect("dashboard jwt secret metadata should exist")
            .control,
        ConfigUiControl::Password
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
    assert_eq!(
        metadata
            .field("result_decorate.t2i_enabled")
            .expect("t2i metadata should exist")
            .control,
        ConfigUiControl::Toggle
    );
    assert_eq!(
        metadata
            .field("result_decorate.tts_enabled")
            .expect("tts metadata should exist")
            .control,
        ConfigUiControl::Toggle
    );
    assert_eq!(
        metadata
            .field("result_decorate.tts_provider_id")
            .expect("tts provider metadata should exist")
            .control,
        ConfigUiControl::Text
    );
    assert_eq!(
        metadata
            .field("result_decorate.content_safety_after_transform")
            .expect("post-transform content safety metadata should exist")
            .control,
        ConfigUiControl::Toggle
    );
}
