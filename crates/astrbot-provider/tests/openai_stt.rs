use std::sync::Arc;

use astrbot_provider::{
    AudioConversionRequest, AudioFormat, AudioMediaConverter, AudioTranscodeTarget,
    OPENAI_SPEECH_TO_TEXT_PROVIDER_TYPE, OpenAiSpeechToTextConfig, OpenAiSpeechToTextProvider,
    SpeechToTextProvider, SpeechToTextRequest,
};
use async_trait::async_trait;
use tokio::sync::Mutex;

mod support;
use support::http_server::{TestResponse, serve_sequence};
use support::media_fixture::TempAudioFile;

#[tokio::test]
async fn sends_openai_stt_request_for_local_file_and_parses_text() {
    let audio = TempAudioFile::wav("openai-stt-success", b"RIFF audio bytes");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![TestResponse::json("200 OK", r#"{"text":"hello world"}"#)],
        captured.clone(),
    )
    .await;
    let provider = OpenAiSpeechToTextProvider::new(
        OpenAiSpeechToTextConfig::new(base_url, "whisper-1").with_api_key("test-key"),
    )
    .expect("provider should build");

    let response = provider
        .transcribe(SpeechToTextRequest::new(audio.path_string()))
        .await
        .expect("provider should parse transcription response");

    assert_eq!(response.text, "hello world");
    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /audio/transcriptions HTTP/1.1"));
    assert!(requests[0].contains("authorization: Bearer test-key"));
    assert!(requests[0].contains("name=\"model\""));
    assert!(requests[0].contains("whisper-1"));
    assert!(requests[0].contains("name=\"file\""));
    assert!(requests[0].contains("filename=\"audio.wav\""));
    assert!(requests[0].contains("RIFF audio bytes"));
}

#[tokio::test]
async fn downloads_http_audio_without_leaking_provider_authorization() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![
            TestResponse::bytes("200 OK", "audio/wav", b"downloaded audio".to_vec()),
            TestResponse::json("200 OK", r#"{"text":"downloaded transcript"}"#),
        ],
        captured.clone(),
    )
    .await;
    let provider = OpenAiSpeechToTextProvider::new(
        OpenAiSpeechToTextConfig::new(base_url.clone(), "whisper-1").with_api_key("test-key"),
    )
    .expect("provider should build");

    let response = provider
        .transcribe(SpeechToTextRequest::new(format!("{base_url}/sample.wav")))
        .await
        .expect("provider should download and transcribe audio");

    assert_eq!(response.text, "downloaded transcript");
    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("GET /sample.wav HTTP/1.1"));
    assert!(!requests[0].contains("authorization: Bearer test-key"));
    assert!(requests[1].starts_with("POST /audio/transcriptions HTTP/1.1"));
    assert!(requests[1].contains("authorization: Bearer test-key"));
    assert!(requests[1].contains("downloaded audio"));
}

#[tokio::test]
async fn transcribes_converted_audio_when_converter_is_configured() {
    let audio = TempAudioFile::new("openai-stt-amr-converted", "amr", b"#!AMR audio bytes");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![TestResponse::json("200 OK", r#"{"text":"converted"}"#)],
        captured.clone(),
    )
    .await;
    let provider = OpenAiSpeechToTextProvider::new(
        OpenAiSpeechToTextConfig::new(base_url, "whisper-1").with_api_key("test-key"),
    )
    .expect("provider should build")
    .with_audio_converter(Arc::new(SttTestConverter));

    let response = provider
        .transcribe(SpeechToTextRequest::new(audio.path_string()))
        .await
        .expect("configured converter should allow AMR transcription");

    assert_eq!(response.text, "converted");
    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].contains("RIFF converted audio"));
}

#[tokio::test]
async fn maps_openai_stt_error_response_to_provider_error() {
    let audio = TempAudioFile::wav("openai-stt-error", b"RIFF audio bytes");
    let base_url = serve_sequence(
        vec![TestResponse::json(
            "400 Bad Request",
            r#"{"error":{"message":"unsupported audio"}}"#,
        )],
        Arc::new(Mutex::new(Vec::new())),
    )
    .await;
    let provider =
        OpenAiSpeechToTextProvider::new(OpenAiSpeechToTextConfig::new(base_url, "whisper-1"))
            .expect("provider should build");

    let error = provider
        .transcribe(SpeechToTextRequest::new(audio.path_string()))
        .await
        .expect_err("provider should map non-success status");

    let message = error.to_string();
    assert!(message.contains("400 Bad Request"));
    assert!(message.contains("unsupported audio"));
}

#[tokio::test]
async fn rejects_missing_openai_stt_audio_file() {
    let provider = OpenAiSpeechToTextProvider::new(OpenAiSpeechToTextConfig::new(
        "http://127.0.0.1:1",
        "whisper-1",
    ))
    .expect("provider should build");

    let error = provider
        .transcribe(SpeechToTextRequest::new("missing.wav"))
        .await
        .expect_err("missing local file should fail before HTTP");

    assert!(error.to_string().contains("failed to read audio file"));
}

#[tokio::test]
async fn rejects_openai_stt_audio_that_requires_media_conversion() {
    let audio = TempAudioFile::new("openai-stt-amr", "amr", b"#!AMR audio bytes");
    let provider = OpenAiSpeechToTextProvider::new(OpenAiSpeechToTextConfig::new(
        "http://127.0.0.1:1",
        "whisper-1",
    ))
    .expect("provider should build");

    let error = provider
        .transcribe(SpeechToTextRequest::new(audio.path_string()))
        .await
        .expect_err("conversion-required audio should fail before HTTP");

    let message = error.to_string();
    assert!(message.contains("OpenAI STT audio requires amr conversion"));
    assert!(message.contains("media conversion boundary"));
}

#[test]
fn provider_type_matches_astrbot_openai_whisper_name() {
    assert_eq!(OPENAI_SPEECH_TO_TEXT_PROVIDER_TYPE, "openai_whisper_api");
}

struct SttTestConverter;

#[async_trait]
impl AudioMediaConverter for SttTestConverter {
    async fn convert(&self, request: AudioConversionRequest) -> astrbot_core::Result<Vec<u8>> {
        assert_eq!(request.format, AudioFormat::Amr);
        assert_eq!(request.target_format, AudioTranscodeTarget::Wav);
        assert_eq!(request.audio, b"#!AMR audio bytes");
        Ok(b"RIFF converted audio".to_vec())
    }
}
