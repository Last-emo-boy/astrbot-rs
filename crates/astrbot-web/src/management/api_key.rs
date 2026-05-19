use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use astrbot_core::Result;
use astrbot_storage::{ApiKeyRecord, ApiKeyRepository};
use axum::http::{HeaderMap, header};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use pbkdf2::pbkdf2_hmac;
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use sha2::Sha256;

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OpenApiScope {
    ManagementRead,
    ManagementWrite,
    Chat,
    Config,
    File,
    Im,
    ProviderRead,
    PluginRead,
    Custom(String),
}

impl OpenApiScope {
    pub fn as_str(&self) -> &str {
        match self {
            Self::ManagementRead => "management.read",
            Self::ManagementWrite => "management.write",
            Self::Chat => "chat",
            Self::Config => "config",
            Self::File => "file",
            Self::Im => "im",
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
            "chat" | "openapi.chat" => Self::Chat,
            "config" | "openapi.config" => Self::Config,
            "file" | "openapi.file" => Self::File,
            "im" | "openapi.im" => Self::Im,
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
            || (*scope == OpenApiScope::ManagementRead
                && self.scopes.contains(&OpenApiScope::ManagementWrite))
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

#[derive(Clone)]
pub struct ManagementApiKeyState {
    repository: Arc<dyn ApiKeyRepository>,
    rate_limiter: Arc<ApiKeyRateLimiter>,
}

impl ManagementApiKeyState {
    pub fn new(repository: Arc<dyn ApiKeyRepository>) -> Self {
        Self {
            repository,
            rate_limiter: Arc::new(ApiKeyRateLimiter::new(120, 60)),
        }
    }

    pub fn repository(&self) -> Arc<dyn ApiKeyRepository> {
        self.repository.clone()
    }

    pub fn with_rate_limit(mut self, max_attempts: u32, window_seconds: u64) -> Self {
        self.rate_limiter = Arc::new(ApiKeyRateLimiter::new(max_attempts, window_seconds));
        self
    }

    pub async fn authorize_presented(
        &self,
        presented: Option<&PresentedApiKey>,
        required_scopes: &[OpenApiScope],
    ) -> Result<ApiKeyAuthDecision> {
        let identifier = presented
            .map(|key| key.key_prefix.as_str())
            .unwrap_or("missing");
        if !self.rate_limiter.check(identifier) {
            return Ok(ApiKeyAuthDecision::Denied(
                ApiKeyRejectionReason::RateLimited,
            ));
        }
        authorize_api_key(self.repository.as_ref(), presented, required_scopes).await
    }

    async fn catalog_response(&self) -> Result<ManagementApiKeyCatalogResponse> {
        let api_keys = self
            .repository
            .list_api_keys()
            .await?
            .into_iter()
            .map(ManagementApiKeyDescriptor::from)
            .collect();
        Ok(ManagementApiKeyCatalogResponse { api_keys })
    }

    async fn issue(
        &self,
        request: ManagementApiKeyIssueRequest,
    ) -> Result<ManagementApiKeyIssueResponse> {
        let key_id = non_empty(request.key_id).unwrap_or_else(generate_api_key_id);
        let name = non_empty(request.name).unwrap_or_else(|| "Untitled API Key".to_string());
        let secret = non_empty(request.secret).unwrap_or_else(generate_api_key_secret);
        let scopes = normalize_scope_strings(request.scopes)?;
        let issued = ApiKeyIssuer::issue(
            key_id.clone(),
            name.clone(),
            secret.clone(),
            OpenApiScopeSet::from_strings(scopes),
            request
                .created_by
                .filter(|created_by| !created_by.trim().is_empty())
                .unwrap_or_else(|| "dashboard".to_string()),
        );
        let mut record = issued.record;
        if let Some(expires_in_days) = request.expires_in_days {
            if expires_in_days == 0 {
                return Err(astrbot_core::AstrbotError::Pipeline(
                    "expires_in_days must be greater than 0".to_string(),
                ));
            }
            record = record.with_expires_at(format!(
                "unix:{}",
                current_unix_seconds().saturating_add(expires_in_days.saturating_mul(86_400))
            ));
        }
        if let Some(expires_at) = request
            .expires_at
            .filter(|expires_at| !expires_at.trim().is_empty())
        {
            record = record.with_expires_at(expires_at);
        }
        self.repository.store_api_key(record.clone()).await?;
        let catalog = self.catalog_response().await?;

        Ok(ManagementApiKeyIssueResponse {
            issued: ManagementApiKeyDescriptor::from(record),
            secret: issued.secret,
            api_keys: catalog.api_keys,
        })
    }

    async fn revoke(
        &self,
        request: ManagementApiKeyRevokeRequest,
    ) -> Result<ManagementApiKeyRevokeResponse> {
        let key_id = request.key_id.trim();
        if key_id.is_empty() {
            return Err(astrbot_core::AstrbotError::Pipeline(
                "key_id is required".to_string(),
            ));
        }
        let revoked = self
            .repository
            .revoke_api_key(
                key_id,
                request.revoked_at.unwrap_or_else(current_unix_timestamp),
            )
            .await?;
        let catalog = self.catalog_response().await?;

        Ok(ManagementApiKeyRevokeResponse {
            revoked,
            api_keys: catalog.api_keys,
        })
    }

    async fn delete(
        &self,
        request: ManagementApiKeyDeleteRequest,
    ) -> Result<ManagementApiKeyDeleteResponse> {
        let key_id = request.key_id.trim();
        if key_id.is_empty() {
            return Err(astrbot_core::AstrbotError::Pipeline(
                "key_id is required".to_string(),
            ));
        }
        let deleted = self.repository.delete_api_key(key_id).await?;
        let catalog = self.catalog_response().await?;

        Ok(ManagementApiKeyDeleteResponse {
            deleted,
            api_keys: catalog.api_keys,
        })
    }
}

impl std::fmt::Debug for ManagementApiKeyState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagementApiKeyState")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementApiKeyDescriptor {
    pub key_id: String,
    pub name: String,
    pub key_prefix: String,
    pub scopes: Vec<String>,
    pub created_by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<String>,
    pub active: bool,
    pub is_expired: bool,
}

impl From<ApiKeyRecord> for ManagementApiKeyDescriptor {
    fn from(record: ApiKeyRecord) -> Self {
        let is_expired = record.is_expired_at(current_unix_seconds());
        let active = !record.is_revoked() && !is_expired;
        Self {
            key_id: record.key_id,
            name: record.name,
            key_prefix: record.key_prefix,
            scopes: record.scopes,
            created_by: record.created_by,
            last_used_at: record.last_used_at,
            expires_at: record.expires_at,
            active,
            is_expired,
            revoked_at: record.revoked_at,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementApiKeyCatalogResponse {
    pub api_keys: Vec<ManagementApiKeyDescriptor>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementApiKeyIssueRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_in_days: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementApiKeyIssueResponse {
    pub issued: ManagementApiKeyDescriptor,
    pub secret: String,
    pub api_keys: Vec<ManagementApiKeyDescriptor>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementApiKeyRevokeRequest {
    pub key_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementApiKeyRevokeResponse {
    pub revoked: bool,
    pub api_keys: Vec<ManagementApiKeyDescriptor>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementApiKeyDeleteRequest {
    pub key_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementApiKeyDeleteResponse {
    pub deleted: bool,
    pub api_keys: Vec<ManagementApiKeyDescriptor>,
}

pub async fn catalog(
    State(state): State<ManagementApiState>,
) -> std::result::Result<Json<ManagementApiKeyCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let api_keys = state.api_keys().ok_or_else(api_keys_unavailable)?;
    api_keys
        .catalog_response()
        .await
        .map(Json)
        .map_err(map_api_key_error)
}

pub async fn issue(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementApiKeyIssueRequest>,
) -> std::result::Result<Json<ManagementApiKeyIssueResponse>, (StatusCode, Json<ErrorResponse>)> {
    let api_keys = state.api_keys().ok_or_else(api_keys_unavailable)?;
    api_keys
        .issue(request)
        .await
        .map(Json)
        .map_err(map_api_key_error)
}

pub async fn revoke(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementApiKeyRevokeRequest>,
) -> std::result::Result<Json<ManagementApiKeyRevokeResponse>, (StatusCode, Json<ErrorResponse>)> {
    let api_keys = state.api_keys().ok_or_else(api_keys_unavailable)?;
    api_keys
        .revoke(request)
        .await
        .map(Json)
        .map_err(map_api_key_error)
}

pub async fn delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementApiKeyDeleteRequest>,
) -> std::result::Result<Json<ManagementApiKeyDeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let api_keys = state.api_keys().ok_or_else(api_keys_unavailable)?;
    api_keys
        .delete(request)
        .await
        .map(Json)
        .map_err(map_api_key_error)
}

pub async fn legacy_catalog(
    State(state): State<ManagementApiState>,
) -> std::result::Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let api_keys = state.api_keys().ok_or_else(api_keys_unavailable)?;
    let catalog = api_keys
        .catalog_response()
        .await
        .map_err(map_api_key_error)?;
    Ok(source_ok(api_key_descriptors_to_source(catalog.api_keys)))
}

pub async fn legacy_issue(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementApiKeyIssueRequest>,
) -> std::result::Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let api_keys = state.api_keys().ok_or_else(api_keys_unavailable)?;
    let issued = api_keys.issue(request).await.map_err(map_api_key_error)?;
    let mut data = api_key_descriptor_to_source(issued.issued);
    data["api_key"] = json!(issued.secret);
    Ok(source_ok(data))
}

pub async fn legacy_revoke(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementApiKeyRevokeRequest>,
) -> std::result::Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let api_keys = state.api_keys().ok_or_else(api_keys_unavailable)?;
    let revoked = api_keys.revoke(request).await.map_err(map_api_key_error)?;
    if revoked.revoked {
        Ok(source_ok(json!({})))
    } else {
        Err(api_key_not_found())
    }
}

pub async fn legacy_delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementApiKeyDeleteRequest>,
) -> std::result::Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let api_keys = state.api_keys().ok_or_else(api_keys_unavailable)?;
    let deleted = api_keys.delete(request).await.map_err(map_api_key_error)?;
    if deleted.deleted {
        Ok(source_ok(json!({})))
    } else {
        Err(api_key_not_found())
    }
}

pub async fn legacy_revoke_path(
    State(state): State<ManagementApiState>,
    Path(key_id): Path<String>,
) -> std::result::Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    legacy_revoke(
        State(state),
        Json(ManagementApiKeyRevokeRequest {
            key_id,
            revoked_at: None,
        }),
    )
    .await
}

pub async fn legacy_delete_path(
    State(state): State<ManagementApiState>,
    Path(key_id): Path<String>,
) -> std::result::Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    legacy_delete(State(state), Json(ManagementApiKeyDeleteRequest { key_id })).await
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentedApiKey {
    pub secret: String,
    pub key_hash: String,
    pub legacy_key_hash: String,
    pub key_prefix: String,
}

impl PresentedApiKey {
    pub fn new(secret: impl Into<String>) -> Self {
        let secret = secret.into();
        Self {
            key_hash: hash_api_key(&secret),
            legacy_key_hash: legacy_sha1_api_key_hash(&secret),
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
    Expired,
    MissingScope,
    RateLimited,
}

#[derive(Debug)]
struct ApiKeyRateLimiter {
    max_attempts: u32,
    window_seconds: u64,
    attempts: Mutex<BTreeMap<String, RateLimitBucket>>,
}

#[derive(Clone, Copy, Debug)]
struct RateLimitBucket {
    window_started_at: u64,
    attempts: u32,
}

impl ApiKeyRateLimiter {
    fn new(max_attempts: u32, window_seconds: u64) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
            window_seconds: window_seconds.max(1),
            attempts: Mutex::new(BTreeMap::new()),
        }
    }

