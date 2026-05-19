use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use astrbot_provider::{
    AudioConversionCommand, AudioConversionCommandRequest, AudioConversionRequest, AudioFormat,
    AudioInputLoader, AudioMediaConverter, AudioTranscodeTarget, FfmpegAudioMediaConverter,
    detect_audio_conversion_requirement,
};
use async_trait::async_trait;

mod support;
use support::media_fixture::{TempAudioFile, TempOutputDir};

#[test]
fn detects_audio_conversion_requirements_from_headers_and_urls() {
    assert_eq!(
        detect_audio_conversion_requirement("sample.wav", b"SILK audio"),
        Some(AudioFormat::Silk)
    );
    assert_eq!(
        detect_audio_conversion_requirement("sample.wav", b"#!AMR audio"),
        Some(AudioFormat::Amr)
    );
    assert_eq!(
        detect_audio_conversion_requirement("voice.silk", b"raw audio"),
        Some(AudioFormat::Silk)
    );
    assert_eq!(
        detect_audio_conversion_requirement("voice.amr", b"raw audio"),
        Some(AudioFormat::Amr)
    );
    assert_eq!(
        detect_audio_conversion_requirement("https://multimedia.nt.qq.com.cn/audio", b"raw audio",),
        Some(AudioFormat::Silk)
    );
    assert_eq!(
        detect_audio_conversion_requirement("voice.wav", b"RIFF audio"),
        None
    );
}

#[tokio::test]
async fn audio_loader_uses_converter_for_detected_media() {
    let audio = TempAudioFile::new("audio-media-convert", "silk", b"SILK source audio");
    let calls = Arc::new(AtomicUsize::new(0));
    let loader = AudioInputLoader::new(Duration::from_secs(1))
        .expect("loader should build")
        .with_converter(Arc::new(TestConverter {
            calls: calls.clone(),
        }));

    let audio = loader
        .load(&audio.path_string(), "test provider")
        .await
        .expect("converter should provide wav bytes");

    assert_eq!(audio, b"RIFF converted audio");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn ffmpeg_converter_uses_runner_and_cleans_temp_files() {
    let temp_dir = TempOutputDir::new("audio-media-ffmpeg-success");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let converter = FfmpegAudioMediaConverter::with_runner(
        temp_dir.path(),
        Arc::new(TestCommandRunner {
            seen: seen.clone(),
            output: Some(b"RIFF converted wav".to_vec()),
            error: None,
        }),
    );

    let audio = converter
        .convert(AudioConversionRequest {
            provider_label: "test provider".to_string(),
            audio_url: "voice.amr".to_string(),
            format: AudioFormat::Amr,
            target_format: AudioTranscodeTarget::Wav,
            audio: b"#!AMR source".to_vec(),
        })
        .await
        .expect("fake ffmpeg runner should convert");

    assert_eq!(audio, b"RIFF converted wav");
    let seen = seen.lock().expect("seen should lock");
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].target_format, AudioTranscodeTarget::Wav);
    assert!(seen[0].input_path.to_string_lossy().ends_with(".amr"));
    assert!(seen[0].output_path.to_string_lossy().ends_with(".wav"));
    assert_temp_dir_empty(temp_dir.path());
}

#[tokio::test]
async fn ffmpeg_converter_reports_empty_output_and_cleans_temp_files() {
    let temp_dir = TempOutputDir::new("audio-media-ffmpeg-empty");
    let converter = FfmpegAudioMediaConverter::with_runner(
        temp_dir.path(),
        Arc::new(TestCommandRunner {
            seen: Arc::new(Mutex::new(Vec::new())),
            output: Some(Vec::new()),
            error: None,
        }),
    );

    let error = converter
        .convert(AudioConversionRequest {
            provider_label: "test provider".to_string(),
            audio_url: "voice.silk".to_string(),
            format: AudioFormat::Silk,
            target_format: AudioTranscodeTarget::Wav,
            audio: b"SILK source".to_vec(),
        })
        .await
        .expect_err("empty output should fail");

    assert!(error.to_string().contains("empty wav audio"));
    assert_temp_dir_empty(temp_dir.path());
}

