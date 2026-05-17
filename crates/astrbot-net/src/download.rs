use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use astrbot_core::{AstrbotError, Result};
use astrbot_storage::TempArtifactRoot;
use async_trait::async_trait;
use reqwest::header::{AUTHORIZATION, HeaderMap, PROXY_AUTHORIZATION};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::progress::{
    DownloadProgressEvent, DownloadProgressSink, NoopDownloadProgressSink, ProgressTracker,
};
use crate::tls::{HttpClientPolicy, TlsVerificationPolicy};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadMethod {
    #[default]
    Get,
    PostJson,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadDestination {
    #[default]
    Memory,
    Path(PathBuf),
    Temp {
        bucket: String,
        file_name: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub url: String,
    pub method: DownloadMethod,
    pub json_body: Option<Value>,
    pub destination: DownloadDestination,
    pub expected_content_types: Vec<String>,
    pub max_bytes: Option<usize>,
    pub client_policy: HttpClientPolicy,
}

impl DownloadRequest {
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: DownloadMethod::Get,
            json_body: None,
            destination: DownloadDestination::Memory,
            expected_content_types: Vec::new(),
            max_bytes: None,
            client_policy: HttpClientPolicy::default(),
        }
    }

    pub fn post_json(url: impl Into<String>, body: Value) -> Self {
        Self {
            method: DownloadMethod::PostJson,
            json_body: Some(body),
            ..Self::get(url)
        }
    }

    pub fn with_destination(mut self, destination: DownloadDestination) -> Self {
        self.destination = destination;
        self
    }

    pub fn with_max_bytes(mut self, max_bytes: Option<usize>) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    pub fn with_expected_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.expected_content_types.push(content_type.into());
        self
    }

    pub fn with_client_policy(mut self, client_policy: HttpClientPolicy) -> Self {
        self.client_policy = client_policy;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadResponse {
    pub url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
    pub path: Option<PathBuf>,
}

#[async_trait]
pub trait DownloadService: Send + Sync {
    async fn download(&self, request: DownloadRequest) -> Result<DownloadResponse>;
}

#[derive(Clone)]
pub struct ReqwestDownloadService {
    temp_root: TempArtifactRoot,
    progress: Arc<dyn DownloadProgressSink>,
}

impl fmt::Debug for ReqwestDownloadService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReqwestDownloadService")
            .field("temp_root", &self.temp_root)
            .finish_non_exhaustive()
    }
}

impl Default for ReqwestDownloadService {
    fn default() -> Self {
        Self {
            temp_root: TempArtifactRoot::default(),
            progress: Arc::new(NoopDownloadProgressSink),
        }
    }
}

impl ReqwestDownloadService {
    pub fn new(temp_root: TempArtifactRoot) -> Self {
        Self {
            temp_root,
            progress: Arc::new(NoopDownloadProgressSink),
        }
    }

    pub fn with_progress_sink(mut self, progress: Arc<dyn DownloadProgressSink>) -> Self {
        self.progress = progress;
        self
    }
}

#[async_trait]
impl DownloadService for ReqwestDownloadService {
    async fn download(&self, request: DownloadRequest) -> Result<DownloadResponse> {
        match download_with_policy(&self.temp_root, self.progress.as_ref(), request.clone()).await {
            Ok(response) => Ok(response),
            Err(error)
                if request
                    .client_policy
                    .tls_verification
                    .allows_insecure_fallback()
                    && looks_like_tls_error(&error) =>
            {
                let mut retry = request;
                retry.client_policy = retry
                    .client_policy
                    .with_tls_verification(TlsVerificationPolicy::Disabled);
                download_with_policy(&self.temp_root, self.progress.as_ref(), retry).await
            }
            Err(error) => Err(error),
        }
    }
}

async fn download_with_policy(
    temp_root: &TempArtifactRoot,
    progress: &dyn DownloadProgressSink,
    request: DownloadRequest,
) -> Result<DownloadResponse> {
    if !is_http_url(&request.url) {
        return Err(AstrbotError::Provider(
            "download requires an HTTP or HTTPS URL".to_string(),
        ));
    }

    let client = request.client_policy.build_client().map_err(|err| {
        AstrbotError::Provider(format!("failed to build download HTTP client: {err}"))
    })?;
    let mut builder = match request.method {
        DownloadMethod::Get => client.get(&request.url),
        DownloadMethod::PostJson => client.post(&request.url).json(
            request
                .json_body
                .as_ref()
                .ok_or_else(|| AstrbotError::Provider("missing JSON download body".to_string()))?,
        ),
    };
    builder = builder.timeout(request.client_policy.timeout);
    let http_request = builder.build().map_err(|err| {
        AstrbotError::Provider(format!(
            "failed to build download request for {}: {err}",
            request.url
        ))
    })?;
    assert_no_sensitive_download_headers(http_request.headers())?;

    let response = client.execute(http_request).await.map_err(|err| {
        AstrbotError::Provider(format!("download failed for {}: {err}", request.url))
    })?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.split(';').next().unwrap_or(value).trim().to_string())
        .filter(|value| !value.is_empty());
    let total_bytes = response.content_length();
    let mut tracker = ProgressTracker::new(request.url.clone(), total_bytes);
    progress
        .record(DownloadProgressEvent::Started(tracker.snapshot()))
        .await;

    let bytes = response.bytes().await.map_err(|err| {
        AstrbotError::Provider(format!("failed to read download response: {err}"))
    })?;
    let snapshot = tracker.advance(bytes.len() as u64);
    progress
        .record(DownloadProgressEvent::Advanced(snapshot.clone()))
        .await;

    if !status.is_success() {
        return Err(AstrbotError::Provider(format!(
            "download returned {status} for {}",
            request.url
        )));
    }
    if bytes.is_empty() {
        return Err(AstrbotError::Provider(
            "downloaded response was empty".to_string(),
        ));
    }
    if let Some(limit) = request.max_bytes
        && bytes.len() > limit
    {
        return Err(AstrbotError::Provider(format!(
            "download exceeded size limit of {limit} bytes"
        )));
    }
    validate_content_type(content_type.as_deref(), &request.expected_content_types)?;

    let path = match &request.destination {
        DownloadDestination::Memory => None,
        DownloadDestination::Path(path) => {
            write_download_file(path, &bytes)?;
            Some(path.clone())
        }
        DownloadDestination::Temp { bucket, file_name } => {
            let artifact = temp_root.allocate(bucket, file_name);
            write_download_file(&artifact.path, &bytes)?;
            Some(artifact.path)
        }
    };

    progress
        .record(DownloadProgressEvent::Finished(snapshot))
        .await;
    Ok(DownloadResponse {
        url: request.url,
        status: status.as_u16(),
        content_type,
        bytes: bytes.to_vec(),
        path,
    })
}