    fn check(&self, identifier: &str) -> bool {
        let now = current_unix_seconds();
        let Ok(mut attempts) = self.attempts.lock() else {
            return false;
        };
        let bucket = attempts
            .entry(identifier.to_string())
            .or_insert(RateLimitBucket {
                window_started_at: now,
                attempts: 0,
            });
        if now.saturating_sub(bucket.window_started_at) >= self.window_seconds {
            *bucket = RateLimitBucket {
                window_started_at: now,
                attempts: 0,
            };
        }
        if bucket.attempts >= self.max_attempts {
            return false;
        }
        bucket.attempts += 1;
        true
    }
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

    let (mut record, legacy_migration) =
        match repository.api_key_by_hash(&presented.key_hash).await? {
            Some(record) => (record, false),
            None => match repository
                .api_key_by_hash(&presented.legacy_key_hash)
                .await?
            {
                Some(record) => (record, true),
                None => {
                    return Ok(ApiKeyAuthDecision::Denied(
                        ApiKeyRejectionReason::UnknownKey,
                    ));
                }
            },
        };

    if record.is_revoked() {
        return Ok(ApiKeyAuthDecision::Denied(ApiKeyRejectionReason::Revoked));
    }
    if record.is_expired_at(current_unix_seconds()) {
        return Ok(ApiKeyAuthDecision::Denied(ApiKeyRejectionReason::Expired));
    }

