use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use astrbot_core::{MessageChain, MessageComponent, MessageStream};
use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::{Router, routing::any};
use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use sha2::Sha256;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, mpsc};
use tokio::time::timeout;
use tokio_tungstenite::{accept_async, tungstenite::Message};

use astrbot_core::{MessageEvent, Result};

use crate::{
    DINGTALK_PLATFORM_TYPE, DISCORD_PLATFORM_TYPE, KOOK_PLATFORM_TYPE, LARK_PLATFORM_TYPE,
    LINE_PLATFORM_TYPE, MISSKEY_PLATFORM_TYPE, PlatformAdapter, PlatformBuildContext,
    PlatformConfig, PlatformManager, PlatformRegistry, QQ_OFFICIAL_PLATFORM_TYPE,
    QQ_OFFICIAL_WEBHOOK_PLATFORM_TYPE, SATORI_PLATFORM_TYPE, SLACK_PLATFORM_TYPE,
    TELEGRAM_PLATFORM_TYPE, WECOM_AI_BOT_PLATFORM_TYPE, WECOM_KF_PLATFORM_TYPE,
    WEIXIN_OFFICIAL_ACCOUNT_PLATFORM_TYPE,
};

#[tokio::test]
async fn telegram_webhook_converts_rich_inbound_and_sends_fake_api_requests() {
    let api = FakeApiServer::start(200).await;
    let port = free_local_port();
    let (manager, mut event_rx) = build_manager(
        PlatformConfig::new("telegram", TELEGRAM_PLATFORM_TYPE)
            .with_secret("telegram_token", "telegram-token")
            .with_option_string("telegram_api_base_url", api.base_url())
            .with_option_string("telegram_webhook_host", "127.0.0.1")
            .with_option_u16("telegram_webhook_port", port),
    );
    let adapter = manager.adapter("telegram").expect("telegram adapter");
    let runner = run_adapter(adapter);
    wait_for_tcp(port).await;

    let body = json!({
        "update_id": 1,
        "message": {
            "message_id": "tg-msg-1",
            "chat": {"id": "tg-chat-1"},
            "from": {"id": "tg-user-1", "first_name": "Ada"},
            "text": "hello telegram",
            "photo": [{"file_id": "tg-image"}],
            "voice": {"file_id": "tg-voice"},
            "document": {"file_id": "tg-file", "file_name": "ops.txt"},
            "reply_to_message": {"message_id": "tg-root"}
        }
    });
    post_json(
        port,
        "/astrbot-telegram-webhook/callback",
        body.to_string(),
        Vec::new(),
    )
    .await;

    let event = event_rx.recv().await.expect("telegram event");
    assert_eq!(event.id, "tg-msg-1");
    assert_eq!(event.sender.id, "tg-user-1");
    assert!(event.session.is_group());
    assert_eq!(event.message.plain_text(), "hello telegram");
    assert_rich_components(&event.message);

    event
        .send(outbound_chain())
        .await
        .expect("telegram outbound should call fake API");
    let calls = api.wait_for_calls(4).await;
    assert!(
        calls.iter().any(|call| call.path.ends_with("/sendMessage")),
        "telegram text endpoint should be used"
    );
    assert!(
        calls.iter().any(|call| {
            call.headers
                .iter()
                .any(|(name, value)| name == "x-telegram-token" && value == "telegram-token")
        }),
        "telegram auth header should be observable"
    );

    manager.terminate().await.expect("terminate telegram");
    join_runner(runner).await;
}

