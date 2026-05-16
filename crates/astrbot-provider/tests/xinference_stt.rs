use std::sync::Arc;

use astrbot_provider::{
    SpeechToTextProvider, SpeechToTextRequest, XINFERENCE_SPEECH_TO_TEXT_PROVIDER_TYPE,
    XinferenceSpeechToTextConfig, XinferenceSpeechToTextProvider,
};
use tokio::sync::Mutex;

mod support;
use support::http_server::{TestResponse, serve_sequence};
use support::media_fixture::TempAudioFile;

#[tokio::test]
async fn resolves_running_audio_model_and_sends_transcription_request() {
    let audio = TempAudioFile::wav("xinference-stt-running", b"RIFF xinference audio");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![
            TestResponse::json(
                "200 OK",
                r#"{"running-whisper":{"model_name":"whisper-large-v3","model_type":"audio"}}"#,
            ),
            TestResponse::json("200 OK", r#"{"text":"hello from xinference"}"#),
        ],
        captured.clone(),
    )
    .await;
    let provider = XinferenceSpeechToTextProvider::new(
        XinferenceSpeechToTextConfig::new(base_url, "whisper-large-v3").with_api_key("test-key"),
    )
    .expect("provider should build");

    let response = provider
        .transcribe(SpeechToTextRequest::new(audio.path_string()))
        .await
        .expect("provider should parse transcription response");

    assert_eq!(response.text, "hello from xinference");
    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("GET /v1/models HTTP/1.1"));
    assert!(requests[0].contains("authorization: Bearer test-key"));
    assert!(requests[1].starts_with("POST /v1/audio/transcriptions HTTP/1.1"));
    assert!(requests[1].contains("authorization: Bearer test-key"));
    assert!(requests[1].contains("name=\"model\""));
    assert!(requests[1].contains("running-whisper"));
    assert!(requests[1].contains("filename=\"audio.wav\""));
    assert!(requests[1].contains("RIFF xinference audio"));
}

#[tokio::test]
async fn launches_audio_model_when_not_running_and_auto_launch_is_enabled() {
    let audio = TempAudioFile::wav("xinference-stt-launch", b"RIFF launch audio");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![
            TestResponse::json("200 OK", r#"{"data":[]}"#),
            TestResponse::json("200 OK", r#"{"model_uid":"launched-whisper"}"#),
            TestResponse::json("200 OK", r#"{"text":"launched transcript"}"#),
        ],
        captured.clone(),
    )
    .await;
    let provider = XinferenceSpeechToTextProvider::new(
        XinferenceSpeechToTextConfig::new(base_url, "whisper-large-v3")
            .with_launch_model_if_not_running(true),
    )
    .expect("provider should build");

    let response = provider
        .transcribe(SpeechToTextRequest::new(audio.path_string()))
        .await
        .expect("provider should launch then transcribe");

    assert_eq!(response.text, "launched transcript");
    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].starts_with("GET /v1/models HTTP/1.1"));
    assert!(requests[1].starts_with("POST /v1/models HTTP/1.1"));
    assert!(requests[1].contains(r#""model_name":"whisper-large-v3""#));
    assert!(requests[1].contains(r#""model_type":"audio""#));
    assert!(requests[2].starts_with("POST /v1/audio/transcriptions HTTP/1.1"));
    assert!(requests[2].contains("launched-whisper"));
}

#[tokio::test]
async fn downloads_http_audio_without_leaking_provider_authorization() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![
            TestResponse::bytes("200 OK", "audio/wav", b"downloaded audio".to_vec()),
            TestResponse::json(
                "200 OK",
                r#"{"running-whisper":{"model_name":"whisper-large-v3"}}"#,
            ),
            TestResponse::json("200 OK", r#"{"text":"downloaded transcript"}"#),
        ],
        captured.clone(),
    )
    .await;
    let provider = XinferenceSpeechToTextProvider::new(
        XinferenceSpeechToTextConfig::new(base_url.clone(), "whisper-large-v3")
            .with_api_key("test-key"),
    )
    .expect("provider should build");

    let response = provider
        .transcribe(SpeechToTextRequest::new(format!("{base_url}/sample.wav")))
        .await
        .expect("provider should download and transcribe audio");

    assert_eq!(response.text, "downloaded transcript");
    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].starts_with("GET /sample.wav HTTP/1.1"));
    assert!(!requests[0].contains("authorization: Bearer test-key"));
    assert!(requests[1].starts_with("GET /v1/models HTTP/1.1"));
    assert!(requests[1].contains("authorization: Bearer test-key"));
    assert!(requests[2].starts_with("POST /v1/audio/transcriptions HTTP/1.1"));
    assert!(requests[2].contains("authorization: Bearer test-key"));
    assert!(requests[2].contains("downloaded audio"));
}

#[tokio::test]
async fn returns_error_when_audio_model_is_not_running_and_auto_launch_is_disabled() {
    let audio = TempAudioFile::wav("xinference-stt-missing-model", b"RIFF missing model audio");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![TestResponse::json("200 OK", r#"{"data":[]}"#)],
        captured.clone(),
    )
    .await;
    let provider = XinferenceSpeechToTextProvider::new(XinferenceSpeechToTextConfig::new(
        base_url,
        "whisper-large-v3",
    ))
    .expect("provider should build");

    let error = provider
        .transcribe(SpeechToTextRequest::new(audio.path_string()))
        .await
        .expect_err("provider should fail when model is unavailable");

    assert!(error.to_string().contains("auto-launch is disabled"));
    assert_eq!(captured.lock().await.len(), 1);
}

#[tokio::test]
async fn maps_xinference_stt_error_response_to_provider_error() {
    let audio = TempAudioFile::wav("xinference-stt-error", b"RIFF error audio");
    let base_url = serve_sequence(
        vec![
            TestResponse::json(
                "200 OK",
                r#"{"running-whisper":{"model_name":"whisper-large-v3"}}"#,
            ),
            TestResponse::json("500 Internal Server Error", r#"{"detail":"stt failed"}"#),
        ],
        Arc::new(Mutex::new(Vec::new())),
    )
    .await;
    let provider = XinferenceSpeechToTextProvider::new(XinferenceSpeechToTextConfig::new(
        base_url,
        "whisper-large-v3",
    ))
    .expect("provider should build");

    let error = provider
        .transcribe(SpeechToTextRequest::new(audio.path_string()))
        .await
        .expect_err("provider should map non-success status");

    let message = error.to_string();
    assert!(message.contains("500 Internal Server Error"));
    assert!(message.contains("stt failed"));
}

#[tokio::test]
async fn rejects_audio_that_requires_deferred_media_conversion() {
    let audio = TempAudioFile::new("xinference-stt-silk", "silk", b"SILK audio bytes");
    let provider = XinferenceSpeechToTextProvider::new(XinferenceSpeechToTextConfig::new(
        "http://127.0.0.1:1",
        "whisper-large-v3",
    ))
    .expect("provider should build");

    let error = provider
        .transcribe(SpeechToTextRequest::new(audio.path_string()))
        .await
        .expect_err("conversion-required audio should fail before HTTP");

    let message = error.to_string();
    assert!(message.contains("Xinference STT audio requires silk conversion"));
    assert!(message.contains("media conversion boundary"));
}

#[test]
fn provider_type_matches_astrbot_xinference_stt_name() {
    assert_eq!(XINFERENCE_SPEECH_TO_TEXT_PROVIDER_TYPE, "xinference_stt");
}