    let scopes = OpenApiScopeSet::from_strings(&record.scopes);
    if !scopes.allows_all(required_scopes) {
        return Ok(ApiKeyAuthDecision::Denied(
            ApiKeyRejectionReason::MissingScope,
        ));
    }

    if legacy_migration {
        record.key_hash = presented.key_hash.clone();
    }
    record.last_used_at = Some(current_unix_timestamp());
    repository.store_api_key(record.clone()).await?;

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
    let mut output = [0_u8; 32];
    pbkdf2_hmac::<Sha256>(secret.as_bytes(), b"astrbot_api_key", 100_000, &mut output);
    hex_encode(&output)
}

pub fn legacy_sha1_api_key_hash(secret: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(secret.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn key_prefix(secret: &str) -> String {
    secret.chars().take(12).collect()
}

fn normalize_scope_strings(scopes: Vec<String>) -> Result<Vec<String>> {
    if scopes.is_empty() {
        return Ok(vec![
            OpenApiScope::Chat.as_str().to_string(),
            OpenApiScope::Config.as_str().to_string(),
            OpenApiScope::File.as_str().to_string(),
            OpenApiScope::Im.as_str().to_string(),
        ]);
    }
    let mut normalized_scopes = Vec::new();
    for scope in scopes {
        if let Some(scope) = normalize_scope(&scope)? {
            normalized_scopes.push(scope);
        }
    }
    let mut scopes = normalized_scopes;
    if scopes.is_empty() {
        return Err(astrbot_core::AstrbotError::Pipeline(
            "At least one valid scope is required".to_string(),
        ));
    }
    scopes.sort();
    scopes.dedup();
    Ok(scopes)
}

fn normalize_scope(scope: &str) -> Result<Option<String>> {
    let scope = scope.trim();
    if scope.is_empty() {
        return Ok(None);
    }
    match OpenApiScope::from(scope) {
        OpenApiScope::Custom(_) => Err(astrbot_core::AstrbotError::Pipeline(format!(
            "Unsupported API key scope: {scope}"
        ))),
        known => Ok(Some(known.as_str().to_string())),
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn generate_api_key_id() -> String {
    format!("key-{}", unix_nanos())
}

fn generate_api_key_secret() -> String {
    let mut bytes = [0_u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("abk_{}", URL_SAFE_NO_PAD.encode(bytes))
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn current_unix_timestamp() -> String {
    format!("unix:{}", current_unix_seconds())
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn api_keys_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "api key management state is not configured".to_string(),
        }),
    )
}

fn map_api_key_error(error: astrbot_core::AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

fn api_key_not_found() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "API key not found".to_string(),
        }),
    )
}

fn source_ok(data: Value) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": null,
        "data": data,
    }))
}

