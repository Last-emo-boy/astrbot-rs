use std::sync::Arc;

use astrbot_provider::{
    TextToSpeechProvider, TextToSpeechRequest, VOLCENGINE_TEXT_TO_SPEECH_PROVIDER_TYPE,
    VolcengineTextToSpeechConfig, VolcengineTextToSpeechProvider,
};
use base64::Engine as _;
use tokio::sync::Mutex;

mod support;
use support::http_server::serve_once;
use support::media_fixture::TempOutputDir;

#[tokio::test]
async fn sends_volcengine_tts_request_and_writes_audio_file() {
    let output_dir = TempOutputDir::new("volcengine-tts-success");
    let audio_bytes = b"mp3-audio-bytes";
    let body = format!(
        r#"{{"data":"{}"}}"#,
        base64::engine::general_purpose::STANDARD.encode(audio_bytes)
    );
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once("200 OK", "application/json", body, captured.clone()).await;
    let provider = VolcengineTextToSpeechProvider::new(
        VolcengineTextToSpeechConfig::new(base_url)
            .with_api_key("test-key")
            .with_appid("test-appid")
            .with_cluster("volcano-icl")
            .with_voice_type("BV700_streaming")
            .with_speed_ratio(1.25)
            .with_output_dir(output_dir.path().to_path_buf()),
    )
    .expect("provider should build");

    let response = provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect("provider should synthesize audio");

    let audio = std::fs::read(&response.audio_path).expect("audio should be written");
    assert_eq!(audio, audio_bytes);
    assert!(response.audio_path.ends_with(".mp3"));

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST / HTTP/1.1"));
    assert!(request.contains("authorization: Bearer; test-key"));
    assert!(request.contains(r#""appid":"test-appid""#));
    assert!(request.contains(r#""token":"test-key""#));
    assert!(request.contains(r#""cluster":"volcano-icl""#));
    assert!(request.contains(r#""voice_type":"BV700_streaming""#));
    assert!(request.contains(r#""encoding":"mp3""#));
    assert!(request.contains(r#""speed_ratio":1.25"#));
    assert!(request.contains(r#""text":"hello""#));
    assert!(request.contains(r#""operation":"query""#));
    assert!(request.contains(r#""frontend_type":"unitTson""#));
}

#[tokio::test]
async fn maps_volcengine_tts_error_response_to_provider_error() {
    let base_url = serve_once(
        "500 Internal Server Error",
        "application/json",
        r#"{"message":"bad voice"}"#.to_string(),
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let provider = VolcengineTextToSpeechProvider::new(
        VolcengineTextToSpeechConfig::new(base_url).with_api_key("test-key"),
    )
    .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect_err("provider should map non-success status");

    let message = error.to_string();
    assert!(message.contains("500 Internal Server Error"));
    assert!(message.contains("bad voice"));
}

#[tokio::test]
async fn rejects_volcengine_tts_response_without_audio() {
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"message":"missing data"}"#.to_string(),
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let provider = VolcengineTextToSpeechProvider::new(VolcengineTextToSpeechConfig::new(base_url))
        .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect_err("response without audio should fail");

    assert!(error.to_string().contains("missing data"));
}

#[tokio::test]
async fn rejects_empty_volcengine_tts_input() {
    let provider = VolcengineTextToSpeechProvider::new(VolcengineTextToSpeechConfig::new(
        "http://127.0.0.1:1",
    ))
    .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new(" "))
        .await
        .expect_err("empty input should fail before HTTP");

    assert!(error.to_string().contains("text-to-speech request"));
}

#[test]
fn provider_type_matches_astrbot_volcengine_tts_name() {
    assert_eq!(VOLCENGINE_TEXT_TO_SPEECH_PROVIDER_TYPE, "volcengine_tts");
}
