use std::sync::Arc;

use astrbot_provider::{
    OPENAI_WHISPER_SELFHOST_SPEECH_TO_TEXT_PROVIDER_TYPE,
    SENSEVOICE_SELFHOST_SPEECH_TO_TEXT_PROVIDER_TYPE, SelfhostSpeechToTextConfig,
    SelfhostSpeechToTextKind, SelfhostSpeechToTextProvider, SpeechToTextProvider,
    SpeechToTextRequest,
};
use tokio::sync::Mutex;

mod support;
use support::captured_request::has_header;
use support::http_server::{TestResponse, serve_sequence};
use support::media_fixture::TempAudioFile;

#[tokio::test]
async fn sends_whisper_selfhost_request_and_parses_text() {
    let audio = TempAudioFile::wav("whisper-selfhost-success", b"RIFF whisper audio");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![TestResponse::json("200 OK", r#"{"text":"hello selfhost"}"#)],
        captured.clone(),
    )
    .await;
    let provider = SelfhostSpeechToTextProvider::new(
        SelfhostSpeechToTextConfig::new(SelfhostSpeechToTextKind::OpenAiWhisper, base_url, "tiny")
            .with_api_key("test-key")
            .with_header("x-selfhost", "yes"),
    )
    .expect("provider should build");

    let response = provider
        .transcribe(SpeechToTextRequest::new(audio.path_string()))
        .await
        .expect("provider should parse transcription response");

    assert_eq!(response.text, "hello selfhost");
    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /audio/transcriptions HTTP/1.1"));
    assert!(has_header(&requests[0], "authorization", "Bearer test-key"));
    assert!(has_header(&requests[0], "x-selfhost", "yes"));
    assert!(requests[0].contains("name=\"model\""));
    assert!(requests[0].contains("tiny"));
    assert!(requests[0].contains("RIFF whisper audio"));
}

#[tokio::test]
async fn sends_sensevoice_selfhost_request_with_emotion_flag() {
    let audio = TempAudioFile::wav("sensevoice-selfhost-success", b"RIFF sensevoice audio");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![TestResponse::json("200 OK", r#"{"result":"你好"}"#)],
        captured.clone(),
    )
    .await;
    let provider = SelfhostSpeechToTextProvider::new(
        SelfhostSpeechToTextConfig::new(
            SelfhostSpeechToTextKind::SenseVoice,
            base_url,
            "iic/SenseVoiceSmall",
        )
        .with_endpoint("/sensevoice/transcribe")
        .with_emotion(true)
        .with_form_field("language", "auto"),
    )
    .expect("provider should build");

    let response = provider
        .transcribe(SpeechToTextRequest::new(audio.path_string()))
        .await
        .expect("provider should parse SenseVoice response");

    assert_eq!(response.text, "你好");
    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /sensevoice/transcribe HTTP/1.1"));
    assert!(requests[0].contains("name=\"stt_model\""));
    assert!(requests[0].contains("iic/SenseVoiceSmall"));
    assert!(requests[0].contains("name=\"is_emotion\""));
    assert!(requests[0].contains("true"));
    assert!(requests[0].contains("name=\"language\""));
    assert!(requests[0].contains("auto"));
}

#[tokio::test]
async fn rejects_empty_selfhost_stt_result() {
    let audio = TempAudioFile::wav("selfhost-stt-empty", b"RIFF empty result");
    let base_url = serve_sequence(
        vec![TestResponse::json("200 OK", r#"{"text":" "}"#)],
        Arc::new(Mutex::new(Vec::new())),
    )
    .await;
    let provider = SelfhostSpeechToTextProvider::new(SelfhostSpeechToTextConfig::new(
        SelfhostSpeechToTextKind::OpenAiWhisper,
        base_url,
        "tiny",
    ))
    .expect("provider should build");

    let error = provider
        .transcribe(SpeechToTextRequest::new(audio.path_string()))
        .await
        .expect_err("empty transcription should fail");

    assert!(
        error
            .to_string()
            .contains("did not contain transcription text")
    );
}

#[tokio::test]
async fn maps_selfhost_stt_http_error() {
    let audio = TempAudioFile::wav("selfhost-stt-error", b"RIFF error audio");
    let base_url = serve_sequence(
        vec![TestResponse::json(
            "500 Internal Server Error",
            r#"{"error":{"message":"model failed"}}"#,
        )],
        Arc::new(Mutex::new(Vec::new())),
    )
    .await;
    let provider = SelfhostSpeechToTextProvider::new(SelfhostSpeechToTextConfig::new(
        SelfhostSpeechToTextKind::SenseVoice,
        base_url,
        "iic/SenseVoiceSmall",
    ))
    .expect("provider should build");

    let error = provider
        .transcribe(SpeechToTextRequest::new(audio.path_string()))
        .await
        .expect_err("HTTP error should fail");

    let message = error.to_string();
    assert!(message.contains("500 Internal Server Error"));
    assert!(message.contains("model failed"));
}

#[tokio::test]
async fn rejects_missing_selfhost_stt_audio() {
    let provider = SelfhostSpeechToTextProvider::new(SelfhostSpeechToTextConfig::new(
        SelfhostSpeechToTextKind::OpenAiWhisper,
        "http://127.0.0.1:1",
        "tiny",
    ))
    .expect("provider should build");

    let error = provider
        .transcribe(SpeechToTextRequest::new("missing-selfhost.wav"))
        .await
        .expect_err("missing local file should fail");

    assert!(error.to_string().contains("failed to read audio file"));
}

#[test]
fn provider_types_match_astrbot_selfhost_stt_names() {
    assert_eq!(
        OPENAI_WHISPER_SELFHOST_SPEECH_TO_TEXT_PROVIDER_TYPE,
        "openai_whisper_selfhost"
    );
    assert_eq!(
        SENSEVOICE_SELFHOST_SPEECH_TO_TEXT_PROVIDER_TYPE,
        "sensevoice_stt_selfhost"
    );
}
