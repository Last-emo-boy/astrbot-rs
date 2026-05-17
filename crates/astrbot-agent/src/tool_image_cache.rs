use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolImageData {
    pub base64_data: String,
    pub mime_type: String,
}

impl ToolImageData {
    pub fn new(base64_data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self {
            base64_data: base64_data.into(),
            mime_type: mime_type.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedToolImage {
    pub tool_call_id: String,
    pub tool_name: String,
    pub uri: String,
    pub mime_type: String,
    pub created_at: SystemTime,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolImageCacheRequest {
    pub base64_data: String,
    pub tool_call_id: String,
    pub tool_name: String,
    pub index: usize,
    pub mime_type: String,
}

impl ToolImageCacheRequest {
    pub fn new(
        base64_data: impl Into<String>,
        tool_call_id: impl Into<String>,
        tool_name: impl Into<String>,
    ) -> Self {
        Self {
            base64_data: base64_data.into(),
            tool_call_id: tool_call_id.into(),
            tool_name: tool_name.into(),
            index: 0,
            mime_type: "image/png".to_string(),
        }
    }

    pub fn with_index(mut self, index: usize) -> Self {
        self.index = index;
        self
    }

    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        let mime_type = mime_type.into();
        if !mime_type.trim().is_empty() {
            self.mime_type = mime_type;
        }
        self
    }
}

#[async_trait]
pub trait ToolImageCachePort: Send + Sync {
    async fn save_image(&self, request: ToolImageCacheRequest) -> Result<CachedToolImage>;

    async fn get_image(&self, uri: &str, mime_type: &str) -> Result<Option<ToolImageData>>;

    async fn cleanup_expired(&self) -> Result<usize>;
}

#[derive(Clone)]
pub struct NoopToolImageCache;

#[async_trait]
impl ToolImageCachePort for NoopToolImageCache {
    async fn save_image(&self, request: ToolImageCacheRequest) -> Result<CachedToolImage> {
        Ok(CachedToolImage {
            tool_call_id: request.tool_call_id,
            tool_name: request.tool_name,
            uri: String::new(),
            mime_type: request.mime_type,
            created_at: SystemTime::now(),
        })
    }

    async fn get_image(&self, _uri: &str, _mime_type: &str) -> Result<Option<ToolImageData>> {
        Ok(None)
    }

    async fn cleanup_expired(&self) -> Result<usize> {
        Ok(0)
    }
}

#[derive(Clone)]
pub struct InMemoryToolImageCache {
    inner: Arc<Mutex<HashMap<String, InMemoryCachedImage>>>,
    ttl: Duration,
}

impl Default for InMemoryToolImageCache {
    fn default() -> Self {
        Self::new(Duration::from_secs(3600))
    }
}

impl InMemoryToolImageCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            ttl: ttl.max(Duration::from_secs(1)),
        }
    }

    pub fn len(&self) -> usize {
        self.inner
            .lock()
            .map(|inner| inner.len())
            .unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[async_trait]
impl ToolImageCachePort for InMemoryToolImageCache {
    async fn save_image(&self, request: ToolImageCacheRequest) -> Result<CachedToolImage> {
        let uri = format!(
            "tool-image://{}-{}{}",
            sanitize_uri_part(&request.tool_call_id),
            request.index,
            extension_for_mime_type(&request.mime_type)
        );
        let cached = CachedToolImage {
            tool_call_id: request.tool_call_id,
            tool_name: request.tool_name,
            uri: uri.clone(),
            mime_type: request.mime_type,
            created_at: SystemTime::now(),
        };
        let image = InMemoryCachedImage {
            data: ToolImageData::new(request.base64_data, cached.mime_type.clone()),
            metadata: cached.clone(),
        };

        self.inner
            .lock()
            .map_err(|_| AstrbotError::Pipeline("tool image cache lock poisoned".to_string()))?
            .insert(uri, image);
        Ok(cached)
    }

    async fn get_image(&self, uri: &str, mime_type: &str) -> Result<Option<ToolImageData>> {
        let image = self
            .inner
            .lock()
            .map_err(|_| AstrbotError::Pipeline("tool image cache lock poisoned".to_string()))?
            .get(uri)
            .cloned();
        let Some(image) = image else {
            return Ok(None);
        };

        if image.is_expired(self.ttl) {
            return Ok(None);
        }

        let mut data = image.data;
        if !mime_type.trim().is_empty() {
            data.mime_type = mime_type.to_string();
        }
        Ok(Some(data))
    }

    async fn cleanup_expired(&self) -> Result<usize> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| AstrbotError::Pipeline("tool image cache lock poisoned".to_string()))?;
        let before = inner.len();
        let ttl = self.ttl;
        inner.retain(|_, image| !image.is_expired(ttl));
        Ok(before - inner.len())
    }
}

#[derive(Clone)]
struct InMemoryCachedImage {
    data: ToolImageData,
    metadata: CachedToolImage,
}

impl InMemoryCachedImage {
    fn is_expired(&self, ttl: Duration) -> bool {
        self.metadata
            .created_at
            .elapsed()
            .is_ok_and(|elapsed| elapsed > ttl)
    }
}

fn sanitize_uri_part(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.trim_matches('_').is_empty() {
        "image".to_string()
    } else {
        sanitized
    }
}

fn extension_for_mime_type(mime_type: &str) -> &'static str {
    match mime_type.trim().to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => ".jpg",
        "image/gif" => ".gif",
        "image/webp" => ".webp",
        "image/bmp" => ".bmp",
        "image/svg+xml" => ".svg",
        _ => ".png",
    }
}
