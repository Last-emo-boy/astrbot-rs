use std::fs;

use astrbot_storage::ApiKeyRecord;

use crate::{
    LiveAudioBuffer, LiveAudioError, OpenApiChatAuthContext, OpenApiChatGateway,
    OpenApiChatGatewayError, OpenApiChatMessagePart, OpenApiChatMessageRequest,
    OpenApiChatResponseMode, RealtimeConnectionSession, RealtimeProcessingState,
    required_openapi_chat_scopes,
};

#[test]
fn realtime_session_tracks_processing_interrupts_and_subscriptions() {
    let mut session = RealtimeConnectionSession::new("ws-1", "alice");
    session.bind_conversation(" conversation-1 ");

    assert_eq!(session.session_id(), "ws-1");
    assert_eq!(session.username(), "alice");
    assert_eq!(session.conversation_id(), Some("conversation-1"));
    assert_eq!(session.processing_state(), RealtimeProcessingState::Idle);

    session.start_processing();
    assert!(session.is_processing());
    assert!(session.request_interrupt());
    assert!(session.should_interrupt());
    assert!(session.take_interrupt());
    assert!(!session.take_interrupt());

    let subscription = session
        .bind_subscription("chat-session-1", "request-1")
        .expect("subscription should bind");
    assert_eq!(subscription.request_id, "request-1");
    assert_eq!(
        session
            .subscription("chat-session-1")
            .expect("subscription should exist")
            .chat_session_id,
        "chat-session-1"
    );
    assert_eq!(session.subscriptions().count(), 1);

    session.finish_processing();
    assert!(!session.is_processing());
    assert!(!session.should_interrupt());
    assert!(session.remove_subscription("chat-session-1").is_some());
    assert_eq!(session.subscriptions().count(), 0);
}

#[test]
fn live_audio_buffer_writes_pcm_frames_as_wav_bytes() {
    let mut audio = LiveAudioBuffer::new();
    audio
        .start_speaking("stamp-1")
        .expect("speech should start");
    audio
        .push_frame("stamp-1", [1, 0, 2, 0])
        .expect("pcm frame should append");

    let wav = audio
        .finish_wav_bytes("stamp-1")
        .expect("wav bytes should finish");

    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    assert_eq!(&wav[12..16], b"fmt ");
    assert_eq!(&wav[36..40], b"data");
    assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 4);
    assert_eq!(&wav[44..], &[1, 0, 2, 0]);
    assert!(!audio.is_speaking());
}

#[test]
fn live_audio_buffer_reports_stamp_mismatch_and_cleans_temp_file() {
    let mut audio = LiveAudioBuffer::new();
    audio
        .start_speaking("stamp-1")
        .expect("speech should start");

    assert_eq!(
        audio
            .push_frame("stamp-2", [1, 0])
            .expect_err("stamp should match"),
        LiveAudioError::StampMismatch {
            expected: "stamp-1".to_string(),
            actual: "stamp-2".to_string()
        }
    );

    audio
        .push_frame("stamp-1", [1, 0])
        .expect("pcm frame should append");
    let path = std::env::temp_dir().join(format!(
        "astrbot-live-audio-{}-stamp-1.wav",
        std::process::id()
    ));
    let _ = fs::remove_file(&path);

    let wav_file = audio
        .finish_wav_to_path("stamp-1", &path)
        .expect("wav file should write");
    assert_eq!(wav_file.pcm_bytes, 2);
    assert!(path.exists());
    wav_file.cleanup().expect("wav file should cleanup");
    assert!(!path.exists());
}

#[test]
fn openapi_chat_gateway_requires_chat_scope_before_enqueue() {
    let gateway = OpenApiChatGateway::default();
    let request = OpenApiChatMessageRequest {
        conversation_id: "conversation-1".to_string(),
        sender_id: None,
        text: "hello".to_string(),
        message_parts: Vec::new(),
        request_id: None,
        stream: false,
    };
    let auth = OpenApiChatAuthContext::new("key-1", "ak_test_", ["management.read"]);

    assert_eq!(
        gateway
            .prepare_enqueue(auth, request)
            .expect_err("chat scope should be required"),
        OpenApiChatGatewayError::MissingChatScope
    );
    assert_eq!(required_openapi_chat_scopes()[0].as_str(), "chat");
}

#[test]
fn openapi_chat_gateway_builds_enqueue_plan_from_typed_parts() {
    let gateway = OpenApiChatGateway::new("fallback-user");
    let record = ApiKeyRecord::new(
        "key-1",
        "OpenAPI",
        "hash",
        "ak_test_",
        ["openapi.chat"],
        "admin",
    );
    let request = OpenApiChatMessageRequest {
        conversation_id: " conversation-1 ".to_string(),
        sender_id: Some(" user-1 ".to_string()),
        text: "hello".to_string(),
        message_parts: vec![
            OpenApiChatMessagePart::Image {
                url: " https://example.com/a.png ".to_string(),
            },
            OpenApiChatMessagePart::File {
                name: "a.txt".to_string(),
                url: "file:///tmp/a.txt".to_string(),
            },
        ],
        request_id: Some(" request-1 ".to_string()),
        stream: true,
    };

    let plan = gateway
        .prepare_enqueue(
            OpenApiChatAuthContext::from_api_key_record(&record),
            request,
        )
        .expect("chat request should prepare");

    assert_eq!(plan.request.conversation_id, "conversation-1");
    assert_eq!(plan.request.sender_id, "user-1");
    assert_eq!(plan.request.request_id.as_deref(), Some("request-1"));
    assert_eq!(
        plan.request.response_mode,
        OpenApiChatResponseMode::Streaming
    );
    assert_eq!(plan.request.message.plain_text(), "hello");
    assert_eq!(
        plan.request.message.image_urls(),
        vec!["https://example.com/a.png".to_string()]
    );
    assert_eq!(
        plan.subscription
            .expect("streaming request should subscribe")
            .request_id,
        "request-1"
    );
}