#[tokio::test]
async fn ffmpeg_converter_reports_command_failure_and_cleans_temp_files() {
    let temp_dir = TempOutputDir::new("audio-media-ffmpeg-failure");
    let converter = FfmpegAudioMediaConverter::with_runner(
        temp_dir.path(),
        Arc::new(TestCommandRunner {
            seen: Arc::new(Mutex::new(Vec::new())),
            output: None,
            error: Some("command failed"),
        }),
    );

    let error = converter
        .convert(AudioConversionRequest {
            provider_label: "test provider".to_string(),
            audio_url: "voice.amr".to_string(),
            format: AudioFormat::Amr,
            target_format: AudioTranscodeTarget::Wav,
            audio: b"#!AMR source".to_vec(),
        })
        .await
        .expect_err("command failure should surface");

    assert!(error.to_string().contains("command failed"));
    assert_temp_dir_empty(temp_dir.path());
}

#[tokio::test]
async fn ffmpeg_converter_uses_configured_silk_backend() {
    let temp_dir = TempOutputDir::new("audio-media-silk-backend");
    let silk_seen = Arc::new(Mutex::new(Vec::new()));
    let converter = FfmpegAudioMediaConverter::with_runner(
        temp_dir.path(),
        Arc::new(TestCommandRunner {
            seen: Arc::new(Mutex::new(Vec::new())),
            output: Some(b"unused".to_vec()),
            error: None,
        }),
    )
    .with_silk_runner(Arc::new(TestCommandRunner {
        seen: silk_seen.clone(),
        output: Some(b"SILK converted".to_vec()),
        error: None,
    }));

    let audio = converter
        .convert(AudioConversionRequest {
            provider_label: "test provider".to_string(),
            audio_url: "voice.amr".to_string(),
            format: AudioFormat::Amr,
            target_format: AudioTranscodeTarget::Silk,
            audio: b"#!AMR source".to_vec(),
        })
        .await
        .expect("silk backend should convert");

    assert_eq!(audio, b"SILK converted");
    let seen = silk_seen.lock().expect("seen should lock");
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].target_format, AudioTranscodeTarget::Silk);
    assert!(seen[0].output_path.to_string_lossy().ends_with(".silk"));
    assert_temp_dir_empty(temp_dir.path());
}

#[tokio::test]
async fn default_audio_loader_rejects_conversion_without_converter() {
    let audio = TempAudioFile::new("audio-media-unsupported", "amr", b"#!AMR source audio");
    let loader = AudioInputLoader::new(Duration::from_secs(1)).expect("loader should build");

    let error = loader
        .load(&audio.path_string(), "test provider")
        .await
        .expect_err("conversion should require configured converter");

    let message = error.to_string();
    assert!(message.contains("test provider audio requires amr conversion"));
    assert!(message.contains("media conversion boundary"));
}

struct TestConverter {
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl AudioMediaConverter for TestConverter {
    async fn convert(&self, request: AudioConversionRequest) -> astrbot_core::Result<Vec<u8>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(request.provider_label, "test provider");
        assert_eq!(request.format, AudioFormat::Silk);
        assert_eq!(request.target_format, AudioTranscodeTarget::Wav);
        assert!(request.audio_url.ends_with(".silk"));
        assert_eq!(request.audio, b"SILK source audio");
        Ok(b"RIFF converted audio".to_vec())
    }
}

struct TestCommandRunner {
    seen: Arc<Mutex<Vec<AudioConversionCommandRequest>>>,
    output: Option<Vec<u8>>,
    error: Option<&'static str>,
}

#[async_trait]
impl AudioConversionCommand for TestCommandRunner {
    async fn run(&self, request: AudioConversionCommandRequest) -> astrbot_core::Result<()> {
        assert!(request.input_path.exists());
        self.seen
            .lock()
            .expect("seen should lock")
            .push(request.clone());
        if let Some(error) = self.error {
            return Err(astrbot_core::AstrbotError::Provider(error.to_string()));
        }
        if let Some(output) = &self.output {
            fs::write(&request.output_path, output).expect("fake output should write");
        }
        Ok(())
    }
}

fn assert_temp_dir_empty(path: &Path) {
    let entries = fs::read_dir(path)
        .expect("temp dir should exist")
        .collect::<Result<Vec<_>, _>>()
        .expect("temp dir should be readable");
    assert!(entries.is_empty(), "temp dir should be empty: {entries:?}");
}