#[tokio::test]
async fn slack_webhook_verifies_signature_disables_streaming_and_maps_api_auth_error() {
    let api = FakeApiServer::start(401).await;
    let port = free_local_port();
    let signing_secret = "slack-signing";
    let (manager, mut event_rx) = build_manager(
        PlatformConfig::new("slack", SLACK_PLATFORM_TYPE)
            .with_option_string("slack_connection_mode", "webhook")
            .with_option_string("slack_api_base_url", api.base_url())
            .with_option_string("slack_webhook_host", "127.0.0.1")
            .with_option_u16("slack_webhook_port", port)
            .with_secret("bot_token", "xoxb-token")
            .with_secret("signing_secret", signing_secret),
    );
    let adapter = manager.adapter("slack").expect("slack adapter");
    let runner = run_adapter(adapter);
    wait_for_tcp(port).await;

    let body = json!({
        "event": {
            "event_id": "slack-event-1",
            "type": "message",
            "channel": "C1",
            "user": "U1",
            "text": "hello slack",
            "thread_ts": "root-ts",
            "files": [
                {"name": "image.png", "mimetype": "image/png", "url_private": "https://files/image.png"},
                {"name": "voice.ogg", "mimetype": "audio/ogg", "url_private": "https://files/voice.ogg"},
                {"name": "ops.txt", "mimetype": "text/plain", "url_private": "https://files/ops.txt"}
            ]
        }
    })
    .to_string();
    let rejected = raw_post(
        port,
        "/astrbot-slack-webhook/callback",
        body.clone(),
        vec![
            ("x-slack-request-timestamp", "1"),
            ("x-slack-signature", "bad"),
        ],
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::UNAUTHORIZED);

    post_json(
        port,
        "/astrbot-slack-webhook/callback",
        body.clone(),
        vec![
            ("x-slack-request-timestamp", "1"),
            (
                "x-slack-signature",
                &slack_signature(signing_secret, "1", &body),
            ),
        ],
    )
    .await;
    let event = event_rx.recv().await.expect("slack event");
    assert_eq!(event.sender.id, "U1");
    assert!(event.session.is_group());
    assert_eq!(event.message.plain_text(), "hello slack");
    assert_rich_components(&event.message);

    let error = event
        .send_streaming(MessageStream::from_chunk(MessageChain::plain(
            "stream chunk",
        )))
        .await
        .expect_err("fake API 401 should surface");
    assert!(error.to_string().contains("Authentication"));
    let calls = api.wait_for_calls(1).await;
    assert_eq!(calls[0].path, "/chat.postMessage");
    assert_eq!(calls[0].body["streaming_fallback"], "disabled");

    manager.terminate().await.expect("terminate slack");
    join_runner(runner).await;
}

#[tokio::test]
async fn lark_socket_converts_callback_and_keeps_streaming_enabled() {
    let api = FakeApiServer::start(200).await;
    let socket = FakeSocketServer::start(json!({
        "event_id": "lark-event-1",
        "reply_to_message_id": "lark-root",
        "sender": {"sender_id": "ou_user_1", "sender_name": "Lark User"},
        "message": {
            "message_id": "lark-msg-1",
            "chat_id": "oc_chat_1",
            "content": "{\"text\":\"hello lark\",\"image_key\":\"img-key\",\"audio_key\":\"aud-key\",\"file_key\":\"file-key\",\"file_name\":\"ops.txt\"}"
        }
    }))
    .await;
    let (manager, mut event_rx) = build_manager(
        PlatformConfig::new("lark", LARK_PLATFORM_TYPE)
            .with_option_string("app_id", "cli_xxx")
            .with_secret("app_secret", "lark-secret")
            .with_option_string("lark_socket_url", socket.url())
            .with_option_string("lark_api_base_url", api.base_url()),
    );
    let adapter = manager.adapter("lark").expect("lark adapter");
    let runner = run_adapter(adapter);

    let event = timeout(Duration::from_secs(3), event_rx.recv())
        .await
        .expect("lark event timeout")
        .expect("lark event");
    assert_eq!(event.id, "lark-event-1");
    assert_eq!(event.sender.id, "ou_user_1");
    assert_eq!(event.message.plain_text(), "hello lark");
    assert_rich_components(&event.message);

    event
        .send_streaming(MessageStream::from_chunk(MessageChain::plain(
            "stream chunk",
        )))
        .await
        .expect("lark streaming should call fake API");
    let calls = api.wait_for_calls(1).await;
    assert_eq!(calls[0].path, "/im/v1/messages");
    assert!(calls[0].body.get("streaming_fallback").is_none());

    manager.terminate().await.expect("terminate lark");
    join_runner(runner).await;
}

#[tokio::test]
async fn line_webhook_verifies_signature_and_sends_media_fallbacks() {
    let api = FakeApiServer::start(200).await;
    let port = free_local_port();
    let channel_secret = "line-secret";
    let (manager, mut event_rx) = build_manager(
        PlatformConfig::new("line", LINE_PLATFORM_TYPE)
            .with_secret("channel_access_token", "line-token")
            .with_secret("channel_secret", channel_secret)
            .with_option_string("line_api_base_url", api.base_url())
            .with_option_string("line_webhook_host", "127.0.0.1")
            .with_option_u16("line_webhook_port", port),
    );
    let adapter = manager.adapter("line").expect("line adapter");
    let runner = run_adapter(adapter);
    wait_for_tcp(port).await;

    let body = json!({
        "events": [
            {
                "replyToken": "line-reply-1",
                "reply_to_message_id": "line-root",
                "image_url": "line-image-1",
                "voice_url": "line-audio-1",
                "file_url": "line-file-1",
                "file_name": "line.txt",
                "source": {"userId": "line-user-1", "groupId": "line-group-1"},
                "message": {"id": "line-text-1", "type": "text", "text": "hello line"}
            },
            {
                "replyToken": "line-reply-2",
                "source": {"userId": "line-user-1", "groupId": "line-group-1"},
                "message": {"id": "line-image-1", "type": "image"}
            }
        ]
    })
    .to_string();
    post_json(
        port,
        "/astrbot-line-webhook/callback",
        body.clone(),
        vec![("x-line-signature", &line_signature(channel_secret, &body))],
    )
    .await;
    let event = event_rx.recv().await.expect("line event");
    assert_eq!(event.id, "line-text-1");
    assert_eq!(event.sender.id, "line-user-1");
    assert!(event.session.is_group());
    assert_eq!(event.message.plain_text(), "hello line");
    assert_rich_components(&event.message);

    event
        .send(outbound_chain())
        .await
        .expect("line outbound should call fake API");
    let calls = api.wait_for_calls(4).await;
    assert!(calls.iter().all(|call| call.path == "/v2/bot/message/push"));

    manager.terminate().await.expect("terminate line");
    join_runner(runner).await;
}

