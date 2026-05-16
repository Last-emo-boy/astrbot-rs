use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use reqwest::header::HeaderMap;

pub(crate) fn build_http_client(timeout: Duration, headers: HeaderMap) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(timeout)
        .default_headers(headers)
        .build()
        .map_err(|err| AstrbotError::Provider(format!("failed to build HTTP client: {err}")))
}
