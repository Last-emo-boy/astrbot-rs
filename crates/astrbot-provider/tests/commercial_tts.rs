use std::sync::Arc;

use astrbot_provider::{
    AZURE_TEXT_TO_SPEECH_PROVIDER_TYPE, AzureCommercialTextToSpeechProvider,
    AzureOttsTextToSpeechConfig, AzureTextToSpeechConfig, DASHSCOPE_TEXT_TO_SPEECH_PROVIDER_TYPE,
    DashscopeTextToSpeechConfig, DashscopeTextToSpeechProvider, EDGE_TEXT_TO_SPEECH_PROVIDER_TYPE,
    EdgeTextToSpeechConfig, EdgeTextToSpeechProvider, FISHAUDIO_TEXT_TO_SPEECH_PROVIDER_TYPE,
    FishAudioTextToSpeechConfig, FishAudioTextToSpeechProvider, GENIE_TEXT_TO_SPEECH_PROVIDER_TYPE,
    GenieTextToSpeechConfig, GenieTextToSpeechProvider, TextToSpeechProvider, TextToSpeechRequest,
};
use base64::Engine as _;
use tokio::sync::Mutex;

mod support;
use support::captured_request::has_header;
use support::http_server::{TestResponse, serve_sequence};
use support::media_fixture::TempOutputDir;

#[tokio::test]
async fn azure_native_gets_token_sends_ssml_and_writes_wav() {
    let output_dir = TempOutputDir::new("azure-native-tts");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![
            TestResponse::bytes("200 OK", "text/plain", b"azure-token".to_vec()),
            TestResponse::bytes("200 OK", "audio/wav", b"azure-audio".to_vec()),
        ],
        captured.clone(),
    )
    .await;
    let provider = AzureCommercialTextToSpeechProvider::from_configs(
        AzureTextToSpeechConfig::new("a".repeat(32))
            .with_region("eastus")
            .with_voice("zh-CN-XiaoxiaoNeural")
            .with_style("cheerful")
            .with_role("Girl")
            .with_rate("1.2")
            .with_volume("80")
            .with_token_url_override(format!("{base_url}/token"))
            .with_endpoint_override(format!("{base_url}/cognitiveservices/v1"))
            .with_output_dir(output_dir.path().to_path_buf()),
        None,
    )
    .expect("provider should build");

    let response = provider
        .synthesize(TextToSpeechRequest::new("hello <world>"))
        .await
        .expect("Azure TTS should synthesize audio");

    assert_eq!(
        std::fs::read(&response.audio_path).expect("audio should be written"),
        b"azure-audio"
    );
    assert!(response.audio_path.ends_with(".wav"));

    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("POST /token HTTP/1.1"));
    assert!(has_header(
        &requests[0],
        "ocp-apim-subscription-key",
        &"a".repeat(32)
    ));
    assert!(requests[1].starts_with("POST /cognitiveservices/v1 HTTP/1.1"));
    assert!(has_header(
        &requests[1],
        "authorization",
        "Bearer azure-token"
    ));
    assert!(has_header(
        &requests[1],
        "x-microsoft-outputformat",
        "riff-48khz-16bit-mono-pcm"
    ));
    assert!(requests[1].contains("zh-CN-XiaoxiaoNeural"));
    assert!(requests[1].contains("hello &lt;world&gt;"));
}

#[tokio::test]
async fn azure_otts_syncs_time_signs_form_request_and_writes_audio() {
    let output_dir = TempOutputDir::new("azure-otts-tts");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![
            TestResponse::json("200 OK", r#"{"timestamp":1710000000}"#),
            TestResponse::bytes("200 OK", "audio/wav", b"otts-audio".to_vec()),
        ],
        captured.clone(),
    )
    .await;
    let provider = AzureCommercialTextToSpeechProvider::from_configs(
        AzureTextToSpeechConfig::new("a".repeat(32)),
        Some(
            AzureOttsTextToSpeechConfig::new(
                "otts-secret",
                format!("{base_url}/otts/tts"),
                format!("{base_url}/otts/time"),
            )
            .with_voice("voice-a")
            .with_style("style-a")
            .with_role("role-a")
            .with_rate("1")
            .with_volume("90")
            .with_output_dir(output_dir.path().to_path_buf()),
        ),
    )
    .expect("provider should build");

    let response = provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect("OTTS provider should synthesize audio");

    assert_eq!(
        std::fs::read(&response.audio_path).expect("audio should be written"),
        b"otts-audio"
    );

    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("GET /otts/time HTTP/1.1"));
    assert!(requests[1].starts_with("POST /otts/tts?sign="));
    assert!(has_header(&requests[1], "uak", "AstrBot/AzureTTS"));
    assert!(requests[1].contains("text=hello"));
    assert!(requests[1].contains("voice=voice-a"));
    assert!(requests[1].contains("style=style-a"));
    assert!(requests[1].contains("role=role-a"));
}

