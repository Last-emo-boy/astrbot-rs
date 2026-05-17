use std::fmt;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use reqwest::header::{AUTHORIZATION, HeaderMap, PROXY_AUTHORIZATION};

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

#[derive(Clone)]
pub struct ReqwestMediaDownloadService {
    client: reqwest::Client,
}

impl fmt::Debug for ReqwestMediaDownloadService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReqwestMediaDownloadService")
            .finish_non_exhaustive()
    }
}

impl ReqwestMediaDownloadService {
    pub fn new(policy: MediaDownloadPolicy) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(policy.timeout)
            .no_gzip()
            .build()
            .map_err(|err| {
                AstrbotError::Provider(format!("failed to build media download client: {err}"))
            })?;
        Ok(Self { client })
    }
}

#[async_trait]
impl MediaDownloadService for ReqwestMediaDownloadService {
    async fn download(&self, request: MediaDownloadRequest) -> Result<DownloadedMedia> {
        if !is_http_url(&request.url) {
            return Err(AstrbotError::Provider(
                "media download requires an HTTP or HTTPS URL".to_string(),
            ));
        }

        let download_request = self.client.get(&request.url).build().map_err(|err| {
            AstrbotError::Provider(format!(
                "failed to build media download request for {}: {err}",
                request.url
            ))
        })?;
        assert_no_sensitive_download_headers(download_request.headers())?;

        let response = self.client.execute(download_request).await.map_err(|err| {
            AstrbotError::Provider(format!("media download failed for {}: {err}", request.url))
        })?;

        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.split(';').next().unwrap_or(value).trim().to_string())
            .filter(|value| !value.is_empty());
        let bytes = response.bytes().await.map_err(|err| {
            AstrbotError::Provider(format!("failed to read media download response: {err}"))
        })?;

        if !status.is_success() {
            return Err(AstrbotError::Provider(format!(
                "media download returned {status} for {}",
                request.url
            )));
        }
        if bytes.is_empty() {
            return Err(AstrbotError::Provider(
                "downloaded media was empty".to_string(),
            ));
        }
        if let Some(limit) = request.policy.max_bytes
            && bytes.len() > limit
        {
            return Err(AstrbotError::Provider(format!(
                "downloaded media exceeded size limit of {limit} bytes"
            )));
        }

        Ok(DownloadedMedia {
            url: request.url,
            content_type,
            bytes: bytes.to_vec(),
        })
    }
}

pub fn assert_no_sensitive_download_headers(headers: &HeaderMap) -> Result<()> {
    if headers.contains_key(AUTHORIZATION) || headers.contains_key(PROXY_AUTHORIZATION) {
        return Err(AstrbotError::Provider(
            "media downloads must not reuse provider authorization headers".to_string(),
        ));
    }
    Ok(())
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
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