#[tokio::test]
async fn wecom_ai_bot_long_connection_converts_callback_and_sends_fake_api_requests() {
    let api = FakeApiServer::start(200).await;
    let socket = FakeSocketServer::start(json!({
        "event_id": "wecom-ai-event-1",
        "sender_id": "wecom-user-1",
        "conversation_id": "wecom-room-1",
        "text": "hello wecom ai",
        "image_url": "https://files/wecom.png",
        "voice_url": "https://files/wecom.amr",
        "file_url": "https://files/wecom.txt",
        "file_name": "wecom.txt",
        "reply_to_message_id": "wecom-root"
    }))
    .await;
    let (manager, mut event_rx) = build_manager(
        PlatformConfig::new("wecom-ai", WECOM_AI_BOT_PLATFORM_TYPE)
            .with_option_string("wecom_ai_bot_connection_mode", "long_connection")
            .with_option_u16("port", 6198)
            .with_option_string("wecom_ai_bot_socket_url", socket.url())
            .with_option_string("wecom_ai_bot_api_base_url", api.base_url())
            .with_secret("wecomaibot_ws_bot_id", "bot-id")
            .with_secret("wecomaibot_ws_secret", "bot-secret"),
    );
    let adapter = manager.adapter("wecom-ai").expect("wecom ai adapter");
    let runner = run_adapter(adapter);

    let event = timeout(Duration::from_secs(3), event_rx.recv())
        .await
        .expect("wecom ai event timeout")
        .expect("wecom ai event");
    assert_eq!(event.sender.id, "wecom-user-1");
    assert_eq!(event.message.plain_text(), "hello wecom ai");
    assert_rich_components(&event.message);

    event
        .send(outbound_chain())
        .await
        .expect("wecom ai outbound should call fake API");
    let calls = api.wait_for_calls(4).await;
    assert!(
        calls
            .iter()
            .all(|call| call.path == "/cgi-bin/webhook/send")
    );

    manager.terminate().await.expect("terminate wecom ai");
    join_runner(runner).await;
}

#[tokio::test]
async fn dingtalk_webhook_verifies_signature_and_supports_streaming_without_fallback() {
    let api = FakeApiServer::start(200).await;
    let port = free_local_port();
    let client_secret = "dingtalk-secret";
    let (manager, mut event_rx) = build_manager(
        PlatformConfig::new("dingtalk", DINGTALK_PLATFORM_TYPE)
            .with_option_string("client_id", "ding-client")
            .with_secret("client_secret", client_secret)
            .with_option_string("dingtalk_connection_mode", "webhook")
            .with_option_string("dingtalk_api_base_url", api.base_url())
            .with_option_string("dingtalk_webhook_host", "127.0.0.1")
            .with_option_u16("dingtalk_webhook_port", port),
    );
    let adapter = manager.adapter("dingtalk").expect("dingtalk adapter");
    let runner = run_adapter(adapter);
    wait_for_tcp(port).await;

    let body = json!({
        "event_id": "ding-event-1",
        "senderStaffId": "staff-1",
        "open_conversation_id": "ding-group-1",
        "text": "hello dingtalk",
        "image_url": "https://files/ding.png",
        "voice_url": "https://files/ding.amr",
        "file_url": "https://files/ding.txt",
        "file_name": "ding.txt",
        "reply_to_message_id": "ding-root"
    })
    .to_string();
    post_json(
        port,
        "/astrbot-dingtalk-webhook/callback",
        body,
        vec![
            ("x-dingtalk-timestamp", "1680000000000"),
            (
                "x-dingtalk-signature",
                &dingtalk_signature(client_secret, "1680000000000"),
            ),
        ],
    )
    .await;
    let event = event_rx.recv().await.expect("dingtalk event");
    assert_eq!(event.sender.id, "staff-1");
    assert_eq!(event.message.plain_text(), "hello dingtalk");
    assert_rich_components(&event.message);

    event
        .send_streaming(MessageStream::from_chunk(MessageChain::plain(
            "stream chunk",
        )))
        .await
        .expect("dingtalk streaming should call fake API");
    let calls = api.wait_for_calls(1).await;
    assert_eq!(calls[0].path, "/v1.0/robot/messages/send");
    assert!(calls[0].body.get("streaming_fallback").is_none());

    manager.terminate().await.expect("terminate dingtalk");
    join_runner(runner).await;
}

