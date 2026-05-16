use std::sync::Arc;

use astrbot_provider::{
    MINIMAX_TEXT_TO_SPEECH_PROVIDER_TYPE, MiniMaxTextToSpeechConfig, MiniMaxTextToSpeechProvider,
    TextToSpeechProvider, TextToSpeechRequest,
};
use tokio::sync::Mutex;

mod support;
use support::http_server::serve_once;
use support::media_fixture::TempOutputDir;

#[tokio::test]
async fn sends_minimax_tts_request_and_writes_audio_file() {
    let output_dir = TempOutputDir::new("minimax-tts-success");
    let body = concat!(
        "data: {\"extra_info\":{\"trace_id\":\"ignored\"}}\n\n",
        "data: {\"data\":{\"audio\":\"6d70332d\"}}\n\n",
        "data: {\"data\":{\"audio\":\"617564696f\"}}\n\n"
    )
    .as_bytes()
    .to_vec();
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once("200 OK", "text/event-stream", body, captured.clone()).await;
    let provider = MiniMaxTextToSpeechProvider::new(
        MiniMaxTextToSpeechConfig::new(format!("{base_url}/v1/t2a_v2"), "speech-02-hd")
            .with_api_key("test-key")
            .with_group_id("group-1")
            .with_language_boost("Chinese")
            .with_voice_id("female-qn-qingse")
            .with_voice_speed(1.2)
            .with_voice_volume(1.1)
            .with_voice_pitch(2.0)
            .with_voice_emotion("happy")
            .with_voice_latex_read(true)
            .with_voice_english_normalization(true)
            .with_output_dir(output_dir.path().to_path_buf()),
    )
    .expect("provider should build");

    let response = provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect("provider should synthesize audio");

    let audio = std::fs::read(&response.audio_path).expect("audio should be written");
    assert_eq!(audio, b"mp3-audio");
    assert!(response.audio_path.ends_with(".mp3"));

    let request = captured.lock().await.clone();
    assert!(request.starts_with("POST /v1/t2a_v2?GroupId=group-1 HTTP/1.1"));
    assert!(request.contains("authorization: Bearer test-key"));
    assert!(request.contains("accept: application/json, text/plain, */*"));
    assert!(request.contains(r#""model":"speech-02-hd""#));
    assert!(request.contains(r#""text":"hello""#));
    assert!(request.contains(r#""stream":true"#));
    assert!(request.contains(r#""language_boost":"Chinese""#));
    assert!(request.contains(r#""voice_id":"female-qn-qingse""#));
    assert!(request.contains(r#""speed":1.2"#));
    assert!(request.contains(r#""vol":1.1"#));
    assert!(request.contains(r#""pitch":2.0"#) || request.contains(r#""pitch":2"#));
    assert!(request.contains(r#""emotion":"happy""#));
    assert!(request.contains(r#""latex_read":true"#));
    assert!(request.contains(r#""english_normalization":true"#));
    assert!(request.contains(r#""sample_rate":32000"#));
    assert!(request.contains(r#""bitrate":128000"#));
    assert!(request.contains(r#""format":"mp3""#));
    assert!(!request.contains(r#""timber_weights""#));
}

#[tokio::test]
async fn sends_minimax_timber_weights_when_enabled() {
    let output_dir = TempOutputDir::new("minimax-tts-timber");
    let body = "data: {\"data\":{\"audio\":\"6f6b\"}}\n\n"
        .as_bytes()
        .to_vec();
    let captured = Arc::new(Mutex::new(String::new()));
    let base_url = serve_once("200 OK", "text/event-stream", body, captured.clone()).await;
    let provider = MiniMaxTextToSpeechProvider::new(
        MiniMaxTextToSpeechConfig::new(format!("{base_url}/v1/t2a_v2"), "speech-02-hd")
            .with_timber_weight_enabled(true)
            .with_timber_weights(serde_json::json!([
                {"voice_id": "Chinese (Mandarin)_Warm_Girl", "weight": 1}
            ]))
            .with_voice_id("should-be-empty")
            .with_output_dir(output_dir.path().to_path_buf()),
    )
    .expect("provider should build");

    provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect("provider should synthesize audio");

    let request = captured.lock().await.clone();
    assert!(request.contains(r#""voice_id":"""#));
    assert!(
        request.contains(
            r#""timber_weights":[{"voice_id":"Chinese (Mandarin)_Warm_Girl","weight":1}]"#
        )
    );
}

#[tokio::test]
async fn maps_minimax_tts_error_response_to_provider_error() {
    let base_url = serve_once(
        "401 Unauthorized",
        "application/json",
        br#"{"message":"bad key"}"#.to_vec(),
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let provider =
        MiniMaxTextToSpeechProvider::new(MiniMaxTextToSpeechConfig::new(base_url, "speech-02-hd"))
            .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect_err("provider should map non-success status");

    let message = error.to_string();
    assert!(message.contains("401 Unauthorized"));
    assert!(message.contains("bad key"));
}

#[tokio::test]
async fn rejects_minimax_tts_stream_without_audio() {
    let base_url = serve_once(
        "200 OK",
        "text/event-stream",
        b"data: {\"extra_info\":{}}\n\n".to_vec(),
        Arc::new(Mutex::new(String::new())),
    )
    .await;
    let provider =
        MiniMaxTextToSpeechProvider::new(MiniMaxTextToSpeechConfig::new(base_url, "speech-02-hd"))
            .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect_err("empty stream should fail");

    assert!(error.to_string().contains("empty audio data"));
}

#[tokio::test]
async fn rejects_empty_minimax_tts_input() {
    let provider = MiniMaxTextToSpeechProvider::new(MiniMaxTextToSpeechConfig::new(
        "http://127.0.0.1:1",
        "speech-02-hd",
    ))
    .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new(" "))
        .await
        .expect_err("empty input should fail before HTTP");

    assert!(error.to_string().contains("text-to-speech request"));
}

#[test]
fn provider_type_matches_astrbot_minimax_tts_name() {
    assert_eq!(MINIMAX_TEXT_TO_SPEECH_PROVIDER_TYPE, "minimax_tts_api");
}
