use std::sync::Arc;

use astrbot_provider::{
    GSVI_TEXT_TO_SPEECH_PROVIDER_TYPE, GsviTextToSpeechConfig, GsviTextToSpeechProvider,
    TextToSpeechProvider, TextToSpeechRequest,
};
use tokio::sync::Mutex;

mod support;
use support::http_server::serve_once;
use support::media_fixture::TempOutputDir;

#[tokio::test]
async fn sends_gsvi_tts_request_and_writes_wav_file() {
    let output_dir = TempOutputDir::new("gsvi-tts-success");
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once(
        "200 OK",
        "audio/wav",
        b"wav-audio".to_vec(),
        captured.clone(),
    )
    .await;
    let provider = GsviTextToSpeechProvider::new(
        GsviTextToSpeechConfig::new(base_url)
            .with_character("可莉")
            .with_emotion("happy")
            .with_output_dir(output_dir.path().to_path_buf()),
    )
    .expect("provider should build");

    let response = provider
        .synthesize(TextToSpeechRequest::new("hello world"))
        .await
        .expect("provider should synthesize audio");

    let audio = std::fs::read(&response.audio_path).expect("audio should be written");
    assert_eq!(audio, b"wav-audio");
    assert!(response.audio_path.ends_with(".wav"));

    let request = captured.lock().await.clone();
    assert!(request.starts_with("GET /tts?"));
    assert!(request.contains("text=hello%20world"));
    assert!(request.contains("character=%E5%8F%AF%E8%8E%89"));
    assert!(request.contains("emotion=happy"));
}

#[tokio::test]
async fn maps_gsvi_tts_error_response_to_provider_error() {
    let base_url = serve_once(
        "500 Internal Server Error",
        "text/plain",
        b"model failed".to_vec(),
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let provider = GsviTextToSpeechProvider::new(GsviTextToSpeechConfig::new(base_url))
        .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect_err("provider should map non-success status");

    let message = error.to_string();
    assert!(message.contains("500 Internal Server Error"));
    assert!(message.contains("model failed"));
}

#[tokio::test]
async fn rejects_empty_gsvi_tts_input() {
    let provider = GsviTextToSpeechProvider::new(GsviTextToSpeechConfig::new("http://127.0.0.1:1"))
        .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new(" "))
        .await
        .expect_err("empty input should fail before HTTP");

    assert!(error.to_string().contains("text-to-speech request"));
}

#[tokio::test]
async fn rejects_empty_gsvi_tts_audio() {
    let base_url = serve_once(
        "200 OK",
        "audio/wav",
        Vec::new(),
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let provider = GsviTextToSpeechProvider::new(GsviTextToSpeechConfig::new(base_url))
        .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect_err("empty audio should fail");

    assert!(error.to_string().contains("empty audio"));
}

#[test]
fn provider_type_matches_astrbot_gsvi_tts_name() {
    assert_eq!(GSVI_TEXT_TO_SPEECH_PROVIDER_TYPE, "gsvi_tts_api");
}