#[tokio::test]
async fn long_tail_webhooks_convert_inbound_and_send_fake_api_requests() {
    let cases = vec![
        LongTailCase {
            platform_id: "discord",
            platform_type: DISCORD_PLATFORM_TYPE,
            config: long_tail_config("discord", DISCORD_PLATFORM_TYPE)
                .with_option_string("discord_connection_mode", "webhook")
                .with_secret("discord_token", "discord-token"),
            api_option: "discord_api_base_url",
            host_option: "discord_webhook_host",
            port_option: "discord_webhook_port",
            path: "/astrbot-discord-webhook/callback",
            payload: json!({
                "id": "discord-msg-1",
                "channel_id": "discord-channel-1",
                "author": {"id": "discord-user-1", "username": "Discord User"},
                "content": "hello discord",
                "attachments": [{"filename": "discord.png", "url": "https://files/discord.png"}],
                "voice_url": "https://files/discord.ogg",
                "file_url": "https://files/discord.txt",
                "file_name": "discord.txt",
                "message_reference": {"message_id": "discord-root"}
            }),
            expected_endpoint: "/channels/messages",
            expected_auth_header: Some(("authorization", "Bearer discord-token")),
        },
        LongTailCase {
            platform_id: "qq-official-webhook",
            platform_type: QQ_OFFICIAL_WEBHOOK_PLATFORM_TYPE,
            config: long_tail_config("qq-official-webhook", QQ_OFFICIAL_WEBHOOK_PLATFORM_TYPE)
                .with_option_string("appid", "qq-app")
                .with_secret("secret", "qq-secret"),
            api_option: "qq_official_webhook_api_base_url",
            host_option: "qq_official_webhook_host",
            port_option: "qq_official_webhook_port",
            path: "/astrbot-qq-official-webhook/callback",
            payload: json!({
                "msg_id": "qq-webhook-msg-1",
                "group_openid": "qq-group-1",
                "author": {"id": "qq-user-1", "username": "QQ User"},
                "content": "hello qq webhook",
                "image_url": "https://files/qq.png",
                "voice_url": "https://files/qq.amr",
                "file_url": "https://files/qq.txt",
                "file_name": "qq.txt",
                "reply_to_message_id": "qq-root"
            }),
            expected_endpoint: "/webhook/messages",
            expected_auth_header: Some(("x-qq-app-id", "qq-app")),
        },
        LongTailCase {
            platform_id: "wecom-kf",
            platform_type: WECOM_KF_PLATFORM_TYPE,
            config: long_tail_config("wecom-kf", WECOM_KF_PLATFORM_TYPE)
                .with_secret("corpid", "corp-id")
                .with_secret("secret", "corp-secret"),
            api_option: "wecom_kf_api_base_url",
            host_option: "wecom_kf_webhook_host",
            port_option: "wecom_kf_webhook_port",
            path: "/astrbot-wecom-kf-webhook/callback",
            payload: json!({
                "MsgId": "wecom-kf-msg-1",
                "open_kfid": "kf-1",
                "external_userid": "external-user-1",
                "Content": "hello wecom kf",
                "image_url": "https://files/wecom-kf.png",
                "voice_url": "https://files/wecom-kf.amr",
                "file_url": "https://files/wecom-kf.txt",
                "file_name": "wecom-kf.txt",
                "reply_to_message_id": "wecom-kf-root"
            }),
            expected_endpoint: "/kf/send_msg",
            expected_auth_header: Some(("x-wecom-corp-id", "corp-id")),
        },
        LongTailCase {
            platform_id: "weixin-official-account",
            platform_type: WEIXIN_OFFICIAL_ACCOUNT_PLATFORM_TYPE,
            config: long_tail_config(
                "weixin-official-account",
                WEIXIN_OFFICIAL_ACCOUNT_PLATFORM_TYPE,
            )
            .with_option_string("appid", "wx-app")
            .with_secret("secret", "wx-secret"),
            api_option: "weixin_official_account_api_base_url",
            host_option: "weixin_official_account_webhook_host",
            port_option: "weixin_official_account_webhook_port",
            path: "/astrbot-weixin-official-account-webhook/callback",
            payload: json!({
                "MsgId": "wx-msg-1",
                "FromUserName": "wx-user-1",
                "ToUserName": "wx-bot-1",
                "Content": "hello weixin",
                "PicUrl": "https://files/wx.png",
                "MediaId": "wx-voice",
                "file_url": "https://files/wx.txt",
                "file_name": "wx.txt",
                "reply_to_message_id": "wx-root"
            }),
            expected_endpoint: "/message/custom/send",
            expected_auth_header: Some(("x-weixin-app-id", "wx-app")),
        },
    ];

    for case in cases {
        run_long_tail_webhook_case(case).await;
    }
}

