use std::fs;

use serde_json::json;

use crate::{
    RuntimeConfig, RuntimeConfigReloadAction, RuntimeConfigService, RuntimeWebChatServerConfig,
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
