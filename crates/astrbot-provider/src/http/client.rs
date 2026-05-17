use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use astrbot_net::HttpClientPolicy;
use reqwest::header::HeaderMap;

pub(crate) fn build_http_client(timeout: Duration, headers: HeaderMap) -> Result<reqwest::Client> {
    HttpClientPolicy::default()
        .with_timeout(timeout)
        .build_client_with_default_headers(headers)
        .map_err(|err| AstrbotError::Provider(format!("failed to build HTTP client: {err}")))
}