#[tokio::test]
async fn long_tail_sockets_convert_inbound_disable_streaming_and_terminate() {
    let cases = vec![
        LongTailSocketCase {
            platform_id: "kook",
            platform_type: KOOK_PLATFORM_TYPE,
            config: long_tail_config("kook", KOOK_PLATFORM_TYPE)
                .with_secret("kook_bot_token", "kook-token"),
            api_option: "kook_api_base_url",
            socket_option: "kook_socket_url",
            payload: json!({
                "d": {
                    "msg_id": "kook-msg-1",
                    "channel_id": "kook-channel-1",
                    "author_id": "kook-user-1",
                    "content": "hello kook",
                    "image_url": "https://files/kook.png",
                    "voice_url": "https://files/kook.amr",
                    "file_url": "https://files/kook.txt",
                    "file_name": "kook.txt",
                    "reply_to_message_id": "kook-root"
                }
            }),
            expected_endpoint: "/message/create",
            expected_auth_header: Some(("authorization", "Bearer kook-token")),
        },
        LongTailSocketCase {
            platform_id: "misskey",
            platform_type: MISSKEY_PLATFORM_TYPE,
            config: long_tail_config("misskey", MISSKEY_PLATFORM_TYPE)
                .with_option_string("misskey_instance_url", "https://misskey.local")
                .with_secret("misskey_token", "misskey-token"),
            api_option: "misskey_api_base_url",
            socket_option: "misskey_socket_url",
            payload: json!({
                "body": {
                    "id": "misskey-note-1",
                    "user": {"id": "misskey-user-1", "username": "misskey-user"},
                    "channel_id": "misskey-channel-1",
                    "text": "hello misskey",
                    "files": [
                        {"name": "misskey.png", "url": "https://files/misskey.png"}
                    ],
                    "voice_url": "https://files/misskey.ogg",
                    "file_url": "https://files/misskey.txt",
                    "file_name": "misskey.txt",
                    "replyId": "misskey-root"
                }
            }),
            expected_endpoint: "/notes/create",
            expected_auth_header: Some(("authorization", "Bearer misskey-token")),
        },
        LongTailSocketCase {
            platform_id: "satori",
            platform_type: SATORI_PLATFORM_TYPE,
            config: long_tail_config("satori", SATORI_PLATFORM_TYPE)
                .with_secret("satori_token", "satori-token"),
            api_option: "satori_api_base_url",
            socket_option: "satori_endpoint",
            payload: json!({
                "body": {
                    "id": "satori-message-1",
                    "user": {"id": "satori-user-1", "name": "Satori User"},
                    "channel": {"id": "satori-channel-1"},
                    "message": {
                        "id": "satori-message-1",
                        "content": "<quote id=\"satori-root\"/><text content=\"hello satori\"/><img src=\"https://files/satori.png\"/><audio src=\"https://files/satori.amr\"/><file src=\"https://files/satori.txt\" title=\"satori.txt\"/>"
                    }
                }
            }),
            expected_endpoint: "/message.create",
            expected_auth_header: Some(("authorization", "Bearer satori-token")),
        },
        LongTailSocketCase {
            platform_id: "qq-official",
            platform_type: QQ_OFFICIAL_PLATFORM_TYPE,
            config: long_tail_config("qq-official", QQ_OFFICIAL_PLATFORM_TYPE)
                .with_option_string("appid", "qq-app")
                .with_secret("secret", "qq-secret"),
            api_option: "qq_official_api_base_url",
            socket_option: "qq_official_socket_url",
            payload: json!({
                "id": "qq-msg-1",
                "channel_id": "qq-channel-1",
                "author": {"id": "qq-user-1", "username": "QQ User"},
                "content": "hello qq official",
                "image_url": "https://files/qq.png",
                "voice_url": "https://files/qq.amr",
                "file_url": "https://files/qq.txt",
                "file_name": "qq.txt",
                "reply_to_message_id": "qq-root"
            }),
            expected_endpoint: "/messages",
            expected_auth_header: Some(("x-qq-app-id", "qq-app")),
        },
    ];

    for case in cases {
        run_long_tail_socket_case(case).await;
    }
}

