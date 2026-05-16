use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use astrbot_provider::{
    AudioConversionRequest, AudioFormat, AudioInputLoader, AudioMediaConverter,
    detect_audio_conversion_requirement,
};
use async_trait::async_trait;

mod support;
use support::media_fixture::TempAudioFile;

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
        assert!(request.audio_url.ends_with(".silk"));
        assert_eq!(request.audio, b"SILK source audio");
        Ok(b"RIFF converted audio".to_vec())
    }
}
