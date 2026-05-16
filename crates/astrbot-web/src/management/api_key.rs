use std::collections::BTreeSet;

use astrbot_core::Result;
use astrbot_storage::{ApiKeyRecord, ApiKeyRepository};
use axum::http::{HeaderMap, header};
use sha1::{Digest, Sha1};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OpenApiScope {
    ManagementRead,
    ManagementWrite,
    Chat,
    ProviderRead,
    PluginRead,
    Custom(String),
}

impl OpenApiScope {
    pub fn as_str(&self) -> &str {
        match self {
            Self::ManagementRead => "management.read",
            Self::ManagementWrite => "management.write",
            Self::Chat => "openapi.chat",
            Self::ProviderRead => "provider.read",
            Self::PluginRead => "plugin.read",
            Self::Custom(scope) => scope,
        }
    }
}

impl From<&str> for OpenApiScope {
    fn from(scope: &str) -> Self {
        match scope.trim() {
            "management.read" => Self::ManagementRead,
            "management.write" => Self::ManagementWrite,
            "openapi.chat" => Self::Chat,
            "provider.read" => Self::ProviderRead,
            "plugin.read" => Self::PluginRead,
            other => Self::Custom(other.to_string()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OpenApiScopeSet {
    scopes: BTreeSet<OpenApiScope>,
}

impl OpenApiScopeSet {
    pub fn new(scopes: impl IntoIterator<Item = OpenApiScope>) -> Self {
        Self {
            scopes: scopes.into_iter().collect(),
        }
    }

    pub fn from_strings(scopes: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        Self::new(
            scopes
                .into_iter()
                .map(|scope| OpenApiScope::from(scope.as_ref())),
        )
    }

    pub fn contains(&self, scope: &OpenApiScope) -> bool {
        self.scopes.contains(scope)
    }

    pub fn allows_all(&self, required: &[OpenApiScope]) -> bool {
        required.iter().all(|scope| self.contains(scope))
    }

    pub fn to_strings(&self) -> Vec<String> {
        self.scopes
            .iter()
            .map(|scope| scope.as_str().to_string())
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IssuedApiKey {
    pub record: ApiKeyRecord,
    pub secret: String,
}

pub struct ApiKeyIssuer;

impl ApiKeyIssuer {
    pub fn issue(
        key_id: impl Into<String>,
        name: impl Into<String>,
        secret: impl Into<String>,
        scopes: OpenApiScopeSet,
        created_by: impl Into<String>,
    ) -> IssuedApiKey {
        let secret = secret.into();
        let prefix = key_prefix(&secret);
        let record = ApiKeyRecord::new(
            key_id,
            name,
            hash_api_key(&secret),
            prefix,
            scopes.to_strings(),
            created_by,
        );
        IssuedApiKey { record, secret }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentedApiKey {
    pub secret: String,
    pub key_hash: String,
    pub key_prefix: String,
}

impl PresentedApiKey {
    pub fn new(secret: impl Into<String>) -> Self {
        let secret = secret.into();
        Self {
            key_hash: hash_api_key(&secret),
            key_prefix: key_prefix(&secret),
            secret,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApiKeyAuthDecision {
    Allowed(ApiKeyRecord),
    Denied(ApiKeyRejectionReason),
}

impl ApiKeyAuthDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiKeyRejectionReason {
    MissingKey,
    UnknownKey,
    Revoked,
    MissingScope,
}

pub async fn authorize_api_key(
    repository: &dyn ApiKeyRepository,
    presented: Option<&PresentedApiKey>,
    required_scopes: &[OpenApiScope],
) -> Result<ApiKeyAuthDecision> {
    let Some(presented) = presented else {
        return Ok(ApiKeyAuthDecision::Denied(
            ApiKeyRejectionReason::MissingKey,
        ));
    };

    let Some(record) = repository.api_key_by_hash(&presented.key_hash).await? else {
        return Ok(ApiKeyAuthDecision::Denied(
            ApiKeyRejectionReason::UnknownKey,
        ));
    };

    if record.is_revoked() {
        return Ok(ApiKeyAuthDecision::Denied(ApiKeyRejectionReason::Revoked));
    }

    let scopes = OpenApiScopeSet::from_strings(&record.scopes);
    if !scopes.allows_all(required_scopes) {
        return Ok(ApiKeyAuthDecision::Denied(
            ApiKeyRejectionReason::MissingScope,
        ));
    }

    Ok(ApiKeyAuthDecision::Allowed(record))
}

pub fn extract_presented_api_key(headers: &HeaderMap) -> Option<PresentedApiKey> {
    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
            value.strip_prefix("Bearer ").map(str::trim)
        })
        .filter(|value| !value.is_empty())
        .map(PresentedApiKey::new)
}

pub fn hash_api_key(secret: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(secret.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn key_prefix(secret: &str) -> String {
    secret.chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    use astrbot_storage::{ApiKeyRepository, InMemoryApiKeyRepository};

    use super::{
        ApiKeyAuthDecision, ApiKeyIssuer, ApiKeyRejectionReason, OpenApiScope, OpenApiScopeSet,
        PresentedApiKey, authorize_api_key, extract_presented_api_key, hash_api_key,
    };

    #[test]
    fn openapi_scope_set_round_trips_typed_scopes() {
        let scopes = OpenApiScopeSet::from_strings(["management.read", "openapi.chat"]);

        assert!(scopes.allows_all(&[OpenApiScope::ManagementRead, OpenApiScope::Chat]));
        assert_eq!(
            scopes.to_strings(),
            vec!["management.read".to_string(), "openapi.chat".to_string()]
        );
    }

    #[test]
    fn api_key_issuer_hashes_secret_and_stores_prefix() {
        let issued = ApiKeyIssuer::issue(
            "key-1",
            "CI",
            "ak_test_secret",
            OpenApiScopeSet::new([OpenApiScope::ManagementRead]),
            "admin",
        );

        assert_eq!(issued.record.key_prefix, "ak_test_");
        assert_eq!(issued.record.key_hash, hash_api_key("ak_test_secret"));
        assert_eq!(issued.record.scopes, vec!["management.read".to_string()]);
    }

    #[test]
    fn api_key_extractor_accepts_header_or_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_static("ak_secret"));

        assert_eq!(
            extract_presented_api_key(&headers)
                .expect("api key should extract")
                .key_hash,
            hash_api_key("ak_secret")
        );
    }

    #[tokio::test]
    async fn api_key_authorizer_checks_repository_revocation_and_scopes() {
        let repository = InMemoryApiKeyRepository::new();
        let issued = ApiKeyIssuer::issue(
            "key-1",
            "CI",
            "ak_test_secret",
            OpenApiScopeSet::new([OpenApiScope::ManagementRead]),
            "admin",
        );
        repository
            .store_api_key(issued.record.clone())
            .await
            .expect("api key should store");

        assert!(matches!(
            authorize_api_key(
                &repository,
                Some(&PresentedApiKey::new("ak_test_secret")),
                &[OpenApiScope::ManagementRead],
            )
            .await
            .expect("api key should authorize"),
            ApiKeyAuthDecision::Allowed(_)
        ));
        assert_eq!(
            authorize_api_key(
                &repository,
                Some(&PresentedApiKey::new("ak_test_secret")),
                &[OpenApiScope::ManagementWrite],
            )
            .await
            .expect("api key should reject"),
            ApiKeyAuthDecision::Denied(ApiKeyRejectionReason::MissingScope)
        );
    }
}
