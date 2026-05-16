use std::path::PathBuf;

use astrbot_core::Result;
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

    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }
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
    use super::{PlatformMediaKind, PlatformMediaSource, PlatformMediaUpload};

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
    }
}
