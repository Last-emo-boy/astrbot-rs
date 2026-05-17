use std::time::Duration;

use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpClientPolicy {
    pub timeout: Duration,
    pub trust_env_proxy: bool,
    pub tls_verification: TlsVerificationPolicy,
}

impl Default for HttpClientPolicy {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            trust_env_proxy: true,
            tls_verification: TlsVerificationPolicy::VerifiedWithInsecureFallback,
        }
    }
}

impl HttpClientPolicy {
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn without_env_proxy(mut self) -> Self {
        self.trust_env_proxy = false;
        self
    }

    pub fn with_tls_verification(mut self, tls_verification: TlsVerificationPolicy) -> Self {
        self.tls_verification = tls_verification;
        self
    }

    pub fn build_client(&self) -> Result<reqwest::Client, reqwest::Error> {
        self.client_builder().build()
    }

    pub fn build_client_with_default_headers(
        &self,
        headers: HeaderMap,
    ) -> Result<reqwest::Client, reqwest::Error> {
        self.client_builder().default_headers(headers).build()
    }

    fn client_builder(&self) -> reqwest::ClientBuilder {
        let mut builder = reqwest::Client::builder().timeout(self.timeout);
        if !self.trust_env_proxy {
            builder = builder.no_proxy();
        }
        if matches!(self.tls_verification, TlsVerificationPolicy::Disabled) {
            builder = builder.danger_accept_invalid_certs(true);
        }
        builder
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TlsVerificationPolicy {
    #[default]
    Verified,
    VerifiedWithInsecureFallback,
    Disabled,
}

impl TlsVerificationPolicy {
    pub fn allows_insecure_fallback(self) -> bool {
        matches!(self, Self::VerifiedWithInsecureFallback)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{HttpClientPolicy, TlsVerificationPolicy};

    #[test]
    fn client_policy_records_proxy_tls_and_timeout_choices() {
        let policy = HttpClientPolicy::default()
            .with_timeout(Duration::from_secs(5))
            .without_env_proxy()
            .with_tls_verification(TlsVerificationPolicy::Disabled);

        assert_eq!(policy.timeout, Duration::from_secs(5));
        assert!(!policy.trust_env_proxy);
        assert_eq!(policy.tls_verification, TlsVerificationPolicy::Disabled);
        assert!(policy.build_client().is_ok());
    }

    #[test]
    fn client_policy_can_build_client_with_default_headers() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );

        let client = HttpClientPolicy::default()
            .build_client_with_default_headers(headers)
            .expect("client should build");

        assert!(format!("{client:?}").contains("Client"));
    }
}