#[test]
fn long_tail_builtin_registry_validates_required_payloads() {
    let registry = PlatformRegistry::with_builtin_platforms();
    let (event_tx, _event_rx) = mpsc::channel(1);

    for platform_type in [
        DISCORD_PLATFORM_TYPE,
        KOOK_PLATFORM_TYPE,
        MISSKEY_PLATFORM_TYPE,
        SATORI_PLATFORM_TYPE,
        QQ_OFFICIAL_PLATFORM_TYPE,
        QQ_OFFICIAL_WEBHOOK_PLATFORM_TYPE,
        WECOM_KF_PLATFORM_TYPE,
        WEIXIN_OFFICIAL_ACCOUNT_PLATFORM_TYPE,
    ] {
        assert!(
            registry.has_platform(platform_type),
            "{platform_type} should be registered"
        );
    }

    let missing_discord = match PlatformManager::from_configs(
        &registry,
        vec![PlatformConfig::new("discord", DISCORD_PLATFORM_TYPE)],
        PlatformBuildContext::new(event_tx.clone()),
    ) {
        Ok(_) => panic!("discord without token should fail"),
        Err(err) => err,
    };
    assert!(missing_discord.to_string().contains("discord_token"));

    let missing_misskey = match PlatformManager::from_configs(
        &registry,
        vec![
            PlatformConfig::new("misskey", MISSKEY_PLATFORM_TYPE)
                .with_secret("misskey_token", "token"),
        ],
        PlatformBuildContext::new(event_tx.clone()),
    ) {
        Ok(_) => panic!("misskey without instance url should fail"),
        Err(err) => err,
    };
    assert!(missing_misskey.to_string().contains("misskey_instance_url"));

    let manager = PlatformManager::from_configs(
        &registry,
        vec![
            long_tail_config("discord", DISCORD_PLATFORM_TYPE)
                .with_secret("discord_token", "discord-token"),
            long_tail_config("kook", KOOK_PLATFORM_TYPE).with_secret("kook_bot_token", "kook"),
            long_tail_config("misskey", MISSKEY_PLATFORM_TYPE)
                .with_option_string("misskey_instance_url", "https://misskey.local")
                .with_secret("misskey_token", "misskey"),
            long_tail_config("satori", SATORI_PLATFORM_TYPE)
                .with_option_string("satori_endpoint", "ws://127.0.0.1:9"),
            long_tail_config("qq", QQ_OFFICIAL_PLATFORM_TYPE)
                .with_option_string("appid", "qq-app")
                .with_secret("secret", "qq-secret"),
            long_tail_config("qq-wh", QQ_OFFICIAL_WEBHOOK_PLATFORM_TYPE)
                .with_option_string("appid", "qq-app")
                .with_secret("secret", "qq-secret"),
            long_tail_config("wecom-kf", WECOM_KF_PLATFORM_TYPE)
                .with_secret("corpid", "corp")
                .with_secret("secret", "secret"),
            long_tail_config("wx", WEIXIN_OFFICIAL_ACCOUNT_PLATFORM_TYPE)
                .with_option_string("appid", "wx-app")
                .with_secret("secret", "wx-secret"),
        ],
        PlatformBuildContext::new(event_tx),
    )
    .expect("valid long-tail platform payloads should build");

    assert_eq!(manager.platform_count(), 8);
    assert_eq!(manager.recording_sink_count(), 8);
}

#[test]
fn wave1_builtin_registry_registers_dingtalk_and_counts_recording_sinks() {
    let registry = PlatformRegistry::with_builtin_platforms();

    assert!(registry.has_platform(TELEGRAM_PLATFORM_TYPE));
    assert!(registry.has_platform(SLACK_PLATFORM_TYPE));
    assert!(registry.has_platform(LARK_PLATFORM_TYPE));
    assert!(registry.has_platform(LINE_PLATFORM_TYPE));
    assert!(registry.has_platform(WECOM_AI_BOT_PLATFORM_TYPE));
    assert!(registry.has_platform(DINGTALK_PLATFORM_TYPE));
}

fn build_manager(config: PlatformConfig) -> (PlatformManager, mpsc::Receiver<MessageEvent>) {
    let registry = PlatformRegistry::with_builtin_platforms();
    let (event_tx, event_rx) = mpsc::channel(16);
    let manager =
        PlatformManager::from_configs(&registry, vec![config], PlatformBuildContext::new(event_tx))
            .expect("wave1 platform should build");
    (manager, event_rx)
}

fn run_adapter(adapter: Arc<dyn PlatformAdapter>) -> tokio::task::JoinHandle<Result<()>> {
    tokio::spawn(async move { adapter.run().await })
}

async fn join_runner(runner: tokio::task::JoinHandle<Result<()>>) {
    timeout(Duration::from_secs(3), runner)
        .await
        .expect("adapter runner should stop")
        .expect("adapter runner should join")
        .expect("adapter runner should succeed");
}

fn outbound_chain() -> MessageChain {
    MessageChain::new(vec![
        MessageComponent::plain("out text"),
        MessageComponent::image("https://files/out.png"),
        MessageComponent::file("out.txt", "https://files/out.txt"),
        MessageComponent::record("https://files/out.amr"),
    ])
}

