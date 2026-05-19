use std::sync::Arc;

use astrbot_provider::{
    GSV_SELFHOST_TEXT_TO_SPEECH_PROVIDER_TYPE, GsvSelfhostTextToSpeechConfig,
    GsvSelfhostTextToSpeechProvider, TextToSpeechProvider, TextToSpeechRequest,
};
use tokio::sync::Mutex;

mod support;
use support::captured_request::has_header;
use support::http_server::{TestResponse, serve_sequence};
use support::media_fixture::TempOutputDir;

#[tokio::test]
async fn sets_weights_sends_gsv_tts_request_and_writes_wav_file() {
    let output_dir = TempOutputDir::new("gsv-selfhost-success");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![
            TestResponse::bytes("200 OK", "text/plain", b"ok".to_vec()),
            TestResponse::bytes("200 OK", "text/plain", b"ok".to_vec()),
            TestResponse::bytes("200 OK", "audio/wav", b"wav-audio".to_vec()),
        ],
        captured.clone(),
    )
    .await;
    let provider = GsvSelfhostTextToSpeechProvider::new(
        GsvSelfhostTextToSpeechConfig::new(base_url)
            .with_gpt_weights_path("C:/models/gpt.ckpt")
            .with_sovits_weights_path("C:/models/sovits.pth")
            .with_default_param("gsv_ref_audio_path", "C:/ref.wav")
            .with_default_param("gsv_prompt_text", "参考音频")
            .with_default_param("gsv_text_lang", "zh")
            .with_default_param("gsv_streaming_mode", "false")
            .with_header("x-gsv", "yes")
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

    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 3);
    assert!(
        requests[0]
            .starts_with("GET /set_gpt_weights?weights_path=C%3A%2Fmodels%2Fgpt.ckpt HTTP/1.1")
    );
    assert!(has_header(&requests[0], "x-gsv", "yes"));
    assert!(
        requests[1].starts_with(
            "GET /set_sovits_weights?weights_path=C%3A%2Fmodels%2Fsovits.pth HTTP/1.1"
        )
    );
    assert!(requests[2].starts_with("GET /tts?"));
    assert!(requests[2].contains("prompt_text=%E5%8F%82%E8%80%83%E9%9F%B3%E9%A2%91"));
    assert!(requests[2].contains("ref_audio_path=C%3A%2Fref.wav"));
    assert!(requests[2].contains("streaming_mode=false"));
    assert!(requests[2].contains("text_lang=zh"));
    assert!(requests[2].contains("text=hello%20world"));
}

#[tokio::test]
async fn maps_gsv_tts_error_response_to_provider_error() {
    let base_url = serve_sequence(
        vec![TestResponse::bytes(
            "500 Internal Server Error",
            "text/plain",
            b"model failed".to_vec(),
        )],
        Arc::new(Mutex::new(Vec::new())),
    )
    .await;
    let provider =
        GsvSelfhostTextToSpeechProvider::new(GsvSelfhostTextToSpeechConfig::new(base_url))
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
async fn rejects_empty_gsv_tts_text() {
    let provider = GsvSelfhostTextToSpeechProvider::new(GsvSelfhostTextToSpeechConfig::new(
        "http://127.0.0.1:1",
    ))
    .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new(" "))
        .await
        .expect_err("empty text should fail before HTTP");

    assert!(error.to_string().contains("text-to-speech request"));
}

#[tokio::test]
async fn rejects_empty_gsv_tts_audio() {
    let base_url = serve_sequence(
        vec![TestResponse::bytes("200 OK", "audio/wav", Vec::new())],
        Arc::new(Mutex::new(Vec::new())),
    )
    .await;
    let provider =
        GsvSelfhostTextToSpeechProvider::new(GsvSelfhostTextToSpeechConfig::new(base_url))
            .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect_err("empty audio should fail");

    assert!(error.to_string().contains("empty audio"));
}

#[test]
fn provider_type_matches_astrbot_gsv_selfhost_name() {
    assert_eq!(
        GSV_SELFHOST_TEXT_TO_SPEECH_PROVIDER_TYPE,
        "gsv_tts_selfhost"
    );
}