fn api_key_descriptors_to_source(api_keys: Vec<ManagementApiKeyDescriptor>) -> Value {
    Value::Array(
        api_keys
            .into_iter()
            .map(api_key_descriptor_to_source)
            .collect(),
    )
}

fn api_key_descriptor_to_source(key: ManagementApiKeyDescriptor) -> Value {
    json!({
        "key_id": key.key_id,
        "name": key.name,
        "key_prefix": key.key_prefix,
        "scopes": key.scopes,
        "created_by": key.created_by,
        "created_at": null,
        "updated_at": null,
        "last_used_at": key.last_used_at,
        "expires_at": key.expires_at,
        "revoked_at": key.revoked_at,
        "is_revoked": !key.active && !key.is_expired,
        "is_expired": key.is_expired,
        "active": key.active,
    })
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    use astrbot_storage::{ApiKeyRepository, InMemoryApiKeyRepository};

    use super::{
        ApiKeyAuthDecision, ApiKeyIssuer, ApiKeyRejectionReason, ManagementApiKeyState,
        OpenApiScope, OpenApiScopeSet, PresentedApiKey, authorize_api_key,
        extract_presented_api_key, hash_api_key, legacy_sha1_api_key_hash,
    };

    #[test]
    fn openapi_scope_set_canonicalizes_legacy_aliases() {
        let scopes = OpenApiScopeSet::from_strings(["management.read", "openapi.chat"]);

        assert!(scopes.allows_all(&[OpenApiScope::ManagementRead, OpenApiScope::Chat]));
        assert_eq!(
            scopes.to_strings(),
            vec!["management.read".to_string(), "chat".to_string()]
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

        assert_eq!(issued.record.key_prefix, "ak_test_secr");
        assert_eq!(issued.record.key_hash, hash_api_key("ak_test_secret"));
        assert_ne!(
            issued.record.key_hash,
            legacy_sha1_api_key_hash("ak_test_secret")
        );
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
        assert!(
            repository
                .api_key_by_hash(&hash_api_key("ak_test_secret"))
                .await
                .expect("api key should load")
                .expect("api key should exist")
                .last_used_at
                .is_some()
        );
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

    #[tokio::test]
    async fn api_key_authorizer_migrates_legacy_sha1_hashes() {
        let repository = InMemoryApiKeyRepository::new();
        let secret = "ak_legacy_secret";
        let legacy_record = astrbot_storage::ApiKeyRecord::new(
            "key-legacy",
            "Legacy",
            legacy_sha1_api_key_hash(secret),
            "ak_legacy_se",
            ["chat"],
            "admin",
        );
        repository
            .store_api_key(legacy_record)
            .await
            .expect("legacy api key should store");

        let decision = authorize_api_key(
            &repository,
            Some(&PresentedApiKey::new(secret)),
            &[OpenApiScope::Chat],
        )
        .await
        .expect("legacy api key should authorize");

        assert!(decision.is_allowed());
        assert!(
            repository
                .api_key_by_hash(&legacy_sha1_api_key_hash(secret))
                .await
                .expect("legacy api key lookup should run")
                .is_none()
        );
        let migrated = repository
            .api_key_by_hash(&hash_api_key(secret))
            .await
            .expect("migrated api key should load")
            .expect("migrated api key should exist");
        assert_eq!(migrated.key_id, "key-legacy");
        assert!(migrated.last_used_at.is_some());
    }

    #[tokio::test]
    async fn api_key_authorizer_rejects_expired_keys() {
        let repository = InMemoryApiKeyRepository::new();
        let issued = ApiKeyIssuer::issue(
            "key-expired",
            "Expired",
            "ak_expired_secret",
            OpenApiScopeSet::new([OpenApiScope::Chat]),
            "admin",
        );
        repository
            .store_api_key(issued.record.with_expires_at("unix:1"))
            .await
            .expect("expired api key should store");

        assert_eq!(
            authorize_api_key(
                &repository,
                Some(&PresentedApiKey::new("ak_expired_secret")),
                &[OpenApiScope::Chat],
            )
            .await
            .expect("api key should reject"),
            ApiKeyAuthDecision::Denied(ApiKeyRejectionReason::Expired)
        );
    }

    #[tokio::test]
    async fn management_api_key_state_rate_limits_authorization_attempts() {
        let repository = InMemoryApiKeyRepository::new();
        let issued = ApiKeyIssuer::issue(
            "key-rate",
            "Rate limited",
            "ak_rate_secret",
            OpenApiScopeSet::new([OpenApiScope::ManagementRead]),
            "admin",
        );
        repository
            .store_api_key(issued.record)
            .await
            .expect("api key should store");
        let state =
            ManagementApiKeyState::new(std::sync::Arc::new(repository)).with_rate_limit(1, 60);

        assert!(matches!(
            state
                .authorize_presented(
                    Some(&PresentedApiKey::new("ak_rate_secret")),
                    &[OpenApiScope::ManagementRead],
                )
                .await
                .expect("first attempt should run"),
            ApiKeyAuthDecision::Allowed(_)
        ));
        assert_eq!(
            state
                .authorize_presented(
                    Some(&PresentedApiKey::new("ak_rate_secret")),
                    &[OpenApiScope::ManagementRead],
                )
                .await
                .expect("second attempt should reject"),
            ApiKeyAuthDecision::Denied(ApiKeyRejectionReason::RateLimited)
        );
    }
}
