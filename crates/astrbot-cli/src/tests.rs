use std::path::PathBuf;

use astrbot_runtime::{RuntimePlatformConfig, RuntimeWebChatServerConfig};
use astrbot_web::SubmitTextRequest;

use crate::args::{CliCommand, default_config_path, parse_command_from};
use crate::webchat_server::prepare_webchat_server;

#[test]
fn default_invocation_uses_smoke_mode() {
    assert_eq!(
        parse_command_from(Vec::<String>::new()).expect("parse should work"),
        CliCommand::Smoke { config_path: None }
    );
}

#[test]
fn init_uses_default_config_path() {
    assert_eq!(
        parse_command_from(vec!["init".to_string()]).expect("parse should work"),
        CliCommand::Init {
            config_path: default_config_path()
        }
    );
}

#[test]
fn run_accepts_config_path() {
    assert_eq!(
        parse_command_from(vec!["run".to_string(), "custom.json".to_string()])
            .expect("parse should work"),
        CliCommand::Run {
            config_path: PathBuf::from("custom.json")
        }
    );
}

#[tokio::test]
async fn webchat_server_config_binds_to_runtime_platform() {
    let config = astrbot_runtime::RuntimeConfig {
        platforms: vec![RuntimePlatformConfig::webchat("webchat")],
        webchat_server: RuntimeWebChatServerConfig::enabled("webchat", "127.0.0.1", 0),
        ..astrbot_runtime::RuntimeConfig::default()
    };
    let runtime = astrbot_runtime::AstrbotRuntime::initialize(config.clone())
        .expect("runtime should initialize");
    let pending = prepare_webchat_server(&runtime, &config.webchat_server)
        .await
        .expect("webchat server should prepare")
        .expect("webchat server should be enabled");

    assert_eq!(pending.address.ip().to_string(), "127.0.0.1");
    assert_ne!(pending.address.port(), 0);
}

#[tokio::test]
async fn cli_webchat_server_submits_events_to_runtime() {
    let config = astrbot_runtime::RuntimeConfig {
        platforms: vec![RuntimePlatformConfig::webchat("webchat")],
        webchat_server: RuntimeWebChatServerConfig::enabled("webchat", "127.0.0.1", 0),
        ..astrbot_runtime::RuntimeConfig::default()
    };
    let runtime = astrbot_runtime::AstrbotRuntime::initialize(config.clone())
        .expect("runtime should initialize");
    let pending = prepare_webchat_server(&runtime, &config.webchat_server)
        .await
        .expect("webchat server should prepare")
        .expect("webchat server should be enabled");
    let address = pending.address;
    let handle = runtime.start();
    let server = pending.start();

    let response = reqwest::Client::new()
        .post(format!("http://{address}/api/webchat/conversation-1"))
        .json(&SubmitTextRequest {
            sender_id: "user-1".to_string(),
            text: "hello from cli webchat".to_string(),
            image_urls: Vec::new(),
            message_parts: Vec::new(),
        })
        .send()
        .await
        .expect("HTTP request should succeed");

    assert!(response.status().is_success());

    let sent = wait_for_sent_messages(&handle, "webchat", 1).await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].session.conversation_id, "conversation-1");
    assert_eq!(sent[0].chain.plain_text(), "hello from astrbot-rs");

    server.stop().await.expect("server should stop");
    handle.stop().await.expect("runtime should stop");
}

async fn wait_for_sent_messages(
    handle: &astrbot_runtime::RuntimeHandle,
    platform_id: &str,
    expected: usize,
) -> Vec<astrbot_platform::SentMessage> {
    for _ in 0..64 {
        let sent = handle.sent_messages_for(platform_id).await;
        if sent.len() >= expected {
            return sent;
        }
        tokio::task::yield_now().await;
    }
    handle.sent_messages_for(platform_id).await
}
