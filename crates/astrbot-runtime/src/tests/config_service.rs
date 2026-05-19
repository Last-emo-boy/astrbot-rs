use std::fs;

use serde_json::json;

use crate::{
    REDACTED_SECRET, RuntimeChatProviderConfig, RuntimeConfig, RuntimeConfigReloadAction,
    RuntimeConfigService, RuntimePlatformConfig, RuntimeProviderSourceConfig,
    RuntimeWebChatServerConfig, UmopConfigRoute,
};

use super::temp_runtime_config_path;

#[test]
fn config_service_validates_writes_and_plans_restart_boundaries() {
    let path = temp_runtime_config_path("config-service-save");
    let _ = fs::remove_file(&path);
    let service = RuntimeConfigService::new(&path);
    let mut config = RuntimeConfig::default();
    config.webchat_server = RuntimeWebChatServerConfig::enabled("webchat", "0.0.0.0", 7000);

    let preview = service
        .preview_update_value(serde_json::to_value(&config).expect("config should serialize"))
        .expect("preview should validate");
    assert_eq!(
        preview.plan.reload_action,
        RuntimeConfigReloadAction::RestartRuntime
    );
    assert_eq!(preview.plan.changed_fields, vec!["webchat_server"]);
    assert!(preview.plan.write_required);
    assert!(preview.plan.restart_required);

    service
        .save_update_value(serde_json::to_value(&config).expect("config should serialize"))
        .expect("config should save");
    let saved = RuntimeConfig::from_json_file(&path).expect("saved config should load");
    assert_eq!(saved.webchat_server.port, 7000);

    let _ = fs::remove_file(path);
}

#[test]
fn config_service_rejects_invalid_runtime_config_shape() {
    let path = temp_runtime_config_path("config-service-invalid");
    let _ = fs::remove_file(&path);
    let service = RuntimeConfigService::new(&path);

    let error = service
        .preview_update_value(json!({"event_queue_capacity": "large"}))
        .expect_err("invalid config should fail validation");

    assert!(error.to_string().contains("validate runtime config"));

    let _ = fs::remove_file(path);
}

#[test]
fn config_service_preserves_redacted_secrets_on_full_apply() {
    let path = temp_runtime_config_path("config-service-redacted");
    let _ = fs::remove_file(&path);
    let service = RuntimeConfigService::new(&path);
    let mut current = RuntimeConfig::default();
    current.chat_providers = vec![
        RuntimeChatProviderConfig::openai_compatible("openai", "https://api.example", "model")
            .with_api_key("sk-current"),
    ];
    current.provider_sources = vec![
        RuntimeProviderSourceConfig::openai("openai-source", "https://api.example")
            .with_api_key("sk-source-current"),
    ];
    current.platforms = vec![RuntimePlatformConfig::telegram(
        "telegram",
        "telegram-current",
    )];
    current.dashboard_auth.password = "pw-current".to_string();
    current.dashboard_auth.jwt_secret = "jwt-current".to_string();
    fs::write(
        &path,
        serde_json::to_string_pretty(&current).expect("config should serialize"),
    )
    .expect("current config should save");

    let mut next = current.clone();
    next.chat_providers[0].api_key = Some(REDACTED_SECRET.to_string());
    next.provider_sources[0].api_key = Some(REDACTED_SECRET.to_string());
    next.platforms[0]
        .secrets
        .insert("telegram_token".to_string(), REDACTED_SECRET.to_string());
    next.dashboard_auth.password = REDACTED_SECRET.to_string();
    next.dashboard_auth.jwt_secret = REDACTED_SECRET.to_string();
    next.default_chat_provider_id = "openai".to_string();

    let preview = service
        .save_update_value(serde_json::to_value(&next).expect("config should serialize"))
        .expect("redacted config should apply");

    assert_eq!(
        preview.config.chat_providers[0].api_key.as_deref(),
        Some("sk-current")
    );
    assert_eq!(
        preview.config.provider_sources[0].api_key.as_deref(),
        Some("sk-source-current")
    );
    assert_eq!(
        preview.config.platforms[0]
            .secrets
            .get("telegram_token")
            .map(String::as_str),
        Some("telegram-current")
    );
    assert_eq!(preview.config.dashboard_auth.password, "pw-current");
    assert_eq!(preview.config.dashboard_auth.jwt_secret, "jwt-current");
    let saved = RuntimeConfig::from_json_file(&path).expect("saved config should load");
    assert_eq!(
        saved.chat_providers[0].api_key.as_deref(),
        Some("sk-current")
    );
    assert_eq!(
        saved.provider_sources[0].api_key.as_deref(),
        Some("sk-source-current")
    );
    assert_eq!(
        saved.platforms[0]
            .secrets
            .get("telegram_token")
            .map(String::as_str),
        Some("telegram-current")
    );
    assert_eq!(saved.dashboard_auth.password, "pw-current");
    assert_eq!(saved.dashboard_auth.jwt_secret, "jwt-current");

    let _ = fs::remove_file(path);
}

#[test]
fn config_service_persists_abconfs_and_umop_routes() {
    let path = temp_runtime_config_path("config-service-abconf-umop");
    let _ = fs::remove_file(&path);
    let service = RuntimeConfigService::new(&path);

    let created = service
        .create_abconf(
            Some("Ops".to_string()),
            Some(serde_json::to_value(RuntimeConfig::default()).expect("config should serialize")),
        )
        .expect("abconf should create");
    assert_eq!(created.name, "Ops");
    assert_eq!(
        service.list_abconfs().expect("abconfs should list")[0].id,
        created.id
    );
    let updated = service
        .update_abconf_info(&created.id, Some("Ops Updated".to_string()))
        .expect("abconf should update")
        .expect("abconf should exist");
    assert_eq!(updated.name, "Ops Updated");

    service
        .save_umop_config_routes(&[UmopConfigRoute::new("webchat:group:room-*", &created.id)])
        .expect("routes should save");
    let reloaded = RuntimeConfigService::new(&path)
        .read_umop_config_router()
        .expect("routes should reload");
    assert_eq!(
        reloaded.resolve_config_id("webchat:group:room-1"),
        Some(created.id.as_str())
    );
    assert!(
        service
            .delete_abconf(&created.id)
            .expect("abconf should delete")
    );
    assert!(
        service
            .get_abconf(&created.id)
            .expect("abconf should load")
            .is_none()
    );

    let _ = fs::remove_file(path);
}
