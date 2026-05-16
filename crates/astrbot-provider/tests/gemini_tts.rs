use std::sync::Arc;

use astrbot_provider::{
    GEMINI_TEXT_TO_SPEECH_PROVIDER_TYPE, GeminiTextToSpeechConfig, GeminiTextToSpeechProvider,
    TextToSpeechProvider, TextToSpeechRequest,
};
use base64::Engine as _;
use tokio::sync::Mutex;

mod support;
use support::http_server::serve_once;
use support::media_fixture::TempOutputDir;

#[tokio::test]
async fn sends_gemini_tts_request_and_writes_wav_file() {
    let output_dir = TempOutputDir::new("gemini-tts-success");
    let pcm_audio = b"\x01\x02\x03\x04";
    let body = format!(
        r#"{{"candidates":[{{"content":{{"parts":[{{"inlineData":{{"mimeType":"audio/pcm","data":"{}"}}}}]}}}}]}}"#,
        base64::engine::general_purpose::STANDARD.encode(pcm_audio)
    );
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once("200 OK", "application/json", body, captured.clone()).await;
    let provider = GeminiTextToSpeechProvider::new(
        GeminiTextToSpeechConfig::new(base_url, "gemini-2.5-flash-preview-tts")
            .with_api_key("test-key")
            .with_voice("Kore")
            .with_prompt_prefix("Say warmly")
            .with_output_dir(output_dir.path().to_path_buf()),
    )
    .expect("provider should build");

    let response = provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect("provider should synthesize audio");

    let audio = std::fs::read(&response.audio_path).expect("audio file should be written");
    assert!(audio.starts_with(b"RIFF"));
    assert_eq!(&audio[8..12], b"WAVE");
    assert_eq!(&audio[12..16], b"fmt ");
    assert_eq!(&audio[36..40], b"data");
    assert_eq!(&audio[audio.len() - pcm_audio.len()..], pcm_audio);

    let request = captured.lock().await.clone();
    assert!(
        request.starts_with(
            "POST /v1beta/models/gemini-2.5-flash-preview-tts:generateContent HTTP/1.1"
        )
    );
    assert!(request.contains("x-goog-api-key: test-key"));
    assert!(request.contains(r#""text":"Say warmly: hello""#));
    assert!(request.contains(r#""responseModalities":["AUDIO"]"#));
    assert!(request.contains(r#""voiceName":"Kore""#));
}

#[tokio::test]
async fn maps_gemini_tts_error_response_to_provider_error() {
    let base_url = serve_once(
        "400 Bad Request",
        "application/json",
        r#"{"error":{"message":"bad voice"}}"#.to_string(),
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let provider =
        GeminiTextToSpeechProvider::new(GeminiTextToSpeechConfig::new(base_url, "gemini-tts"))
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
async fn rejects_gemini_tts_response_without_audio() {
    let base_url = serve_once(
        "200 OK",
        "application/json",
        r#"{"candidates":[{"content":{"parts":[{"text":"not audio"}]}}]}"#.to_string(),
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let provider =
        GeminiTextToSpeechProvider::new(GeminiTextToSpeechConfig::new(base_url, "gemini-tts"))
            .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect_err("response without audio should fail");

    assert!(error.to_string().contains("No audio content"));
}

#[tokio::test]
async fn rejects_empty_gemini_tts_input() {
    let provider = GeminiTextToSpeechProvider::new(GeminiTextToSpeechConfig::new(
        "http://127.0.0.1:1",
        "gemini-tts",
    ))
    .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new(" "))
        .await
        .expect_err("empty input should fail before HTTP");

    assert!(error.to_string().contains("text-to-speech request"));
}

#[test]
fn provider_type_matches_astrbot_gemini_tts_name() {
    assert_eq!(GEMINI_TEXT_TO_SPEECH_PROVIDER_TYPE, "gemini_tts");
}
