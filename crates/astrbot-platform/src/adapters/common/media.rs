use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};
use astrbot_media::MediaInput;
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlatformMediaKind {
    Image,
    Audio,
    Video,
    File,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlatformMediaSource {
    Url(String),
    Path(PathBuf),
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformMediaUpload {
    pub kind: PlatformMediaKind,
    pub source: PlatformMediaSource,
    pub filename: Option<String>,
    pub content_type: Option<String>,
}

impl PlatformMediaUpload {
    pub fn image_url(url: impl Into<String>) -> Self {
        Self {
            kind: PlatformMediaKind::Image,
            source: PlatformMediaSource::Url(url.into()),
            filename: None,
            content_type: None,
        }
    }

    pub fn file_path(path: impl Into<PathBuf>) -> Self {
        Self {
            kind: PlatformMediaKind::File,
            source: PlatformMediaSource::Path(path.into()),
            filename: None,
            content_type: None,
        }
    }

    pub fn audio_path(path: impl Into<PathBuf>) -> Self {
        Self {
            kind: PlatformMediaKind::Audio,
            source: PlatformMediaSource::Path(path.into()),
            filename: None,
            content_type: None,
        }
    }

    pub fn audio_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            kind: PlatformMediaKind::Audio,
            source: PlatformMediaSource::Bytes(bytes.into()),
            filename: None,
            content_type: None,
        }
    }

    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    pub fn to_media_input(&self) -> MediaInput {
        let mut media = match &self.source {
            PlatformMediaSource::Url(url) if url.starts_with("data:") => {
                MediaInput::data_url(url.clone())
            }
            PlatformMediaSource::Url(url) => MediaInput::url(url.clone()),
            PlatformMediaSource::Path(path) => MediaInput::file(path.clone()),
            PlatformMediaSource::Bytes(bytes) => MediaInput::bytes(bytes.clone()),
        };
        if let Some(filename) = &self.filename {
            media = media.with_filename(filename.clone());
        }
        if let Some(content_type) = &self.content_type {
            media = media.with_content_type(content_type.clone());
        }
        media
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformVoiceTargetFormat {
    Amr,
    Silk,
}

impl PlatformVoiceTargetFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Amr => "amr",
            Self::Silk => "silk",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::Amr => "audio/amr",
            Self::Silk => "audio/silk",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformVoiceConversionRequest {
    pub platform_label: String,
    pub target_format: PlatformVoiceTargetFormat,
    pub source_format: Option<String>,
    pub filename: Option<String>,
    pub audio: Vec<u8>,
}

#[async_trait]
pub trait PlatformVoiceMediaConverter: Send + Sync {
    async fn convert(&self, request: PlatformVoiceConversionRequest) -> Result<Vec<u8>>;
}

#[derive(Clone, Debug, Default)]
pub struct UnsupportedPlatformVoiceMediaConverter;

#[async_trait]
impl PlatformVoiceMediaConverter for UnsupportedPlatformVoiceMediaConverter {
    async fn convert(&self, request: PlatformVoiceConversionRequest) -> Result<Vec<u8>> {
        Err(AstrbotError::Platform(format!(
            "{} voice messages require {} conversion, but no platform voice media converter is configured",
            request.platform_label,
            request.target_format.as_str()
        )))
    }
}

#[derive(Clone)]
pub struct PlatformVoiceUploadPreparer {
    converter: Arc<dyn PlatformVoiceMediaConverter>,
}

impl Default for PlatformVoiceUploadPreparer {
    fn default() -> Self {
        Self {
            converter: Arc::new(UnsupportedPlatformVoiceMediaConverter),
        }
    }
}

impl PlatformVoiceUploadPreparer {
    pub fn new(converter: Arc<dyn PlatformVoiceMediaConverter>) -> Self {
        Self { converter }
    }

    pub async fn prepare(
        &self,
        platform_label: &str,
        upload: PlatformMediaUpload,
        target_format: PlatformVoiceTargetFormat,
    ) -> Result<PlatformMediaUpload> {
        if upload.kind != PlatformMediaKind::Audio {
            return Err(AstrbotError::Platform(
                "platform voice conversion requires an audio upload".to_string(),
            ));
        }

        let audio = read_voice_upload_bytes(&upload)?;
        if audio.is_empty() {
            return Err(AstrbotError::Platform(format!(
                "{platform_label} voice upload source was empty"
            )));
        }
        let source_format = detect_platform_voice_format(&upload, &audio);
        if source_format.as_deref() == Some(target_format.as_str()) {
            return Ok(upload);
        }

        let converted = self
            .converter
            .convert(PlatformVoiceConversionRequest {
                platform_label: platform_label.to_string(),
                target_format,
                source_format,
                filename: upload.filename.clone(),
                audio,
            })
            .await?;
        if converted.is_empty() {
            return Err(AstrbotError::Platform(format!(
                "{platform_label} voice media converter returned empty {} audio",
                target_format.as_str()
            )));
        }

        Ok(PlatformMediaUpload::audio_bytes(converted)
            .with_filename(format!("voice.{}", target_format.as_str()))
            .with_content_type(target_format.content_type()))
    }
}

pub fn detect_platform_voice_format(upload: &PlatformMediaUpload, bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(b"#!AMR") {
        return Some("amr".to_string());
    }
    if bytes.starts_with(b"SILK") {
        return Some("silk".to_string());
    }
    if bytes.starts_with(b"RIFF") {
        return Some("wav".to_string());
    }
    if bytes.starts_with(b"OggS") {
        return Some("ogg".to_string());
    }
    if bytes.starts_with(b"ID3") {
        return Some("mp3".to_string());
    }

    upload
        .content_type
        .as_deref()
        .and_then(format_from_content_type)
        .or_else(|| upload.filename.as_deref().and_then(format_from_filename))
        .or_else(|| match &upload.source {
            PlatformMediaSource::Path(path) => path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(format_from_filename),
            PlatformMediaSource::Url(url) => format_from_filename(url),
            PlatformMediaSource::Bytes(_) => None,
        })
}

fn read_voice_upload_bytes(upload: &PlatformMediaUpload) -> Result<Vec<u8>> {
    match &upload.source {
        PlatformMediaSource::Bytes(bytes) => Ok(bytes.clone()),
        PlatformMediaSource::Path(path) => fs::read(path).map_err(|err| {
            AstrbotError::Platform(format!(
                "failed to read platform voice upload {}: {err}",
                path.display()
            ))
        }),
        PlatformMediaSource::Url(_) => Err(AstrbotError::Platform(
            "platform voice conversion requires local bytes or a local path before upload"
                .to_string(),
        )),
    }
}

fn format_from_content_type(content_type: &str) -> Option<String> {
    let content_type = content_type.to_ascii_lowercase();
    match content_type.split(';').next().unwrap_or_default().trim() {
        "audio/amr" | "audio/amr-wb" => Some("amr".to_string()),
        "audio/silk" => Some("silk".to_string()),
        "audio/wav" | "audio/x-wav" | "audio/wave" => Some("wav".to_string()),
        "audio/mpeg" | "audio/mp3" => Some("mp3".to_string()),
        "audio/ogg" => Some("ogg".to_string()),
        _ => None,
    }
}

fn format_from_filename(filename: &str) -> Option<String> {
    filename
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .filter(|extension| matches!(extension.as_str(), "amr" | "silk" | "wav" | "mp3" | "ogg"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformMediaReference {
    pub kind: PlatformMediaKind,
    pub media_id: Option<String>,
    pub url: Option<String>,
    pub filename: Option<String>,
    pub content_type: Option<String>,
}

impl PlatformMediaReference {
    pub fn url(kind: PlatformMediaKind, url: impl Into<String>) -> Self {
        Self {
            kind,
            media_id: None,
            url: Some(url.into()),
            filename: None,
            content_type: None,
        }
    }

    pub fn media_id(kind: PlatformMediaKind, media_id: impl Into<String>) -> Self {
        Self {
            kind,
            media_id: Some(media_id.into()),
            url: None,
            filename: None,
            content_type: None,
        }
    }
}

#[async_trait]
pub trait PlatformMediaUploadClient: Send + Sync {
    async fn upload(&self, request: PlatformMediaUpload) -> Result<PlatformMediaReference>;
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{
        PlatformMediaKind, PlatformMediaSource, PlatformMediaUpload,
        PlatformVoiceConversionRequest, PlatformVoiceMediaConverter, PlatformVoiceTargetFormat,
        PlatformVoiceUploadPreparer, detect_platform_voice_format,
    };

    #[test]
    fn media_upload_models_platform_upload_metadata() {
        let upload = PlatformMediaUpload::image_url("https://example.test/image.png")
            .with_filename("image.png")
            .with_content_type("image/png");

        assert_eq!(upload.kind, PlatformMediaKind::Image);
        assert_eq!(
            upload.source,
            PlatformMediaSource::Url("https://example.test/image.png".to_string())
        );
        assert_eq!(upload.filename.as_deref(), Some("image.png"));
        assert_eq!(upload.content_type.as_deref(), Some("image/png"));
        let media = upload.to_media_input();
        assert_eq!(media.content_type.as_deref(), Some("image/png"));
    }

    #[test]
    fn platform_voice_format_detection_uses_headers_content_type_and_names() {
        let wav = PlatformMediaUpload::audio_bytes(b"RIFF data".to_vec());
        assert_eq!(
            detect_platform_voice_format(&wav, b"RIFF data").as_deref(),
            Some("wav")
        );

        let ogg = PlatformMediaUpload::audio_bytes(b"raw".to_vec()).with_filename("voice.ogg");
        assert_eq!(
            detect_platform_voice_format(&ogg, b"raw").as_deref(),
            Some("ogg")
        );

        let mp3 = PlatformMediaUpload::audio_bytes(b"raw".to_vec()).with_content_type("audio/mpeg");
        assert_eq!(
            detect_platform_voice_format(&mp3, b"raw").as_deref(),
            Some("mp3")
        );
    }

    #[tokio::test]
    async fn platform_voice_preparer_converts_wav_to_amr_upload() {
        let converter = std::sync::Arc::new(TestVoiceConverter {
            calls: AtomicUsize::new(0),
            output: b"#!AMR converted".to_vec(),
        });
        let preparer = PlatformVoiceUploadPreparer::new(converter.clone());
        let upload = PlatformMediaUpload::audio_bytes(b"RIFF wav".to_vec())
            .with_filename("voice.wav")
            .with_content_type("audio/wav");

        let prepared = preparer
            .prepare("WeCom", upload, PlatformVoiceTargetFormat::Amr)
            .await
            .expect("voice upload should convert");

        assert_eq!(converter.calls.load(Ordering::SeqCst), 1);
        assert_eq!(prepared.kind, PlatformMediaKind::Audio);
        assert_eq!(prepared.filename.as_deref(), Some("voice.amr"));
        assert_eq!(prepared.content_type.as_deref(), Some("audio/amr"));
        assert_eq!(
            prepared.source,
            PlatformMediaSource::Bytes(b"#!AMR converted".to_vec())
        );
    }

    #[tokio::test]
    async fn platform_voice_preparer_keeps_already_matching_format() {
        let converter = std::sync::Arc::new(TestVoiceConverter {
            calls: AtomicUsize::new(0),
            output: b"unused".to_vec(),
        });
        let preparer = PlatformVoiceUploadPreparer::new(converter.clone());
        let upload = PlatformMediaUpload::audio_bytes(b"#!AMR source".to_vec())
            .with_filename("voice.amr")
            .with_content_type("audio/amr");

        let prepared = preparer
            .prepare("WeCom", upload.clone(), PlatformVoiceTargetFormat::Amr)
            .await
            .expect("matching format should not convert");

        assert_eq!(prepared, upload);
        assert_eq!(converter.calls.load(Ordering::SeqCst), 0);
    }

    struct TestVoiceConverter {
        calls: AtomicUsize,
        output: Vec<u8>,
    }

    #[async_trait::async_trait]
    impl PlatformVoiceMediaConverter for TestVoiceConverter {
        async fn convert(
            &self,
            request: PlatformVoiceConversionRequest,
        ) -> astrbot_core::Result<Vec<u8>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert_eq!(request.platform_label, "WeCom");
            assert_eq!(request.source_format.as_deref(), Some("wav"));
            assert_eq!(request.target_format, PlatformVoiceTargetFormat::Amr);
            assert_eq!(request.audio, b"RIFF wav");
            Ok(self.output.clone())
        }
    }
}