fn assert_rich_components(chain: &MessageChain) {
    assert!(
        chain
            .components()
            .iter()
            .any(|component| matches!(component, MessageComponent::Image { .. })),
        "inbound image should be converted"
    );
    assert!(
        chain
            .components()
            .iter()
            .any(|component| matches!(component, MessageComponent::Record { .. })),
        "inbound voice should be converted"
    );
    assert!(
        chain
            .components()
            .iter()
            .any(|component| matches!(component, MessageComponent::File { .. })),
        "inbound file should be converted"
    );
    assert!(
        chain
            .components()
            .iter()
            .any(|component| matches!(component, MessageComponent::Reply { .. })),
        "inbound reply should be converted"
    );
}

struct LongTailCase {
    platform_id: &'static str,
    platform_type: &'static str,
    config: PlatformConfig,
    api_option: &'static str,
    host_option: &'static str,
    port_option: &'static str,
    path: &'static str,
    payload: Value,
    expected_endpoint: &'static str,
    expected_auth_header: Option<(&'static str, &'static str)>,
}

struct LongTailSocketCase {
    platform_id: &'static str,
    platform_type: &'static str,
    config: PlatformConfig,
    api_option: &'static str,
    socket_option: &'static str,
    payload: Value,
    expected_endpoint: &'static str,
    expected_auth_header: Option<(&'static str, &'static str)>,
}

async fn run_long_tail_webhook_case(case: LongTailCase) {
    let api = FakeApiServer::start(200).await;
    let port = free_local_port();
    let config = case
        .config
        .with_option_string(case.api_option, api.base_url())
        .with_option_string(case.host_option, "127.0.0.1")
        .with_option_u16(case.port_option, port);
    let (manager, mut event_rx) = build_manager(config);
    let adapter = manager
        .adapter(case.platform_id)
        .unwrap_or_else(|| panic!("{} adapter", case.platform_type));
    let runner = run_adapter(adapter);
    wait_for_tcp(port).await;

    post_json(port, case.path, case.payload.to_string(), Vec::new()).await;
    let event = event_rx
        .recv()
        .await
        .unwrap_or_else(|| panic!("{} event", case.platform_type));
    assert_ne!(event.sender.id, "unknown");
    assert!(
        !event.message.plain_text().trim().is_empty(),
        "{} inbound text should be converted",
        case.platform_type
    );
    assert_rich_components(&event.message);

    event
        .send(outbound_chain())
        .await
        .unwrap_or_else(|err| panic!("{} outbound: {err}", case.platform_type));
    let calls = api.wait_for_calls(4).await;
    assert!(
        calls.iter().any(|call| call.path == case.expected_endpoint),
        "{} expected endpoint {}",
        case.platform_type,
        case.expected_endpoint
    );
    if let Some((name, value)) = case.expected_auth_header {
        assert!(
            calls.iter().any(|call| call
                .headers
                .iter()
                .any(|(header_name, header_value)| header_name == name && header_value == value)),
            "{} expected auth header {name}",
            case.platform_type
        );
    }

    manager
        .terminate()
        .await
        .unwrap_or_else(|err| panic!("terminate {}: {err}", case.platform_type));
    join_runner(runner).await;
}

async fn run_long_tail_socket_case(case: LongTailSocketCase) {
    let api = FakeApiServer::start(200).await;
    let socket = FakeSocketServer::start(case.payload).await;
    let config = case
        .config
        .with_option_string(case.api_option, api.base_url())
        .with_option_string(case.socket_option, socket.url());
    let (manager, mut event_rx) = build_manager(config);
    let adapter = manager
        .adapter(case.platform_id)
        .unwrap_or_else(|| panic!("{} adapter", case.platform_type));
    let runner = run_adapter(adapter);

    let event = timeout(Duration::from_secs(3), event_rx.recv())
        .await
        .unwrap_or_else(|_| panic!("{} event timeout", case.platform_type))
        .unwrap_or_else(|| panic!("{} event", case.platform_type));
    assert_ne!(event.sender.id, "unknown");
    assert!(
        !event.message.plain_text().trim().is_empty(),
        "{} inbound text should be converted",
        case.platform_type
    );
    assert_rich_components(&event.message);

    event
        .send_streaming(MessageStream::from_chunk(MessageChain::plain(
            "stream chunk",
        )))
        .await
        .unwrap_or_else(|err| panic!("{} streaming outbound: {err}", case.platform_type));
    let calls = api.wait_for_calls(1).await;
    assert_eq!(calls[0].path, case.expected_endpoint);
    assert_eq!(calls[0].body["streaming_fallback"], "disabled");
    if let Some((name, value)) = case.expected_auth_header {
        assert!(
            calls[0]
                .headers
                .iter()
                .any(|(header_name, header_value)| header_name == name && header_value == value),
            "{} expected auth header {name}",
            case.platform_type
        );
    }

    manager
        .terminate()
        .await
        .unwrap_or_else(|err| panic!("terminate {}: {err}", case.platform_type));
    join_runner(runner).await;
}

