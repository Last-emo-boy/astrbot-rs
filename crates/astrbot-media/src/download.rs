use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use astrbot_net::{DownloadRequest, DownloadService, HttpClientPolicy, ReqwestDownloadService};
use async_trait::async_trait;
use reqwest::header::HeaderMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaDownloadPolicy {
    pub timeout: Duration,
    pub max_bytes: Option<usize>,
}

impl Default for MediaDownloadPolicy {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_bytes: Some(20 * 1024 * 1024),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaDownloadRequest {
    pub url: String,
    pub policy: MediaDownloadPolicy,
}

impl MediaDownloadRequest {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            policy: MediaDownloadPolicy::default(),
        }
    }

    pub fn with_policy(mut self, policy: MediaDownloadPolicy) -> Self {
        self.policy = policy;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DownloadedMedia {
    pub url: String,
    pub content_type: Option<String>,
    pub bytes: Vec<u8>,
}

#[async_trait]
pub trait MediaDownloadService: Send + Sync {
    async fn download(&self, request: MediaDownloadRequest) -> Result<DownloadedMedia>;
}

#[derive(Clone, Debug)]
pub struct ReqwestMediaDownloadService {
    inner: ReqwestDownloadService,
    client_policy: HttpClientPolicy,
}

impl ReqwestMediaDownloadService {
    pub fn new(policy: MediaDownloadPolicy) -> Result<Self> {
        let client_policy = HttpClientPolicy::default().with_timeout(policy.timeout);
        client_policy.build_client().map_err(|err| {
            AstrbotError::Provider(format!("failed to build media download client: {err}"))
        })?;
        Ok(Self {
            inner: ReqwestDownloadService::default(),
            client_policy,
        })
    }
}

#[async_trait]
impl MediaDownloadService for ReqwestMediaDownloadService {
    async fn download(&self, request: MediaDownloadRequest) -> Result<DownloadedMedia> {
        if !astrbot_net::is_http_url(&request.url) {
            return Err(AstrbotError::Provider(
                "media download requires an HTTP or HTTPS URL".to_string(),
            ));
        }

        let response = self
            .inner
            .download(
                DownloadRequest::get(request.url.clone())
                    .with_client_policy(
                        self.client_policy
                            .clone()
                            .with_timeout(request.policy.timeout),
                    )
                    .with_max_bytes(request.policy.max_bytes),
            )
            .await
            .map_err(|err| {
                AstrbotError::Provider(format!("media download failed for {}: {err}", request.url))
            })?;

        Ok(DownloadedMedia {
            url: request.url,
            content_type: response.content_type,
            bytes: response.bytes,
        })
    }
}

pub fn assert_no_sensitive_download_headers(headers: &HeaderMap) -> Result<()> {
    astrbot_net::assert_no_sensitive_download_headers(headers)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    use super::{
        MediaDownloadPolicy, MediaDownloadRequest, MediaDownloadService,
        ReqwestMediaDownloadService, assert_no_sensitive_download_headers,
    };

    #[test]
    fn rejects_provider_authorization_headers_for_media_downloads() {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer provider-key"),
        );

        let error = assert_no_sensitive_download_headers(&headers).expect_err("auth header");

        assert!(error.to_string().contains("authorization"));
    }

    #[tokio::test]
    async fn reqwest_media_downloader_does_not_send_provider_authorization() {
        let captured = Arc::new(Mutex::new(String::new()));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test server should bind");
        let addr = listener.local_addr().expect("local addr");
        let captured_server = captured.clone();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("connection");
            let mut buffer = vec![0; 4096];
            let read = stream.read(&mut buffer).await.expect("read request");
            *captured_server.lock().await = String::from_utf8_lossy(&buffer[..read]).to_string();
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 8\r\n\r\nPNGDATA!",
                )
                .await
                .expect("write response");
        });
        let downloader =
            ReqwestMediaDownloadService::new(MediaDownloadPolicy::default()).expect("downloader");

        let media = downloader
            .download(MediaDownloadRequest::new(format!(
                "http://{addr}/image.png"
            )))
            .await
            .expect("download should succeed");

        assert_eq!(media.bytes, b"PNGDATA!");
        let request = captured.lock().await.clone();
        assert!(!request.to_ascii_lowercase().contains("authorization:"));
    }
}
