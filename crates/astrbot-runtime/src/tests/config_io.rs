use std::fs;

use astrbot_platform::WEBCHAT_PLATFORM_TYPE;

use crate::RuntimeConfig;

use super::temp_runtime_config_path;

#[test]
fn missing_config_file_is_created_with_defaults() {
    let path = temp_runtime_config_path("missing");
    let _ = fs::remove_file(&path);

    let config = RuntimeConfig::from_json_file(&path).expect("default config should load");

    assert_eq!(config, RuntimeConfig::default());
    assert!(path.exists());
    let content = fs::read_to_string(&path).expect("config file should be readable");
    assert!(content.contains("\"platforms\""));
    assert!(content.contains("\"wake_check\""));
    assert!(content.contains("\"whitelist_policy\""));
    assert!(content.contains("\"session_status\""));
    assert!(content.contains("\"rate_limit\""));
    assert!(content.contains("\"content_safety\""));
    assert!(content.contains("\"provider_fallback\""));
    assert!(content.contains("\"result_decorate\""));
    assert!(content.contains("\"state_policy\""));
    assert!(content.contains("\"webchat_server\""));
    let _ = fs::remove_file(path);
}

#[test]
fn missing_top_level_defaults_are_written_back() {
    let path = temp_runtime_config_path("normalize");
    let _ = fs::remove_file(&path);
    fs::write(&path, r#"{"event_queue_capacity":16}"#).expect("seed config should be writable");

    let config = RuntimeConfig::from_json_file(&path).expect("config should load");

    assert_eq!(config.event_queue_capacity, 16);
    assert_eq!(
        config.chat_providers,
        RuntimeConfig::default().chat_providers
    );
    assert_eq!(config.platforms, RuntimeConfig::default().platforms);
    let content = fs::read_to_string(&path).expect("normalized file should be readable");
    assert!(content.contains("\"chat_providers\""));
    assert!(content.contains("\"platforms\""));
    assert!(content.contains("\"wake_check\""));
    assert!(content.contains("\"whitelist_policy\""));
    assert!(content.contains("\"session_status\""));
    assert!(content.contains("\"rate_limit\""));
    assert!(content.contains("\"content_safety\""));
    assert!(content.contains("\"provider_fallback\""));
    assert!(content.contains("\"result_decorate\""));
    assert!(content.contains("\"state_policy\""));
    assert!(content.contains("\"webchat_server\""));
    let _ = fs::remove_file(path);
}

#[test]
fn missing_webchat_server_defaults_are_written_back() {
    let path = temp_runtime_config_path("webchat-server");
    let _ = fs::remove_file(&path);
    fs::write(&path, r#"{"webchat_server":{"enabled":true}}"#)
        .expect("seed config should be writable");

    let config = RuntimeConfig::from_json_file(&path).expect("config should load");

    assert!(config.webchat_server.enabled);
    assert_eq!(config.webchat_server.platform_id, WEBCHAT_PLATFORM_TYPE);
    assert_eq!(config.webchat_server.host, "127.0.0.1");
    assert_eq!(config.webchat_server.port, 6185);
    let content = fs::read_to_string(&path).expect("normalized file should be readable");
    assert!(content.contains("\"webchat_server\""));
    assert!(content.contains("\"platform_id\""));
    let _ = fs::remove_file(path);
}