fn long_tail_config(id: impl Into<String>, platform_type: impl Into<String>) -> PlatformConfig {
    PlatformConfig::new(id, platform_type)
}

async fn raw_post(
    port: u16,
    path: &str,
    body: String,
    headers: Vec<(&str, &str)>,
) -> reqwest::Response {
    let client = reqwest::Client::new();
    let mut request = client
        .post(format!("http://127.0.0.1:{port}{path}"))
        .body(body)
        .header("content-type", "application/json");
    for (name, value) in headers {
        request = request.header(name, value);
    }
    request.send().await.expect("webhook post should send")
}

async fn post_json(port: u16, path: &str, body: String, headers: Vec<(&str, &str)>) {
    let response = raw_post(port, path, body, headers).await;
    assert_eq!(response.status(), StatusCode::OK);
}

fn free_local_port() -> u16 {
    let listener =
        std::net::TcpListener::bind(("127.0.0.1", 0)).expect("free port listener should bind");
    listener.local_addr().expect("local addr").port()
}

async fn wait_for_tcp(port: u16) {
    for _ in 0..64 {
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("server on port {port} did not start");
}

#[derive(Clone, Debug)]
struct ApiCall {
    path: String,
    headers: Vec<(String, String)>,
    body: Value,
}

struct FakeApiServer {
    address: SocketAddr,
    calls: Arc<Mutex<Vec<ApiCall>>>,
}

impl FakeApiServer {
    async fn start(status: u16) -> Self {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let state = FakeApiState {
            calls: calls.clone(),
            status,
        };
        let app = Router::new()
            .route("/{*path}", any(fake_api_handler))
            .with_state(state);
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("api bind");
        let address = listener.local_addr().expect("api addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Self { address, calls }
    }

    fn base_url(&self) -> String {
        format!("http://{}", self.address)
    }

    async fn wait_for_calls(&self, expected: usize) -> Vec<ApiCall> {
        for _ in 0..80 {
            let calls = self.calls.lock().await.clone();
            if calls.len() >= expected {
                return calls;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("fake API did not receive {expected} calls");
    }
}

#[derive(Clone)]
struct FakeApiState {
    calls: Arc<Mutex<Vec<ApiCall>>>,
    status: u16,
}

async fn fake_api_handler(
    State(state): State<FakeApiState>,
    request: Request<Body>,
) -> impl IntoResponse {
    let path = request.uri().path().to_string();
    let headers = request
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_string(), value.to_string()))
        })
        .collect::<Vec<_>>();
    let body = to_bytes(request.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    let body = serde_json::from_slice::<Value>(&body).unwrap_or_else(|_| Value::Null);
    state.calls.lock().await.push(ApiCall {
        path,
        headers,
        body,
    });
    let status = StatusCode::from_u16(state.status).unwrap_or(StatusCode::OK);
    (status, "{\"ok\":true}")
}

struct FakeSocketServer {
    address: SocketAddr,
}

impl FakeSocketServer {
    async fn start(payload: Value) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("ws bind");
        let address = listener.local_addr().expect("ws addr");
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("ws accept");
            let mut socket = accept_async(stream).await.expect("ws accept async");
            socket
                .send(Message::Text(payload.to_string().into()))
                .await
                .expect("send ws payload");
            while let Some(message) = socket.next().await {
                match message.expect("ws message") {
                    Message::Ping(payload) => {
                        socket.send(Message::Pong(payload)).await.expect("pong");
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        });
        Self { address }
    }

    fn url(&self) -> String {
        format!("ws://{}", self.address)
    }
}

fn line_signature(secret: &str, body: &str) -> String {
    hmac_sha256_base64(secret, body)
}

fn slack_signature(secret: &str, timestamp: &str, body: &str) -> String {
    let payload = format!("v0:{timestamp}:{body}");
    format!("v0={}", hmac_sha256_hex(secret, &payload))
}

fn dingtalk_signature(secret: &str, timestamp: &str) -> String {
    hmac_sha256_base64(secret, &format!("{timestamp}\n{secret}"))
}

#[allow(dead_code)]
fn wecom_signature(token: &str, timestamp: &str, nonce: &str, body: &str) -> String {
    let mut fields = [token, timestamp, nonce, body];
    fields.sort_unstable();
    let mut hasher = Sha1::new();
    for field in fields {
        hasher.update(field.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn hmac_sha256_base64(secret: &str, payload: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("hmac");
    mac.update(payload.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

fn hmac_sha256_hex(secret: &str, payload: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("hmac");
    mac.update(payload.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
