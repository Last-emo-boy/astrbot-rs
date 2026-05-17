use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};
use serde::{Deserialize, Serialize};

use crate::data_url::{DataUrl, encode_data_url};
use crate::download::{MediaDownloadPolicy, MediaDownloadRequest, MediaDownloadService};
use crate::mime::detect_image_mime_type;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MediaInputSource {
    Url(String),
    File(PathBuf),
    DataUrl(String),
    Base64 {
        data: String,
        mime_type: Option<String>,
    },
    Attachment {
        id: String,
    },
    Bytes(Vec<u8>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaInput {
    pub source: MediaInputSource,
    pub filename: Option<String>,
    pub content_type: Option<String>,
}

impl MediaInput {
    pub fn url(url: impl Into<String>) -> Self {
        Self::new(MediaInputSource::Url(url.into()))
    }

    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self::new(MediaInputSource::File(path.into()))
    }

    pub fn data_url(data_url: impl Into<String>) -> Self {
        Self::new(MediaInputSource::DataUrl(data_url.into()))
    }

    pub fn base64(data: impl Into<String>) -> Self {
        Self::new(MediaInputSource::Base64 {
            data: data.into(),
            mime_type: None,
        })
    }

    pub fn attachment(id: impl Into<String>) -> Self {
        Self::new(MediaInputSource::Attachment { id: id.into() })
    }

    pub fn bytes(bytes: Vec<u8>) -> Self {
        Self::new(MediaInputSource::Bytes(bytes))
    }

    pub fn new(source: MediaInputSource) -> Self {
        Self {
            source,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolvedMediaKind {
    Image,
    Audio,
    Video,
    File,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedMedia {
    pub kind: ResolvedMediaKind,
    pub bytes: Vec<u8>,
    pub mime_type: String,
    pub source_label: String,
    pub filename: Option<String>,
}

impl ResolvedMedia {
    pub fn as_data_url(&self) -> String {
        encode_data_url(&self.mime_type, &self.bytes)
    }

    pub fn image_data_url(&self) -> Result<DataUrl> {
        if self.kind != ResolvedMediaKind::Image {
            return Err(AstrbotError::Provider(
                "resolved media is not an image".to_string(),
            ));
        }
        DataUrl::parse_image(&self.as_data_url())
    }
}

pub struct MediaInputResolver {
    downloader: Arc<dyn MediaDownloadService>,
    download_policy: MediaDownloadPolicy,
}

impl MediaInputResolver {
    pub fn new(downloader: Arc<dyn MediaDownloadService>) -> Self {
        Self {
            downloader,
            download_policy: MediaDownloadPolicy::default(),
        }
    }

    pub fn with_download_policy(mut self, policy: MediaDownloadPolicy) -> Self {
        self.download_policy = policy;
        self
    }

    pub async fn resolve_image(&self, input: MediaInput) -> Result<ResolvedMedia> {
        let (bytes, content_type, source_label) = match &input.source {
            MediaInputSource::Url(url) if is_http_url(url) => {
                let downloaded = self
                    .downloader
                    .download(
                        MediaDownloadRequest::new(url.clone())
                            .with_policy(self.download_policy.clone()),
                    )
                    .await?;
                (downloaded.bytes, downloaded.content_type, downloaded.url)
            }
            MediaInputSource::Url(url) if url.starts_with("data:") => {
                let parsed = DataUrl::parse_image(url)?;
                (
                    parsed.decode_bytes()?,
                    Some(parsed.mime_type().to_string()),
                    "data-url".to_string(),
                )
            }
            MediaInputSource::Url(url) if url.starts_with("base64://") => {
                let raw = url.trim_start_matches("base64://");
                let parsed = DataUrl::from_base64(
                    input
                        .content_type
                        .clone()
                        .unwrap_or_else(|| "image/jpeg".to_string()),
                    raw,
                )?;
                (
                    parsed.decode_bytes()?,
                    Some(parsed.mime_type().to_string()),
                    "base64".to_string(),
                )
            }
            MediaInputSource::Url(url) => {
                let bytes = fs::read(url).map_err(|err| {
                    AstrbotError::Provider(format!("failed to read media file {url}: {err}"))
                })?;
                (bytes, input.content_type.clone(), url.clone())
            }
            MediaInputSource::File(path) => {
                let bytes = fs::read(path).map_err(|err| {
                    AstrbotError::Provider(format!(
                        "failed to read media file {}: {err}",
                        path.display()
                    ))
                })?;
                (
                    bytes,
                    input.content_type.clone(),
                    path.display().to_string(),
                )
            }
            MediaInputSource::DataUrl(value) => {
                let parsed = DataUrl::parse_image(value)?;
                (
                    parsed.decode_bytes()?,
                    Some(parsed.mime_type().to_string()),
                    "data-url".to_string(),
                )
            }
            MediaInputSource::Base64 { data, mime_type } => {
                let parsed = DataUrl::from_base64(
                    mime_type
                        .clone()
                        .or_else(|| input.content_type.clone())
                        .unwrap_or_else(|| "image/jpeg".to_string()),
                    data.clone(),
                )?;
                (
                    parsed.decode_bytes()?,
                    Some(parsed.mime_type().to_string()),
                    "base64".to_string(),
                )
            }
            MediaInputSource::Attachment { id } => {
                return Err(AstrbotError::Provider(format!(
                    "attachment {id} requires an attachment resolver before media bytes can be loaded"
                )));
            }
            MediaInputSource::Bytes(bytes) => (
                bytes.clone(),
                input.content_type.clone(),
                "bytes".to_string(),
            ),
        };

        if bytes.is_empty() {
            return Err(AstrbotError::Provider(
                "resolved media was empty".to_string(),
            ));
        }
        let mime_type = content_type
            .as_deref()
            .and_then(normalize_image_mime)
            .or_else(|| detect_image_mime_type(&bytes).map(str::to_string))
            .ok_or_else(|| AstrbotError::Provider("unsupported image media type".to_string()))?;

        Ok(ResolvedMedia {
            kind: ResolvedMediaKind::Image,
            bytes,
            mime_type,
            source_label,
            filename: input.filename,
        })
    }
}

fn normalize_image_mime(value: &str) -> Option<String> {
    let value = value
        .split(';')
        .next()
        .unwrap_or(value)
        .trim()
        .to_ascii_lowercase();
    match value.as_str() {
        "image/png" | "image/jpeg" | "image/gif" | "image/webp" => Some(value),
        _ => None,
    }
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use astrbot_core::Result;
    use async_trait::async_trait;

    use crate::download::{DownloadedMedia, MediaDownloadRequest, MediaDownloadService};
    use crate::resolver::{MediaInput, MediaInputResolver};

    #[derive(Default)]
    struct StubDownloader;

    #[async_trait]
    impl MediaDownloadService for StubDownloader {
        async fn download(&self, request: MediaDownloadRequest) -> Result<DownloadedMedia> {
            Ok(DownloadedMedia {
                url: request.url,
                content_type: Some("image/png".to_string()),
                bytes: b"\x89PNG\r\n\x1a\nimage".to_vec(),
            })
        }
    }

    #[tokio::test]
    async fn resolves_remote_image_through_download_boundary() {
        let resolver = MediaInputResolver::new(Arc::new(StubDownloader));

        let resolved = resolver
            .resolve_image(MediaInput::url("https://example.test/image.png"))
            .await
            .expect("image should resolve");

        assert_eq!(resolved.mime_type, "image/png");
        assert!(resolved.as_data_url().starts_with("data:image/png;base64,"));
    }

    #[tokio::test]
    async fn resolves_base64_url_shape_used_by_astrbot() {
        let resolver = MediaInputResolver::new(Arc::new(StubDownloader));

        let resolved = resolver
            .resolve_image(MediaInput::url("base64://iVBORw0KGgo=").with_content_type("image/png"))
            .await
            .expect("base64 image should resolve");

        assert_eq!(resolved.mime_type, "image/png");
    }
}