#[derive(Clone)]
pub struct FileDownloadService {
    inner: Arc<dyn DownloadService>,
}

impl FileDownloadService {
    pub fn new(inner: Arc<dyn DownloadService>) -> Self {
        Self { inner }
    }

    pub async fn download_to_path(&self, url: impl Into<String>, path: PathBuf) -> Result<PathBuf> {
        let response = self
            .inner
            .download(DownloadRequest::get(url).with_destination(DownloadDestination::Path(path)))
            .await?;
        response.path.ok_or_else(|| {
            AstrbotError::Provider("download did not produce a file path".to_string())
        })
    }
}

pub fn assert_no_sensitive_download_headers(headers: &HeaderMap) -> Result<()> {
    if headers.contains_key(AUTHORIZATION) || headers.contains_key(PROXY_AUTHORIZATION) {
        return Err(AstrbotError::Provider(
            "downloads must not reuse provider authorization headers".to_string(),
        ));
    }
    Ok(())
}

pub fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn validate_content_type(content_type: Option<&str>, expected: &[String]) -> Result<()> {
    if expected.is_empty() {
        return Ok(());
    }
    let Some(content_type) = content_type else {
        return Err(AstrbotError::Provider(
            "download response did not include a content type".to_string(),
        ));
    };
    let content_type = content_type.to_ascii_lowercase();
    if expected
        .iter()
        .any(|expected| content_type == expected.to_ascii_lowercase())
    {
        Ok(())
    } else {
        Err(AstrbotError::Provider(format!(
            "download content type {content_type} did not match expected types"
        )))
    }
}

fn write_download_file(path: &PathBuf, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            AstrbotError::Provider(format!(
                "failed to create download directory {}: {err}",
                parent.display()
            ))
        })?;
    }
    fs::write(path, bytes).map_err(|err| {
        AstrbotError::Provider(format!(
            "failed to write download file {}: {err}",
            path.display()
        ))
    })
}

fn looks_like_tls_error(error: &AstrbotError) -> bool {
    error
        .to_string()
        .to_ascii_lowercase()
        .contains("certificate")
        || error.to_string().to_ascii_lowercase().contains("tls")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use crate::progress::{DownloadProgressEvent, DownloadProgressSink};
    use crate::{
        DownloadDestination, DownloadProgressSnapshot, DownloadRequest, DownloadService,
        HttpClientPolicy, ReqwestDownloadService, assert_no_sensitive_download_headers,
    };

    #[test]
    fn rejects_provider_authorization_headers_for_downloads() {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer provider-key"),
        );

        let error = assert_no_sensitive_download_headers(&headers).expect_err("auth header");

        assert!(error.to_string().contains("authorization"));
    }

    #[tokio::test]
    async fn reqwest_download_writes_temp_file_and_reports_progress() {
        let sink = Arc::new(RecordingProgressSink::default());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0; 4096];
            let _ = stream.read(&mut buffer).await.unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 8\r\n\r\nPNGDATA!",
                )
                .await
                .unwrap();
        });
        let root = std::env::temp_dir().join(format!("astrbot_net_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let service = ReqwestDownloadService::new(astrbot_storage::TempArtifactRoot::new(&root))
            .with_progress_sink(sink.clone());

        let response = service
            .download(
                DownloadRequest::get(format!("http://{addr}/image.png"))
                    .with_client_policy(HttpClientPolicy::default().without_env_proxy())
                    .with_expected_content_type("image/png")
                    .with_destination(DownloadDestination::Temp {
                        bucket: "images".to_string(),
                        file_name: "hello.png".to_string(),
                    }),
            )
            .await
            .unwrap();

        assert_eq!(response.bytes, b"PNGDATA!");
        assert_eq!(
            response.path.as_deref(),
            Some(PathBuf::from(&root).join("images/hello_png").as_path())
        );
        assert_eq!(std::fs::read(response.path.unwrap()).unwrap(), b"PNGDATA!");
        assert!(sink.events.lock().unwrap().len() >= 3);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[derive(Default)]
    struct RecordingProgressSink {
        events: Mutex<Vec<DownloadProgressEvent>>,
    }

    #[async_trait]
    impl DownloadProgressSink for RecordingProgressSink {
        async fn record(&self, event: DownloadProgressEvent) {
            if let DownloadProgressEvent::Finished(DownloadProgressSnapshot {
                downloaded_bytes,
                ..
            }) = &event
            {
                assert!(*downloaded_bytes > 0);
            }
            self.events.lock().unwrap().push(event);
        }
    }
}
