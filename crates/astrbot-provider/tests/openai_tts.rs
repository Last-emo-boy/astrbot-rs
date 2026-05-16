use std::sync::Arc;

use astrbot_provider::{
    OPENAI_TEXT_TO_SPEECH_PROVIDER_TYPE, OpenAiTextToSpeechConfig, OpenAiTextToSpeechProvider,
    TextToSpeechProvider, TextToSpeechRequest,
};
use tokio::sync::Mutex;

mod support;
use support::http_server::serve_once;
use support::media_fixture::TempOutputDir;

#[tokio::test]
async fn sends_openai_tts_request_and_writes_audio_file() {
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once("200 OK", "audio/wav", "audio-bytes", captured.clone()).await;
    let output_dir = TempOutputDir::new("openai-tts");
    let provider = OpenAiTextToSpeechProvider::new(
        OpenAiTextToSpeechConfig::new(base_url, "tts-1")
            .with_api_key("test-key")
            .with_voice("verse")
            .with_output_dir(output_dir.path().to_path_buf()),
    )
    .expect("provider should build");

    let response = provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect("provider should synthesize audio");

    let audio = std::fs::read(&response.audio_path).expect("audio should be written");
    assert_eq!(audio, b"audio-bytes");
    assert!(response.audio_path.ends_with(".wav"));

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /audio/speech HTTP/1.1"));
    assert!(request.contains("authorization: Bearer test-key"));
    assert!(request.contains(r#""model":"tts-1""#));
    assert!(request.contains(r#""voice":"verse""#));
    assert!(request.contains(r#""input":"hello""#));
    assert!(request.contains(r#""response_format":"wav""#));
}

#[tokio::test]
async fn maps_openai_tts_error_response_to_provider_error() {
    let base_url = serve_once(
        "400 Bad Request",
        "application/json",
        r#"{"error":{"message":"bad voice"}}"#,
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let provider =
        OpenAiTextToSpeechProvider::new(OpenAiTextToSpeechConfig::new(base_url, "tts-1"))
            .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect_err("provider should map non-success status");

    let message = error.to_string();
    assert!(message.contains("400 Bad Request"));
    assert!(message.contains("bad voice"));
}

#[tokio::test]
async fn rejects_empty_openai_tts_input() {
    let provider = OpenAiTextToSpeechProvider::new(OpenAiTextToSpeechConfig::new(
        "http://127.0.0.1:1",
        "tts-1",
    ))
    .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new(" "))
        .await
        .expect_err("empty text should fail before HTTP");

    assert!(error.to_string().contains("must contain text"));
}

#[test]
fn provider_type_matches_astrbot_openai_tts_name() {
    assert_eq!(OPENAI_TEXT_TO_SPEECH_PROVIDER_TYPE, "openai_tts_api");
}
