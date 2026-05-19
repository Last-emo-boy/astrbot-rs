use crate::{
    AstrbotRuntime, RuntimeBaiduAipContentSafetyConfig, RuntimeConfig, RuntimeContentSafetyConfig,
    RuntimeKeywordContentSafetyConfig, RuntimeProviderFallbackConfig, RuntimeResultDecorateConfig,
    RuntimeSessionStatusConfig, RuntimeTextToSpeechProviderConfig, RuntimeWakeCheckConfig,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn runtime_session_status_policy_stops_disabled_sessions() {
    let config = RuntimeConfig {
        session_status: RuntimeSessionStatusConfig {
            disabled_sessions: vec!["conversation-1".to_string()],
        },
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    assert!(runtime.sent_messages().await.is_empty());
}

#[tokio::test]
async fn runtime_provider_fallback_can_be_disabled() {
    let config = RuntimeConfig {
        provider_fallback: RuntimeProviderFallbackConfig {
            enabled: false,
            ..RuntimeProviderFallbackConfig::default()
        },
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    assert!(runtime.sent_messages().await.is_empty());
    assert_eq!(runtime.provider_manager().chat_provider_count(), 1);
}

#[tokio::test]
async fn runtime_provider_wake_prefix_is_normalized_against_bot_wake_prefix() {
    let config = RuntimeConfig {
        wake_check: RuntimeWakeCheckConfig {
            wake_prefixes: vec!["/".to_string()],
            ..RuntimeWakeCheckConfig::default()
        },
        provider_fallback: RuntimeProviderFallbackConfig {
            wake_prefix: "/llm".to_string(),
            ..RuntimeProviderFallbackConfig::default()
        },
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "plain")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");
    assert!(runtime.sent_messages().await.is_empty());

    runtime
        .emit_mock_text("event-2", "conversation-1", "user-1", "llm hello")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    let sent = runtime.sent_messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "hello from astrbot-rs");
}

#[tokio::test]
async fn runtime_result_decorate_adds_reply_prefix() {
    let config = RuntimeConfig {
        result_decorate: RuntimeResultDecorateConfig {
            reply_prefix: Some("[bot] ".to_string()),
            only_llm_result: true,
            ..RuntimeResultDecorateConfig::default()
        },
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    let sent = runtime.sent_messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "[bot] hello from astrbot-rs");
}

#[tokio::test]
async fn runtime_content_safety_policy_blocks_configured_keywords() {
    let config = RuntimeConfig {
        content_safety: RuntimeContentSafetyConfig {
            rejection_message: Some("blocked".to_string()),
            internal_keywords: RuntimeKeywordContentSafetyConfig {
                enabled: true,
                extra_keywords: vec!["unsafe".to_string()],
            },
            baidu_aip: Default::default(),
        },
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "unsafe request")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    let sent = runtime.sent_messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chain.plain_text(), "blocked");
}

#[test]
fn runtime_content_safety_baidu_strategy_respects_enable_flag() {
    let disabled = RuntimeContentSafetyConfig {
        rejection_message: None,
        internal_keywords: RuntimeKeywordContentSafetyConfig {
            enabled: false,
            extra_keywords: vec!["unsafe".to_string()],
        },
        baidu_aip: RuntimeBaiduAipContentSafetyConfig {
            enabled: false,
            app_id: "app".to_string(),
            api_key: "ak".to_string(),
            secret_key: "sk".to_string(),
            token_url: Some("http://127.0.0.1/token".to_string()),
            censor_url: Some("http://127.0.0.1/censor".to_string()),
        },
    };
    let content_safety = astrbot_pipeline::ContentSafetyConfig::from(disabled);
    assert!(!content_safety.is_enabled());

    let enabled = RuntimeContentSafetyConfig {
        baidu_aip: RuntimeBaiduAipContentSafetyConfig {
            enabled: true,
            app_id: "app".to_string(),
            api_key: "ak".to_string(),
            secret_key: "sk".to_string(),
            token_url: Some("http://127.0.0.1/token".to_string()),
            censor_url: Some("http://127.0.0.1/censor".to_string()),
        },
        ..RuntimeContentSafetyConfig::default()
    };
    let content_safety = astrbot_pipeline::ContentSafetyConfig::from(enabled);
    assert!(content_safety.is_enabled());
}

#[tokio::test]
async fn runtime_result_decorate_converts_llm_reply_to_tts_record() {
    let config = RuntimeConfig {
        default_text_to_speech_provider_id: Some("tts".to_string()),
        text_to_speech_providers: vec![RuntimeTextToSpeechProviderConfig::mock("tts", "voice.wav")],
        result_decorate: RuntimeResultDecorateConfig {
            tts_enabled: true,
            ..RuntimeResultDecorateConfig::default()
        },
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    let sent = runtime.sent_messages().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(
        sent[0].chain.components(),
        &[astrbot_core::MessageComponent::record("voice.wav")]
    );
}

#[tokio::test]
async fn runtime_result_decorate_renders_long_llm_reply_to_t2i_image() {
    let root = std::env::temp_dir().join(format!("astrbot_runtime_t2i_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let config = RuntimeConfig {
        paths: crate::RuntimePathConfig::default().with_root_dir(&root),
        chat_providers: vec![crate::RuntimeChatProviderConfig::mock(
            "default-mock",
            "x".repeat(180),
        )],
        result_decorate: RuntimeResultDecorateConfig {
            t2i_enabled: true,
            t2i_strategy: "local".to_string(),
            t2i_word_threshold: 50,
            ..RuntimeResultDecorateConfig::default()
        },
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    let sent = runtime.sent_messages().await;
    assert_eq!(sent.len(), 1);
    let [astrbot_core::MessageComponent::Image { url }] = sent[0].chain.components() else {
        panic!("image component expected");
    };
    assert!(url.contains("render"));
    assert!(std::path::Path::new(url).exists());

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn runtime_result_decorate_consumes_active_template_for_t2i() {
    let root = std::env::temp_dir().join(format!(
        "astrbot_runtime_t2i_template_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let template_dir = root.join("data").join("t2i_templates");
    std::fs::create_dir_all(&template_dir).expect("template dir should be created");
    std::fs::write(template_dir.join("ops.html"), "OPS {{ text }}").expect("template should write");
    let config = RuntimeConfig {
        paths: crate::RuntimePathConfig::default().with_root_dir(&root),
        chat_providers: vec![crate::RuntimeChatProviderConfig::mock(
            "default-mock",
            "y".repeat(180),
        )],
        result_decorate: RuntimeResultDecorateConfig {
            t2i_enabled: true,
            t2i_strategy: "template".to_string(),
            t2i_active_template: "ops".to_string(),
            t2i_word_threshold: 50,
            ..RuntimeResultDecorateConfig::default()
        },
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    let sent = runtime.sent_messages().await;
    let [astrbot_core::MessageComponent::Image { url }] = sent[0].chain.components() else {
        panic!("image component expected");
    };
    let rendered = std::fs::read_to_string(url).expect("rendered template artifact should exist");
    assert!(rendered.starts_with("OPS "));
    assert!(rendered.contains(&"y".repeat(180)));

    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn runtime_result_decorate_remote_t2i_posts_endpoint_and_returns_image_url() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("test T2I server should bind");
    let address = listener.local_addr().expect("test T2I address");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("T2I request should arrive");
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let read = stream.read(&mut chunk).await.expect("request should read");
            if read == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..read]);
            if let Some(header_end) = find_header_end(&buffer) {
                let headers = String::from_utf8_lossy(&buffer[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .or_else(|| {
                        headers
                            .lines()
                            .find_map(|line| line.strip_prefix("Content-Length:"))
                    })
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                let body_start = header_end + 4;
                if buffer.len() >= body_start + content_length {
                    let path = headers
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .unwrap_or("")
                        .to_string();
                    let body = serde_json::from_slice::<serde_json::Value>(
                        &buffer[body_start..body_start + content_length],
                    )
                    .expect("T2I request body should parse");
                    assert_eq!(path, "/text2img/generate");
                    assert_eq!(body["json"], true);
                    assert!(
                        body["tmpl"]
                            .as_str()
                            .unwrap_or_default()
                            .contains("{{ text")
                    );
                    assert!(
                        body["tmpldata"]["text"]
                            .as_str()
                            .unwrap_or_default()
                            .contains(&"z".repeat(80))
                    );
                    assert_eq!(body["options"]["full_page"], true);
                    assert_eq!(body["options"]["type"], "jpeg");
                    break;
                }
            }
        }
        let response_body = "{\"data\":{\"id\":\"runtime-img\"}}";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response_body.len(),
            response_body
        );
        stream
            .write_all(response.as_bytes())
            .await
            .expect("T2I response should write");
    });

    let config = RuntimeConfig {
        chat_providers: vec![crate::RuntimeChatProviderConfig::mock(
            "default-mock",
            "z".repeat(180),
        )],
        result_decorate: RuntimeResultDecorateConfig {
            t2i_enabled: true,
            t2i_strategy: "network".to_string(),
            t2i_endpoint: Some(format!("http://{address}")),
            t2i_word_threshold: 50,
            ..RuntimeResultDecorateConfig::default()
        },
        ..RuntimeConfig::default()
    };
    let mut runtime = AstrbotRuntime::initialize(config).expect("runtime should initialize");

    runtime
        .emit_mock_text("event-1", "conversation-1", "user-1", "hello")
        .await
        .expect("event should enter runtime");
    runtime.run_once().await.expect("event should process");

    let sent = runtime.sent_messages().await;
    let [astrbot_core::MessageComponent::Image { url }] = sent[0].chain.components() else {
        panic!("image component expected");
    };
    assert_eq!(url, &format!("http://{address}/text2img/runtime-img"));
    server.await.expect("T2I server should finish");
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}
