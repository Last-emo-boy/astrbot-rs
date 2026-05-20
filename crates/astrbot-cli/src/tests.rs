use std::{path::PathBuf, sync::Arc};

use astrbot_platform::CONSOLE_PLATFORM_TYPE;
use astrbot_runtime::{RuntimePlatformConfig, RuntimeWebChatServerConfig};
use astrbot_storage::BackupManifest;
use astrbot_web::{
    MaintenanceRestartExecutor, MaintenanceRestartRequest, ManagementStatusResponse,
    SubmitTextRequest,
};
use tokio::sync::mpsc;

use crate::args::{CliCommand, default_config_path, parse_command_from};
use crate::commands;
use crate::webchat_server::{prepare_webchat_server, prepare_webchat_server_with_config_apply};

const OPENAI_CHAT_PROVIDER_TYPE: &str = "openai_chat_completion";
const MOCK_CHAT_PROVIDER_TYPE: &str = "mock_chat_completion";

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
async fn init_creates_dashboard_ready_config() {
    let path = std::env::temp_dir().join(format!(
        "astrbot-cli-init-{}-dashboard.json",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    commands::execute(CliCommand::Init {
        config_path: path.clone(),
    })
    .await
    .expect("init should create config");
    let config =
        astrbot_runtime::RuntimeConfig::from_json_file(&path).expect("created config should parse");

    assert!(config.webchat_server.enabled);
    assert_eq!(config.webchat_server.platform_id, "webchat");
    assert_eq!(
        config.default_embedding_provider_id.as_deref(),
        Some("embedding")
    );
    assert_eq!(config.default_rerank_provider_id.as_deref(), Some("rerank"));
    assert!(
        config
            .platforms
            .iter()
            .any(|platform| platform.id == "webchat")
    );

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn webchat_server_config_binds_to_runtime_platform() {
    let config_path = temp_cli_config_path("binds");
    write_test_config(&config_path);
    let config = astrbot_runtime::RuntimeConfig {
        platforms: vec![RuntimePlatformConfig::webchat("webchat")],
        webchat_server: RuntimeWebChatServerConfig::enabled("webchat", "127.0.0.1", 0),
        ..astrbot_runtime::RuntimeConfig::default()
    };
    let runtime = astrbot_runtime::AstrbotRuntime::initialize(config.clone())
        .expect("runtime should initialize");
    let pending = prepare_webchat_server(&runtime, &config.webchat_server, &config_path)
        .await
        .expect("webchat server should prepare")
        .expect("webchat server should be enabled");

    assert_eq!(pending.address.ip().to_string(), "127.0.0.1");
    assert_ne!(pending.address.port(), 0);
    let _ = std::fs::remove_file(config_path);
}

#[tokio::test]
async fn cli_webchat_server_submits_events_to_runtime() {
    let config_path = temp_cli_config_path("server");
    write_test_config(&config_path);
    let config = astrbot_runtime::RuntimeConfig {
        platforms: vec![RuntimePlatformConfig::webchat("webchat")],
        webchat_server: RuntimeWebChatServerConfig::enabled("webchat", "127.0.0.1", 0),
        ..astrbot_runtime::RuntimeConfig::default()
    };
    let runtime = astrbot_runtime::AstrbotRuntime::initialize(config.clone())
        .expect("runtime should initialize");
    let pending = prepare_webchat_server(&runtime, &config.webchat_server, &config_path)
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

    let login: serde_json::Value = reqwest::Client::new()
        .post(format!("http://{address}/api/auth/login"))
        .json(&serde_json::json!({
            "username": "astrbot",
            "password": "77b90590a8945a7d36c963981a307dc9"
        }))
        .send()
        .await
        .expect("dashboard login request should succeed")
        .json()
        .await
        .expect("dashboard login response should parse");
    let dashboard_token = login["data"]["token"]
        .as_str()
        .expect("dashboard token should be present");
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {dashboard_token}")
            .parse()
            .expect("auth header should parse"),
    );
    let management_client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .expect("management client should build");

    let status: ManagementStatusResponse = management_client
        .get(format!("http://{address}/api/management/status"))
        .send()
        .await
        .expect("management request should succeed")
        .json()
        .await
        .expect("management response should parse");
    assert_eq!(status.platforms.platform_ids, vec!["webchat".to_string()]);

    let config_current: serde_json::Value = management_client
        .get(format!("http://{address}/api/management/config/current"))
        .send()
        .await
        .expect("config request should succeed")
        .json()
        .await
        .expect("config response should parse");
    assert!(config_current["config"]["event_queue_capacity"].is_number());

    let tools: serde_json::Value = management_client
        .get(format!("http://{address}/api/management/tools"))
        .send()
        .await
        .expect("tools request should succeed")
        .json()
        .await
        .expect("tools response should parse");
    assert!(tools["tools"].is_array());

    let conversation_upsert: serde_json::Value = management_client
        .post(format!(
            "http://{address}/api/management/conversations/upsert"
        ))
        .json(&serde_json::json!({
            "platform_id": "webchat",
            "conversation_id": "conversation-cli",
            "title": "CLI conversation",
            "set_current": true
        }))
        .send()
        .await
        .expect("conversation upsert request should succeed")
        .json()
        .await
        .expect("conversation upsert response should parse");
    assert_eq!(
        conversation_upsert["conversation"]["conversation_id"],
        "conversation-cli"
    );
    assert_eq!(conversation_upsert["conversation"]["current"], true);

    let conversations: serde_json::Value = management_client
        .post(format!("http://{address}/api/management/conversations"))
        .json(&serde_json::json!({ "platform_id": "webchat" }))
        .send()
        .await
        .expect("conversation list request should succeed")
        .json()
        .await
        .expect("conversation list response should parse");
    assert!(
        conversations["conversations"]
            .as_array()
            .expect("conversations")
            .iter()
            .any(|conversation| conversation["conversation_id"] == "conversation-cli")
    );

    let commands: serde_json::Value = management_client
        .get(format!("http://{address}/api/management/commands"))
        .send()
        .await
        .expect("commands request should succeed")
        .json()
        .await
        .expect("commands response should parse");
    assert!(commands["commands"].is_array());

    let command_update: serde_json::Value = management_client
        .post(format!("http://{address}/api/management/commands/update"))
        .json(&serde_json::json!({
            "plugin_name": "cli",
            "handler_name": "ping",
            "command": "cli-ping",
            "response": "pong",
            "enabled": true,
            "permission": "admin"
        }))
        .send()
        .await
        .expect("command update request should succeed")
        .json()
        .await
        .expect("command update response should parse");
    assert_eq!(command_update["command"]["effective_command"], "cli-ping");
    assert_eq!(command_update["command"]["permission"], "admin");

    let mcp_upsert: serde_json::Value = management_client
        .post(format!(
            "http://{address}/api/management/mcp/servers/upsert"
        ))
        .json(&serde_json::json!({
            "name": "CLI Docs",
            "server": {
                "active": true,
                "transport": "stdio",
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-filesystem"]
            }
        }))
        .send()
        .await
        .expect("mcp upsert request should succeed")
        .json()
        .await
        .expect("mcp upsert response should parse");
    assert_eq!(mcp_upsert["catalog"]["active_count"], 1);

    let mcp_check: serde_json::Value = management_client
        .post(format!("http://{address}/api/management/mcp/servers/check"))
        .json(&serde_json::json!({ "name": "CLI Docs" }))
        .send()
        .await
        .expect("mcp check request should succeed")
        .json()
        .await
        .expect("mcp check response should parse");
    assert_eq!(mcp_check["ok"], true);

    let mcp_sync: serde_json::Value = management_client
        .post(format!("http://{address}/api/management/mcp/servers/sync"))
        .json(&serde_json::json!({ "names": ["CLI Docs"] }))
        .send()
        .await
        .expect("mcp sync request should succeed")
        .json()
        .await
        .expect("mcp sync response should parse");
    assert!(
        mcp_sync["bridge_tools"]
            .as_array()
            .expect("mcp bridge tools")
            .iter()
            .any(|tool| tool == "mcp_cli_docs_read_resource")
    );

    let kb: serde_json::Value = management_client
        .get(format!("http://{address}/api/management/kb/catalog"))
        .send()
        .await
        .expect("kb request should succeed")
        .json()
        .await
        .expect("kb response should parse");
    assert!(kb["knowledge_bases"].is_array());

    let capabilities: serde_json::Value = management_client
        .get(format!(
            "http://{address}/api/management/dashboard/capabilities"
        ))
        .send()
        .await
        .expect("capabilities request should succeed")
        .json()
        .await
        .expect("capabilities response should parse");
    assert!(
        capabilities["services"]
            .as_array()
            .expect("capabilities services")
            .iter()
            .any(|service| service["id"] == "plugin_market"
                && service["closure_level"] == "in_memory")
    );
    assert!(
        capabilities["services"]
            .as_array()
            .expect("capabilities services")
            .iter()
            .any(|service| service["id"] == "openapi_chat"
                && service["api_base"] == "/api/openapi/chat"
                && service["closure_level"] == "runtime")
    );
    assert!(
        capabilities["services"]
            .as_array()
            .expect("capabilities services")
            .iter()
            .any(|service| service["id"] == "providers" && service["closure_level"] == "runtime")
    );
    assert!(
        capabilities["services"]
            .as_array()
            .expect("capabilities services")
            .iter()
            .any(|service| service["id"] == "platforms" && service["closure_level"] == "runtime")
    );

    let provider_catalog: serde_json::Value = management_client
        .get(format!("http://{address}/api/management/providers/catalog"))
        .send()
        .await
        .expect("provider catalog request should succeed")
        .json()
        .await
        .expect("provider catalog response should parse");
    assert!(provider_catalog["chat_providers"].is_array());

    let provider_upsert: serde_json::Value = management_client
        .post(format!("http://{address}/api/management/providers/upsert"))
        .json(&serde_json::json!({
            "provider": {
                "id": "cli-openai",
                "type": MOCK_CHAT_PROVIDER_TYPE,
                "enabled": true,
                "mock_response": "cli provider check ok",
                "timeout_secs": 30
            },
            "set_default": true
        }))
        .send()
        .await
        .expect("provider upsert request should succeed")
        .json()
        .await
        .expect("provider upsert response should parse");
    assert_eq!(
        provider_upsert["catalog"]["default_chat_provider_id"],
        "cli-openai"
    );

    let provider_check: serde_json::Value = management_client
        .post(format!("http://{address}/api/management/providers/check"))
        .json(&serde_json::json!({ "id": "cli-openai" }))
        .send()
        .await
        .expect("provider check request should succeed")
        .json()
        .await
        .expect("provider check response should parse");
    assert_eq!(provider_check["ok"], true);

    let provider_models: serde_json::Value = management_client
        .post(format!("http://{address}/api/management/providers/models"))
        .json(&serde_json::json!({ "provider_type": OPENAI_CHAT_PROVIDER_TYPE }))
        .send()
        .await
        .expect("provider models request should succeed")
        .json()
        .await
        .expect("provider models response should parse");
    assert!(provider_models["models"].is_array());

    let platform_catalog: serde_json::Value = management_client
        .get(format!("http://{address}/api/management/platforms/catalog"))
        .send()
        .await
        .expect("platform catalog request should succeed")
        .json()
        .await
        .expect("platform catalog response should parse");
    assert!(platform_catalog["platforms"].is_array());

    let platform_upsert: serde_json::Value = management_client
        .post(format!("http://{address}/api/management/platforms/upsert"))
        .json(&serde_json::json!({
            "platform": {
                "id": "cli-console",
                "type": CONSOLE_PLATFORM_TYPE,
                "enabled": true,
                "name": "CLI Console"
            }
        }))
        .send()
        .await
        .expect("platform upsert request should succeed")
        .json()
        .await
        .expect("platform upsert response should parse");
    assert!(
        platform_upsert["catalog"]["platforms"]
            .as_array()
            .expect("platforms")
            .iter()
            .any(|platform| platform["id"] == "cli-console")
    );

    let platform_check: serde_json::Value = management_client
        .post(format!("http://{address}/api/management/platforms/check"))
        .json(&serde_json::json!({ "id": "cli-console" }))
        .send()
        .await
        .expect("platform check request should succeed")
        .json()
        .await
        .expect("platform check response should parse");
    assert_eq!(platform_check["ok"], true);

    let update: serde_json::Value = management_client
        .get(format!("http://{address}/api/management/update/check"))
        .send()
        .await
        .expect("update check request should succeed")
        .json()
        .await
        .expect("update check response should parse");
    assert_eq!(update["check"]["has_new_version"], false);

    let skills: serde_json::Value = management_client
        .get(format!("http://{address}/api/management/skills"))
        .send()
        .await
        .expect("skills request should succeed")
        .json()
        .await
        .expect("skills response should parse");
    assert!(skills["skills"].is_array());

    let market: serde_json::Value = management_client
        .get(format!("http://{address}/api/management/plugin-market"))
        .send()
        .await
        .expect("plugin market request should succeed")
        .json()
        .await
        .expect("plugin market response should parse");
    assert!(market["plugins"].is_array());

    let install: serde_json::Value = management_client
        .post(format!(
            "http://{address}/api/management/plugin-market/install"
        ))
        .json(&serde_json::json!({
            "plugin_id": "astrbot_web_tools"
        }))
        .send()
        .await
        .expect("plugin install request should succeed")
        .json()
        .await
        .expect("plugin install response should parse");
    assert_eq!(install["operation"]["action"], "install");
    assert_eq!(
        install["installed_plugins"][0]["plugin_id"],
        "astrbot_web_tools"
    );

    let update_all_plan: serde_json::Value = management_client
        .get(format!(
            "http://{address}/api/management/plugin-market/update-all-plan"
        ))
        .send()
        .await
        .expect("plugin update-all plan request should succeed")
        .json()
        .await
        .expect("plugin update-all plan response should parse");
    assert!(update_all_plan["plans"].is_array());

    let update_all: serde_json::Value = management_client
        .post(format!(
            "http://{address}/api/management/plugin-market/update-all"
        ))
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("plugin update-all request should succeed")
        .json()
        .await
        .expect("plugin update-all response should parse");
    assert!(update_all["operations"].is_array());

    let plugin_lifecycle: serde_json::Value = management_client
        .get(format!("http://{address}/api/management/plugins/lifecycle"))
        .send()
        .await
        .expect("plugin lifecycle request should succeed")
        .json()
        .await
        .expect("plugin lifecycle response should parse");
    assert!(
        plugin_lifecycle["plugins"]
            .as_array()
            .expect("plugins")
            .iter()
            .any(|plugin| plugin["plugin_id"] == "astrbot_web_tools")
    );

    let plugin_disable: serde_json::Value = management_client
        .post(format!(
            "http://{address}/api/management/plugins/lifecycle/action"
        ))
        .json(&serde_json::json!({
            "plugin_id": "astrbot_web_tools",
            "action": "disable"
        }))
        .send()
        .await
        .expect("plugin lifecycle action request should succeed")
        .json()
        .await
        .expect("plugin lifecycle action response should parse");
    assert_eq!(plugin_disable["event"]["next"], "disabled");

    let plugin_upload_plan: serde_json::Value = management_client
        .post(format!(
            "http://{address}/api/management/plugins/upload-plan"
        ))
        .json(&serde_json::json!({
            "entries": ["cli_plugin/main.py", "cli_plugin/metadata.yaml"]
        }))
        .send()
        .await
        .expect("plugin upload plan request should succeed")
        .json()
        .await
        .expect("plugin upload plan response should parse");
    assert_eq!(plugin_upload_plan["plugin_id"], "cli_plugin");

    let plugin_config: serde_json::Value = management_client
        .post(format!("http://{address}/api/management/plugins/config"))
        .json(&serde_json::json!({
            "plugin_id": "astrbot_web_tools",
            "config": { "enabled": true }
        }))
        .send()
        .await
        .expect("plugin config request should succeed")
        .json()
        .await
        .expect("plugin config response should parse");
    assert_eq!(
        plugin_config["catalog"]["plugins"][0]["plugin_id"],
        "astrbot_web_tools"
    );

    let backup: serde_json::Value = management_client
        .post(format!("http://{address}/api/management/backup/precheck"))
        .json(&serde_json::json!({
            "manifest": BackupManifest::new("0.1.0", "2026-05-17T00:00:00Z")
        }))
        .send()
        .await
        .expect("backup precheck request should succeed")
        .json()
        .await
        .expect("backup precheck response should parse");
    assert_eq!(backup["precheck"]["can_import"], true);

    let logs: serde_json::Value = management_client
        .get(format!("http://{address}/api/management/logs"))
        .send()
        .await
        .expect("logs request should succeed")
        .json()
        .await
        .expect("logs response should parse");
    assert!(logs["snapshot"]["entries"].is_array());

    let trace: serde_json::Value = management_client
        .get(format!("http://{address}/api/management/trace"))
        .send()
        .await
        .expect("trace request should succeed")
        .json()
        .await
        .expect("trace response should parse");
    assert!(trace["events"].is_array());

    let stats: serde_json::Value = management_client
        .get(format!("http://{address}/api/management/stats"))
        .send()
        .await
        .expect("stats request should succeed")
        .json()
        .await
        .expect("stats response should parse");
    assert_eq!(stats["total_messages"], 3);
    assert_eq!(stats["total_llm_calls"], 1);
    assert_eq!(stats["total_tokens"], 56);

    let personas: serde_json::Value = management_client
        .post(format!("http://{address}/api/management/personas"))
        .json(&serde_json::json!({ "folder_id": "builtin" }))
        .send()
        .await
        .expect("personas request should succeed")
        .json()
        .await
        .expect("personas response should parse");
    assert!(personas["personas"].is_array());

    let cron: serde_json::Value = management_client
        .post(format!("http://{address}/api/management/cron/jobs"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("cron request should succeed")
        .json()
        .await
        .expect("cron response should parse");
    assert!(cron["jobs"].is_array());

    let subagents: serde_json::Value = management_client
        .get(format!("http://{address}/api/management/subagents"))
        .send()
        .await
        .expect("subagents request should succeed")
        .json()
        .await
        .expect("subagents response should parse");
    assert_eq!(subagents["agents"][0]["name"], "researcher");
    assert_eq!(
        subagents["handoffs"][0]["tool_name"],
        "transfer_to_researcher"
    );

    let api_keys: serde_json::Value = management_client
        .get(format!("http://{address}/api/management/api-keys"))
        .send()
        .await
        .expect("api keys request should succeed")
        .json()
        .await
        .expect("api keys response should parse");
    assert!(api_keys["api_keys"].is_array());

    let issued_key: serde_json::Value = management_client
        .post(format!("http://{address}/api/management/api-keys/issue"))
        .json(&serde_json::json!({
            "key_id": "cli-openapi",
            "name": "CLI OpenAPI",
            "secret": "ak_cli_openapi",
            "scopes": ["openapi.chat"]
        }))
        .send()
        .await
        .expect("api key issue request should succeed")
        .json()
        .await
        .expect("api key issue response should parse");
    assert_eq!(issued_key["issued"]["key_id"], "cli-openapi");

    let openapi_chat: serde_json::Value = management_client
        .post(format!("http://{address}/api/openapi/chat"))
        .bearer_auth("ak_cli_openapi")
        .json(&serde_json::json!({
            "conversation_id": "conversation-openapi",
            "sender_id": "cli-api-user",
            "text": "hello from openapi",
            "request_id": "cli-request-1"
        }))
        .send()
        .await
        .expect("openapi chat request should succeed")
        .json()
        .await
        .expect("openapi chat response should parse");
    assert_eq!(openapi_chat["accepted"], true);
    assert_eq!(openapi_chat["conversation_id"], "conversation-openapi");

    let sent = wait_for_sent_messages(&handle, "webchat", 2).await;
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].session.conversation_id, "conversation-openapi");

    server.stop().await.expect("server should stop");
    handle.stop().await.expect("runtime should stop");
    let _ = std::fs::remove_file(config_path);
}

#[tokio::test]
async fn cli_webchat_server_wires_restart_executor_to_management_route() {
    let config_path = temp_cli_config_path("restart-executor");
    write_test_config(&config_path);
    let config = astrbot_runtime::RuntimeConfig {
        platforms: vec![RuntimePlatformConfig::webchat("webchat")],
        webchat_server: RuntimeWebChatServerConfig::enabled("webchat", "127.0.0.1", 0),
        ..astrbot_runtime::RuntimeConfig::default()
    };
    let runtime =
        astrbot_runtime::AstrbotRuntime::initialize(config.clone()).expect("runtime should init");
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let pending = prepare_webchat_server_with_config_apply(
        &runtime,
        &config.webchat_server,
        &config_path,
        None,
        Some(Arc::new(RecordingDashboardRestartExecutor { sender })),
    )
    .await
    .expect("webchat server should prepare")
    .expect("webchat server should be enabled");
    let address = pending.address;
    let handle = runtime.start();
    let server = pending.start();

    let client = reqwest::Client::new();
    let login: serde_json::Value = client
        .post(format!("http://{address}/api/auth/login"))
        .json(&serde_json::json!({
            "username": "astrbot",
            "password": "77b90590a8945a7d36c963981a307dc9"
        }))
        .send()
        .await
        .expect("login request should succeed")
        .json()
        .await
        .expect("login response should parse");
    let dashboard_token = login["data"]["token"]
        .as_str()
        .expect("dashboard token should be present");
    let restart: serde_json::Value = client
        .post(format!(
            "http://{address}/api/management/update/restart-run"
        ))
        .bearer_auth(dashboard_token)
        .json(&serde_json::json!({
            "reason": "route restart",
            "delay_secs": 0
        }))
        .send()
        .await
        .expect("restart route should respond")
        .json()
        .await
        .expect("restart response should parse");
    assert_eq!(restart["operation"]["progress"]["status"], "completed");

    let request = receiver.recv().await.expect("restart should be queued");
    assert_eq!(request.reason.as_deref(), Some("route restart"));

    server.stop().await.expect("server should stop");
    handle.stop().await.expect("runtime should stop");
    let _ = std::fs::remove_file(config_path);
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

fn temp_cli_config_path(suffix: &str) -> PathBuf {
    std::env::temp_dir().join(format!("astrbot-cli-{}-{suffix}.json", std::process::id()))
}

fn write_test_config(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
    let config = astrbot_runtime::RuntimeConfig {
        platforms: vec![RuntimePlatformConfig::webchat("webchat")],
        webchat_server: RuntimeWebChatServerConfig::enabled("webchat", "127.0.0.1", 0),
        ..astrbot_runtime::RuntimeConfig::default()
    };
    std::fs::write(
        path,
        serde_json::to_string_pretty(&config).expect("config JSON"),
    )
    .expect("test config should write");
}

#[derive(Debug)]
struct RecordingDashboardRestartExecutor {
    sender: mpsc::UnboundedSender<MaintenanceRestartRequest>,
}

impl MaintenanceRestartExecutor for RecordingDashboardRestartExecutor {
    fn restart(&self, request: &MaintenanceRestartRequest) -> std::result::Result<String, String> {
        self.sender
            .send(request.clone())
            .map_err(|error| format!("queue test restart: {error}"))?;
        Ok("test restart queued".to_string())
    }
}