#[test]
fn azure_rejects_missing_or_malformed_credentials() {
    let error = AzureCommercialTextToSpeechProvider::from_configs(
        AzureTextToSpeechConfig::new("bad-key"),
        None,
    )
    .expect_err("invalid subscription key should fail");
    assert!(error.to_string().contains("Azure TTS subscription key"));

    let otts = AzureCommercialTextToSpeechProvider::from_configs(
        AzureTextToSpeechConfig::new("a".repeat(32)),
        Some(AzureOttsTextToSpeechConfig::new(
            "",
            "http://example/tts",
            "http://example/time",
        )),
    )
    .expect_err("missing OTTS_SKEY should fail");
    assert!(otts.to_string().contains("OTTS_SKEY"));
}

#[tokio::test]
async fn edge_http_adapter_sends_voice_controls_and_writes_binary_audio() {
    let output_dir = TempOutputDir::new("edge-tts");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![TestResponse::bytes(
            "200 OK",
            "audio/wav",
            b"edge-audio".to_vec(),
        )],
        captured.clone(),
    )
    .await;
    let provider = EdgeTextToSpeechProvider::new(
        EdgeTextToSpeechConfig::new(base_url)
            .with_voice("zh-CN-XiaoxiaoNeural")
            .with_rate("+10%")
            .with_volume("+20%")
            .with_pitch("+5Hz")
            .with_output_dir(output_dir.path().to_path_buf()),
    )
    .expect("provider should build");

    let response = provider
        .synthesize(TextToSpeechRequest::new("hello edge"))
        .await
        .expect("Edge adapter should synthesize audio");

    assert_eq!(
        std::fs::read(&response.audio_path).expect("audio should be written"),
        b"edge-audio"
    );
    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /tts HTTP/1.1"));
    assert!(requests[0].contains(r#""voice":"zh-CN-XiaoxiaoNeural""#));
    assert!(requests[0].contains(r#""rate":"+10%""#));
    assert!(requests[0].contains(r#""volume":"+20%""#));
    assert!(requests[0].contains(r#""pitch":"+5Hz""#));
}

#[tokio::test]
async fn dashscope_qwen_decodes_base64_audio_and_reports_errors() {
    let output_dir = TempOutputDir::new("dashscope-qwen-tts");
    let audio = base64::engine::general_purpose::STANDARD.encode(b"dashscope-audio");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![TestResponse::json(
            "200 OK",
            &format!(r#"{{"output":{{"audio":{{"data":"{audio}"}}}}}}"#),
        )],
        captured.clone(),
    )
    .await;
    let provider = DashscopeTextToSpeechProvider::new(
        DashscopeTextToSpeechConfig::new(base_url, "qwen-tts-latest")
            .with_api_key("dashscope-key")
            .with_voice("Cherry")
            .with_output_dir(output_dir.path().to_path_buf()),
    )
    .expect("provider should build");

    let response = provider
        .synthesize(TextToSpeechRequest::new("hello dashscope"))
        .await
        .expect("Dashscope provider should synthesize audio");

    assert_eq!(
        std::fs::read(&response.audio_path).expect("audio should be written"),
        b"dashscope-audio"
    );
    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /qwen/tts HTTP/1.1"));
    assert!(has_header(
        &requests[0],
        "authorization",
        "Bearer dashscope-key"
    ));
    assert!(requests[0].contains(r#""model":"qwen-tts-latest""#));
    assert!(requests[0].contains(r#""voice":"Cherry""#));
    assert!(requests[0].contains(r#""text":"hello dashscope""#));

    let missing_key = DashscopeTextToSpeechProvider::new(DashscopeTextToSpeechConfig::new(
        "http://127.0.0.1:1",
        "qwen-tts-latest",
    ))
    .expect_err("Dashscope should require api_key");
    assert!(missing_key.to_string().contains("api_key"));
}

#[tokio::test]
async fn dashscope_downloads_audio_url_and_surfaces_error_body() {
    let output_dir = TempOutputDir::new("dashscope-url-tts");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![
            TestResponse::json(
                "200 OK",
                r#"{"output":{"audio":{"url":"http://127.0.0.1:1/audio.wav"}}}"#,
            ),
            TestResponse::json("400 Bad Request", r#"{"message":"bad audio url"}"#),
        ],
        captured,
    )
    .await;
    let provider = DashscopeTextToSpeechProvider::new(
        DashscopeTextToSpeechConfig::new(base_url, "qwen-tts-latest")
            .with_api_key("dashscope-key")
            .with_output_dir(output_dir.path().to_path_buf()),
    )
    .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect_err("download failure should be surfaced");

    assert!(error.to_string().contains("audio download failed"));
}

#[tokio::test]
async fn fishaudio_uses_reference_id_and_writes_audio() {
    let output_dir = TempOutputDir::new("fishaudio-ref-tts");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![TestResponse::bytes(
            "200 OK",
            "audio/wav",
            b"fish-audio".to_vec(),
        )],
        captured.clone(),
    )
    .await;
    let provider = FishAudioTextToSpeechProvider::new(
        FishAudioTextToSpeechConfig::new(format!("{base_url}/v1"))
            .with_api_key("fish-key")
            .with_reference_id("626bb6d3f3364c9cbc3aa6a67300a664")
            .with_output_dir(output_dir.path().to_path_buf()),
    )
    .expect("provider should build");

    let response = provider
        .synthesize(TextToSpeechRequest::new("hello fish"))
        .await
        .expect("FishAudio provider should synthesize audio");

    assert_eq!(
        std::fs::read(&response.audio_path).expect("audio should be written"),
        b"fish-audio"
    );
    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /v1/tts HTTP/1.1"));
    assert!(has_header(&requests[0], "authorization", "Bearer fish-key"));
    assert!(requests[0].contains(r#""format":"wav""#));
    assert!(requests[0].contains(r#""reference_id":"626bb6d3f3364c9cbc3aa6a67300a664""#));
}

#[tokio::test]
async fn fishaudio_can_lookup_character_and_maps_error_body() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![
            TestResponse::json(
                "200 OK",
                r#"{"total":1,"items":[{"title":"可莉 voice","_id":"626bb6d3f3364c9cbc3aa6a67300a664"}]}"#,
            ),
            TestResponse::json("401 Unauthorized", r#"{"message":"bad fish token"}"#),
        ],
        captured.clone(),
    )
    .await;
    let provider = FishAudioTextToSpeechProvider::new(
        FishAudioTextToSpeechConfig::new(format!("{base_url}/v1"))
            .with_api_key("fish-key")
            .with_character("可莉"),
    )
    .expect("provider should build");

    let error = provider
        .synthesize(TextToSpeechRequest::new("hello"))
        .await
        .expect_err("non-audio response should fail");

    assert!(error.to_string().contains("401 Unauthorized"));
    assert!(error.to_string().contains("bad fish token"));
    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].starts_with("GET /model?title=%E5%8F%AF%E8%8E%89&sort_by=score"));

    let missing_key = FishAudioTextToSpeechProvider::new(FishAudioTextToSpeechConfig::new(
        "http://127.0.0.1:1/v1",
    ))
    .expect_err("FishAudio should require api_key");
    assert!(missing_key.to_string().contains("api_key"));

    let bad_reference = FishAudioTextToSpeechProvider::new(
        FishAudioTextToSpeechConfig::new("http://127.0.0.1:1/v1")
            .with_api_key("fish-key")
            .with_reference_id("not-valid"),
    )
    .expect_err("invalid reference id should fail");
    assert!(bad_reference.to_string().contains("reference_id"));
}

#[tokio::test]
async fn genie_sends_character_payload_writes_audio_and_advertises_streaming() {
    let output_dir = TempOutputDir::new("genie-tts");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let base_url = serve_sequence(
        vec![TestResponse::bytes(
            "200 OK",
            "audio/wav",
            b"genie-audio".to_vec(),
        )],
        captured.clone(),
    )
    .await;
    let provider = GenieTextToSpeechProvider::new(
        GenieTextToSpeechConfig::new(base_url)
            .with_character_name("mika")
            .with_language("Japanese")
            .with_onnx_model_dir("C:/models/genie")
            .with_refer_audio_path("C:/ref.wav")
            .with_refer_text("reference text")
            .with_output_dir(output_dir.path().to_path_buf()),
    )
    .expect("provider should build");

    assert!(provider.supports_streaming());
    let response = provider
        .synthesize(TextToSpeechRequest::new("hello genie"))
        .await
        .expect("Genie provider should synthesize audio");

    assert_eq!(
        std::fs::read(&response.audio_path).expect("audio should be written"),
        b"genie-audio"
    );
    let requests = captured.lock().await.clone();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /tts HTTP/1.1"));
    assert!(requests[0].contains(r#""character_name":"mika""#));
    assert!(requests[0].contains(r#""language":"Japanese""#));
    assert!(requests[0].contains(r#""onnx_model_dir":"C:/models/genie""#));
    assert!(requests[0].contains(r#""refer_audio_path":"C:/ref.wav""#));
    assert!(requests[0].contains(r#""refer_text":"reference text""#));
}

#[test]
fn commercial_provider_type_constants_match_source_names() {
    assert_eq!(AZURE_TEXT_TO_SPEECH_PROVIDER_TYPE, "azure_tts");
    assert_eq!(EDGE_TEXT_TO_SPEECH_PROVIDER_TYPE, "edge_tts");
    assert_eq!(DASHSCOPE_TEXT_TO_SPEECH_PROVIDER_TYPE, "dashscope_tts");
    assert_eq!(FISHAUDIO_TEXT_TO_SPEECH_PROVIDER_TYPE, "fishaudio_tts_api");
    assert_eq!(GENIE_TEXT_TO_SPEECH_PROVIDER_TYPE, "genie_tts");
}
