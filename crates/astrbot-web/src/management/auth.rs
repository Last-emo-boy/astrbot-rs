use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use astrbot_runtime::{RuntimeConfig, RuntimeConfigService, RuntimeDashboardAuthConfig};
use axum::{
    Json,
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use sha2::Sha256;

use crate::ErrorResponse;

use super::api_key::{
    ApiKeyAuthDecision, ApiKeyRejectionReason, ManagementApiKeyState, OpenApiScope,
    extract_presented_api_key,
};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct DashboardAuthPolicy {
    source: DashboardAuthSource,
    clock: Arc<dyn Fn() -> u64 + Send + Sync>,
    login_rate_limiter: Arc<LoginRateLimiter>,
}

impl std::fmt::Debug for DashboardAuthPolicy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DashboardAuthPolicy")
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
enum DashboardAuthSource {
    StaticToken(String),
    RuntimeConfig(RuntimeConfigService),
}

impl DashboardAuthPolicy {
    pub fn new(session_token: impl Into<String>) -> Self {
        Self {
            source: DashboardAuthSource::StaticToken(session_token.into()),
            clock: Arc::new(current_unix_seconds),
            login_rate_limiter: Arc::new(LoginRateLimiter::new(5, 60)),
        }
    }

    pub fn from_config_service(config_service: RuntimeConfigService) -> Self {
        Self {
            source: DashboardAuthSource::RuntimeConfig(config_service),
            clock: Arc::new(current_unix_seconds),
            login_rate_limiter: Arc::new(LoginRateLimiter::new(5, 60)),
        }
    }

    pub fn with_clock(mut self, clock: impl Fn() -> u64 + Send + Sync + 'static) -> Self {
        self.clock = Arc::new(clock);
        self
    }

    pub fn with_login_rate_limit(mut self, max_attempts: u32, window_seconds: u64) -> Self {
        self.login_rate_limiter = Arc::new(LoginRateLimiter::new(max_attempts, window_seconds));
        self
    }

    pub fn authenticate(&self, presented_token: Option<&str>) -> DashboardAuthDecision {
        match &self.source {
            DashboardAuthSource::StaticToken(session_token) => match presented_token {
                Some(token) if token == session_token => {
                    DashboardAuthDecision::Allowed(ManagementActor::dashboard("dashboard"))
                }
                Some(_) => DashboardAuthDecision::Denied(AuthRejectionReason::InvalidToken),
                None => DashboardAuthDecision::Denied(AuthRejectionReason::MissingToken),
            },
            DashboardAuthSource::RuntimeConfig(config_service) => {
                let Some(token) = presented_token else {
                    return DashboardAuthDecision::Denied(AuthRejectionReason::MissingToken);
                };
                match config_service.read_config() {
                    Ok(config) => match validate_dashboard_token(
                        token,
                        &config.dashboard_auth,
                        (self.clock)(),
                    ) {
                        Ok(payload) => DashboardAuthDecision::Allowed(ManagementActor::dashboard(
                            payload.username,
                        )),
                        Err(reason) => DashboardAuthDecision::Denied(reason),
                    },
                    Err(_) => DashboardAuthDecision::Denied(AuthRejectionReason::InvalidToken),
                }
            }
        }
    }

    fn login(&self, request: DashboardLoginRequest) -> Result<DashboardLoginData, AuthRouteError> {
        let config = self.runtime_config()?;
        let dashboard = &config.dashboard_auth;
        let now = (self.clock)();
        let username_key = request.username.trim().to_ascii_lowercase();
        if self.login_rate_limiter.is_blocked(&username_key, now) {
            return Err(AuthRouteError::TooManyRequests(
                "too many dashboard login attempts; retry later".to_string(),
            ));
        }
        if request.username != dashboard.username || request.password != dashboard.password {
            self.login_rate_limiter.record_failure(&username_key, now);
            return Err(AuthRouteError::Unauthorized("用户名或密码错误".to_string()));
        }
        self.login_rate_limiter.reset(&username_key);
        let change_pwd_hint = dashboard.is_default_credential();
        let mut security_warnings = Vec::new();
        if change_pwd_hint {
            security_warnings.push(
                "Default dashboard credentials are still enabled; change the username or password immediately."
                    .to_string(),
            );
        }
        Ok(DashboardLoginData {
            token: issue_dashboard_token(dashboard, (self.clock)())?,
            username: dashboard.username.clone(),
            change_pwd_hint,
            security_warnings,
        })
    }

    fn edit_account(&self, request: DashboardAccountEditRequest) -> Result<(), AuthRouteError> {
        let mut config = self.runtime_config()?;
        if request.password != config.dashboard_auth.password {
            return Err(AuthRouteError::BadRequest("原密码错误".to_string()));
        }

        let new_password = non_empty(request.new_password);
        let new_username = non_empty(request.new_username);
        if new_password.is_none() && new_username.is_none() {
            return Err(AuthRouteError::BadRequest(
                "新用户名和新密码不能同时为空".to_string(),
            ));
        }
        if let Some(new_password) = new_password {
            if request.confirm_password.as_deref() != Some(new_password.as_str()) {
                return Err(AuthRouteError::BadRequest(
                    "两次输入的新密码不一致".to_string(),
                ));
            }
            config.dashboard_auth.password = new_password;
        }
        if let Some(new_username) = new_username {
            config.dashboard_auth.username = new_username;
        }

        self.save_runtime_config(config)?;
        Ok(())
    }

    fn runtime_config(&self) -> Result<RuntimeConfig, AuthRouteError> {
        match &self.source {
            DashboardAuthSource::RuntimeConfig(config_service) => config_service
                .read_config()
                .map_err(|err| AuthRouteError::Internal(format!("read runtime config: {err}"))),
            DashboardAuthSource::StaticToken(_) => Err(AuthRouteError::Internal(
                "dashboard auth routes require runtime config".to_string(),
            )),
        }
    }

    fn save_runtime_config(&self, config: RuntimeConfig) -> Result<(), AuthRouteError> {
        match &self.source {
            DashboardAuthSource::RuntimeConfig(config_service) => {
                let value = serde_json::to_value(config).map_err(|err| {
                    AuthRouteError::Internal(format!("serialize runtime config: {err}"))
                })?;
                config_service.save_update_value(value).map_err(|err| {
                    AuthRouteError::Internal(format!("save runtime config: {err}"))
                })?;
                Ok(())
            }
            DashboardAuthSource::StaticToken(_) => Err(AuthRouteError::Internal(
                "dashboard auth routes require runtime config".to_string(),
            )),
        }
    }
}

impl PartialEq for DashboardAuthPolicy {
    fn eq(&self, other: &Self) -> bool {
        match (&self.source, &other.source) {
            (DashboardAuthSource::StaticToken(left), DashboardAuthSource::StaticToken(right)) => {
                left == right
            }
            _ => false,
        }
    }
}

impl Eq for DashboardAuthPolicy {}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementActor {
    pub actor_id: String,
    pub auth_method: String,
    pub scopes: Vec<String>,
}

impl ManagementActor {
    fn dashboard(username: impl Into<String>) -> Self {
        Self {
            actor_id: username.into(),
            auth_method: "dashboard".to_string(),
            scopes: vec![
                OpenApiScope::ManagementRead.as_str().to_string(),
                OpenApiScope::ManagementWrite.as_str().to_string(),
            ],
        }
    }

    fn api_key(record: &astrbot_storage::ApiKeyRecord) -> Self {
        Self {
            actor_id: record.key_id.clone(),
            auth_method: "api_key".to_string(),
            scopes: record.scopes.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DashboardAuthDecision {
    Allowed(ManagementActor),
    Denied(AuthRejectionReason),
}

impl DashboardAuthDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed(_))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthRejectionReason {
    MissingToken,
    InvalidToken,
    ExpiredToken,
}

impl AuthRejectionReason {
    fn message(self) -> &'static str {
        match self {
            Self::MissingToken => "missing management authorization token",
            Self::InvalidToken => "invalid management authorization token",
            Self::ExpiredToken => "expired management authorization token",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ManagementAuthState {
    policy: DashboardAuthPolicy,
    api_keys: Option<ManagementApiKeyState>,
    audit_log: Option<Arc<ManagementAuditFileStore>>,
    csrf: ManagementCsrfPolicy,
}

impl ManagementAuthState {
    pub fn new(policy: DashboardAuthPolicy) -> Self {
        Self {
            policy,
            api_keys: None,
            audit_log: None,
            csrf: ManagementCsrfPolicy::default(),
        }
    }

    pub fn from_config_service(config_service: RuntimeConfigService) -> Self {
        Self::new(DashboardAuthPolicy::from_config_service(config_service))
    }

    pub fn policy(&self) -> &DashboardAuthPolicy {
        &self.policy
    }

    pub fn with_api_keys(mut self, api_keys: Option<ManagementApiKeyState>) -> Self {
        self.api_keys = api_keys;
        self
    }

    pub fn with_audit_log_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.audit_log = Some(Arc::new(ManagementAuditFileStore::new(path.into())));
        self
    }

    pub fn with_allowed_origin(mut self, origin: impl Into<String>) -> Self {
        self.csrf.allowed_origins.push(origin.into());
        self
    }

    async fn authorize_management_request(
        &self,
        headers: &HeaderMap,
        method: &Method,
        path: &str,
    ) -> Result<ManagementActor, AuthRouteError> {
        self.csrf.enforce(headers, method)?;
        match self
            .policy()
            .authenticate(extract_bearer_token(headers).as_deref())
        {
            DashboardAuthDecision::Allowed(actor) => Ok(actor),
            DashboardAuthDecision::Denied(token_reason) => {
                let Some(api_keys) = self.api_keys.as_ref() else {
                    return Err(AuthRouteError::Unauthorized(
                        token_reason.message().to_string(),
                    ));
                };
                let presented = extract_presented_api_key(headers);
                let policy = route_security_policy(method, path);
                match api_keys
                    .authorize_presented(presented.as_ref(), &policy.required_scopes)
                    .await
                    .map_err(|error| AuthRouteError::Internal(error.to_string()))?
                {
                    ApiKeyAuthDecision::Allowed(record) => Ok(ManagementActor::api_key(&record)),
                    ApiKeyAuthDecision::Denied(reason) => Err(api_key_route_error(reason)),
                }
            }
        }
    }

    fn audit(
        &self,
        request_id: String,
        actor: ManagementActor,
        method: &Method,
        path: &str,
        action: &str,
        status: StatusCode,
    ) {
        let Some(store) = &self.audit_log else {
            return;
        };
        let entry = ManagementAuditEntry {
            request_id,
            actor_id: actor.actor_id,
            auth_method: actor.auth_method,
            method: method.to_string(),
            path: path.to_string(),
            action: action.to_string(),
            status: status.as_u16(),
            result: if status.is_success() {
                "success".to_string()
            } else {
                "failure".to_string()
            },
            occurred_at_unix: current_unix_seconds(),
        };
        let _ = store.append(&entry);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementAuditEntry {
    pub request_id: String,
    pub actor_id: String,
    pub auth_method: String,
    pub method: String,
    pub path: String,
    pub action: String,
    pub status: u16,
    pub result: String,
    pub occurred_at_unix: u64,
}

#[derive(Clone, Debug)]
pub struct ManagementAuditFileStore {
    path: PathBuf,
}

impl ManagementAuditFileStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn append(&self, entry: &ManagementAuditEntry) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("audit log create {}: {error}", parent.display()))?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|error| format!("audit log open {}: {error}", self.path.display()))?;
        let line = serde_json::to_string(entry)
            .map_err(|error| format!("audit log serialize: {error}"))?;
        writeln!(file, "{line}")
            .map_err(|error| format!("audit log write {}: {error}", self.path.display()))
    }
}

#[derive(Clone, Debug, Default)]
struct ManagementCsrfPolicy {
    allowed_origins: Vec<String>,
}

impl ManagementCsrfPolicy {
    fn enforce(&self, headers: &HeaderMap, method: &Method) -> Result<(), AuthRouteError> {
        if !is_mutating_method(method) {
            return Ok(());
        }
        let Some(origin) = headers
            .get("origin")
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(());
        };
        if self.allowed_origins.iter().any(|allowed| allowed == origin) {
            return Ok(());
        }
        let same_origin = headers
            .get("host")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|host| origin_matches_host(origin, host));
        if same_origin {
            Ok(())
        } else {
            Err(AuthRouteError::Forbidden(
                "cross-origin management mutation is not allowed".to_string(),
            ))
        }
    }
}

#[derive(Clone, Debug)]
struct ManagementRouteSecurityPolicy {
    required_scopes: Vec<OpenApiScope>,
    audit_action: Option<&'static str>,
}

fn route_security_policy(method: &Method, path: &str) -> ManagementRouteSecurityPolicy {
    let required_scopes = if is_read_method(method) {
        vec![OpenApiScope::ManagementRead]
    } else {
        vec![OpenApiScope::ManagementWrite]
    };
    ManagementRouteSecurityPolicy {
        required_scopes,
        audit_action: audit_action_for(method, path),
    }
}

fn audit_action_for(method: &Method, path: &str) -> Option<&'static str> {
    if !is_mutating_method(method) {
        return None;
    }
    if path.starts_with("/api/auth/account") {
        return Some("auth.account");
    }
    if path.starts_with("/api/management/backup") || path.starts_with("/api/backup") {
        return Some("backup");
    }
    if path.starts_with("/api/management/update")
        || path.starts_with("/api/update")
        || path.starts_with("/api/stat/restart-core")
    {
        return Some("update");
    }
    if path.starts_with("/api/management/config")
        || path.starts_with("/api/config")
        || path.starts_with("/api/management/api-keys")
        || path.starts_with("/api/apikey")
        || path.starts_with("/api/v1/apikeys")
    {
        return Some("config_or_access");
    }
    None
}

fn is_read_method(method: &Method) -> bool {
    matches!(method, &Method::GET | &Method::HEAD | &Method::OPTIONS)
}

fn is_mutating_method(method: &Method) -> bool {
    matches!(
        method,
        &Method::POST | &Method::PUT | &Method::PATCH | &Method::DELETE
    )
}

fn origin_matches_host(origin: &str, host: &str) -> bool {
    let Some(origin_host) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    origin_host.trim_end_matches('/') == host
}

fn api_key_route_error(reason: ApiKeyRejectionReason) -> AuthRouteError {
    match reason {
        ApiKeyRejectionReason::MissingKey | ApiKeyRejectionReason::UnknownKey => {
            AuthRouteError::Unauthorized(api_key_rejection_message(reason).to_string())
        }
        ApiKeyRejectionReason::Revoked
        | ApiKeyRejectionReason::Expired
        | ApiKeyRejectionReason::MissingScope => {
            AuthRouteError::Forbidden(api_key_rejection_message(reason).to_string())
        }
        ApiKeyRejectionReason::RateLimited => {
            AuthRouteError::TooManyRequests(api_key_rejection_message(reason).to_string())
        }
    }
}

fn api_key_rejection_message(reason: ApiKeyRejectionReason) -> &'static str {
    match reason {
        ApiKeyRejectionReason::MissingKey => "management api key is required",
        ApiKeyRejectionReason::UnknownKey => "management api key is unknown",
        ApiKeyRejectionReason::Revoked => "management api key is revoked",
        ApiKeyRejectionReason::Expired => "management api key is expired",
        ApiKeyRejectionReason::MissingScope => "management scope is required",
        ApiKeyRejectionReason::RateLimited => "management api key rate limit exceeded",
    }
}

fn request_id_from_headers(headers: &HeaderMap) -> String {
    headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("req-{}", unix_nanos()))
}

#[derive(Debug)]
struct LoginRateLimiter {
    max_attempts: u32,
    window_seconds: u64,
    attempts: Mutex<BTreeMap<String, LoginRateLimitBucket>>,
}

#[derive(Clone, Copy, Debug)]
struct LoginRateLimitBucket {
    window_started_at: u64,
    failures: u32,
}

impl LoginRateLimiter {
    fn new(max_attempts: u32, window_seconds: u64) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
            window_seconds: window_seconds.max(1),
            attempts: Mutex::new(BTreeMap::new()),
        }
    }

    fn is_blocked(&self, identifier: &str, now: u64) -> bool {
        let Ok(mut attempts) = self.attempts.lock() else {
            return true;
        };
        let Some(bucket) = attempts.get_mut(identifier) else {
            return false;
        };
        if now.saturating_sub(bucket.window_started_at) >= self.window_seconds {
            attempts.remove(identifier);
            return false;
        }
        bucket.failures >= self.max_attempts
    }

    fn record_failure(&self, identifier: &str, now: u64) {
        let Ok(mut attempts) = self.attempts.lock() else {
            return;
        };
        let bucket = attempts
            .entry(identifier.to_string())
            .or_insert(LoginRateLimitBucket {
                window_started_at: now,
                failures: 0,
            });
        if now.saturating_sub(bucket.window_started_at) >= self.window_seconds {
            *bucket = LoginRateLimitBucket {
                window_started_at: now,
                failures: 0,
            };
        }
        bucket.failures = bucket.failures.saturating_add(1);
    }

    fn reset(&self, identifier: &str) {
        if let Ok(mut attempts) = self.attempts.lock() {
            attempts.remove(identifier);
        }
    }
}

pub async fn require_management_auth(
    State(auth): State<ManagementAuthState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let request_id = request_id_from_headers(request.headers());
    let policy = route_security_policy(&method, &path);
    let actor = match auth
        .authorize_management_request(request.headers(), &method, &path)
        .await
    {
        Ok(actor) => actor,
        Err(error) => {
            let status = error.status_code();
            if let Some(action) = policy.audit_action {
                auth.audit(
                    request_id,
                    ManagementActor {
                        actor_id: "anonymous".to_string(),
                        auth_method: "none".to_string(),
                        scopes: Vec::new(),
                    },
                    &method,
                    &path,
                    action,
                    status,
                );
            }
            return error.into_error_response();
        }
    };
    request.extensions_mut().insert(actor.clone());
    let response = next.run(request).await;
    if let Some(action) = policy.audit_action {
        auth.audit(request_id, actor, &method, &path, action, response.status());
    }
    response
}

pub async fn login(
    State(auth): State<ManagementAuthState>,
    Json(request): Json<DashboardLoginRequest>,
) -> Response {
    match auth.policy().login(request) {
        Ok(data) => Json(DashboardAuthResponse::ok(data, None)).into_response(),
        Err(err) => err.into_response(),
    }
}

pub async fn edit_account(
    State(auth): State<ManagementAuthState>,
    headers: HeaderMap,
    Json(request): Json<DashboardAccountEditRequest>,
) -> Response {
    let method = Method::POST;
    let path = "/api/auth/account/edit";
    let request_id = request_id_from_headers(&headers);
    if let Err(error) = auth.csrf.enforce(&headers, &method) {
        let status = error.status_code();
        auth.audit(
            request_id,
            ManagementActor {
                actor_id: "anonymous".to_string(),
                auth_method: "none".to_string(),
                scopes: Vec::new(),
            },
            &method,
            path,
            "auth.account",
            status,
        );
        return error.into_response();
    }
    let actor = match auth
        .policy()
        .authenticate(extract_bearer_token(&headers).as_deref())
    {
        DashboardAuthDecision::Allowed(actor) => actor,
        DashboardAuthDecision::Denied(reason) => {
            let error = AuthRouteError::Unauthorized(reason.message().to_string());
            let status = error.status_code();
            auth.audit(
                request_id,
                ManagementActor {
                    actor_id: "anonymous".to_string(),
                    auth_method: "none".to_string(),
                    scopes: Vec::new(),
                },
                &method,
                path,
                "auth.account",
                status,
            );
            return error.into_response();
        }
    };

    let response = match auth.policy().edit_account(request) {
        Ok(()) => Json(DashboardAuthResponse::ok(json!({}), Some("修改成功"))).into_response(),
        Err(err) => err.into_response(),
    };
    auth.audit(
        request_id,
        actor,
        &method,
        path,
        "auth.account",
        response.status(),
    );
    response
}

pub fn extract_bearer_token(headers: &header::HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    let token = value.strip_prefix("Bearer ")?;
    let token = token.trim();
    (!token.is_empty()).then(|| token.to_string())
}

#[derive(Clone, Debug, Deserialize)]
pub struct DashboardLoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashboardLoginData {
    pub token: String,
    pub username: String,
    pub change_pwd_hint: bool,
    #[serde(default)]
    pub security_warnings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DashboardAccountEditRequest {
    pub password: String,
    #[serde(default)]
    pub new_password: Option<String>,
    #[serde(default)]
    pub confirm_password: Option<String>,
    #[serde(default)]
    pub new_username: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashboardAuthResponse<T> {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub data: T,
}

impl<T> DashboardAuthResponse<T> {
    fn ok(data: T, message: Option<&str>) -> Self {
        Self {
            status: "ok".to_string(),
            message: message.map(str::to_string),
            data,
        }
    }
}

impl DashboardAuthResponse<Value> {
    fn error(message: impl Into<String>) -> Self {
        Self {
            status: "error".to_string(),
            message: Some(message.into()),
            data: json!({}),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DashboardTokenPayload {
    username: String,
    exp: u64,
    credential_revision: String,
}

#[derive(Debug)]
enum AuthRouteError {
    Unauthorized(String),
    Forbidden(String),
    BadRequest(String),
    TooManyRequests(String),
    Internal(String),
}

impl AuthRouteError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn into_response(self) -> Response {
        let status = self.status_code();
        let message = self.into_message();
        (status, Json(DashboardAuthResponse::error(message))).into_response()
    }

    fn into_error_response(self) -> Response {
        let status = self.status_code();
        let message = self.into_message();
        (status, Json(ErrorResponse { error: message })).into_response()
    }

    fn into_message(self) -> String {
        let message = match self {
            Self::Unauthorized(message)
            | Self::Forbidden(message)
            | Self::BadRequest(message)
            | Self::TooManyRequests(message)
            | Self::Internal(message) => message,
        };
        message
    }
}

fn issue_dashboard_token(
    config: &RuntimeDashboardAuthConfig,
    now: u64,
) -> Result<String, AuthRouteError> {
    let secret = required_jwt_secret(config)?;
    let header = json!({"alg": "HS256", "typ": "JWT"});
    let payload = DashboardTokenPayload {
        username: config.username.clone(),
        exp: now.saturating_add(config.token_ttl_seconds),
        credential_revision: credential_revision(config),
    };
    let header = encode_json(&header)?;
    let payload = encode_json(&payload)?;
    let signing_input = format!("{header}.{payload}");
    let signature = sign_token(secret, &signing_input)?;
    Ok(format!("{signing_input}.{signature}"))
}

fn validate_dashboard_token(
    token: &str,
    config: &RuntimeDashboardAuthConfig,
    now: u64,
) -> Result<DashboardTokenPayload, AuthRejectionReason> {
    let secret = required_jwt_secret(config).map_err(|_| AuthRejectionReason::InvalidToken)?;
    let mut parts = token.split('.');
    let (Some(header), Some(payload), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(AuthRejectionReason::InvalidToken);
    };
    let signing_input = format!("{header}.{payload}");
    verify_signature(secret, &signing_input, signature)?;
    let payload = decode_payload(payload)?;
    if now >= payload.exp {
        return Err(AuthRejectionReason::ExpiredToken);
    }
    if payload.username != config.username
        || payload.credential_revision != credential_revision(config)
    {
        return Err(AuthRejectionReason::InvalidToken);
    }
    Ok(payload)
}

fn encode_json<T: Serialize>(value: &T) -> Result<String, AuthRouteError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|err| AuthRouteError::Internal(format!("encode auth token: {err}")))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn decode_payload(payload: &str) -> Result<DashboardTokenPayload, AuthRejectionReason> {
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| AuthRejectionReason::InvalidToken)?;
    serde_json::from_slice(&bytes).map_err(|_| AuthRejectionReason::InvalidToken)
}

fn sign_token(secret: &str, signing_input: &str) -> Result<String, AuthRouteError> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|err| AuthRouteError::Internal(format!("initialize token signer: {err}")))?;
    mac.update(signing_input.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}

fn verify_signature(
    secret: &str,
    signing_input: &str,
    signature: &str,
) -> Result<(), AuthRejectionReason> {
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| AuthRejectionReason::InvalidToken)?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| AuthRejectionReason::InvalidToken)?;
    mac.update(signing_input.as_bytes());
    mac.verify_slice(&signature)
        .map_err(|_| AuthRejectionReason::InvalidToken)
}

fn credential_revision(config: &RuntimeDashboardAuthConfig) -> String {
    let mut hasher = Sha1::new();
    hasher.update(config.username.as_bytes());
    hasher.update(b":");
    hasher.update(config.password.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn required_jwt_secret(config: &RuntimeDashboardAuthConfig) -> Result<&str, AuthRouteError> {
    let secret = config.jwt_secret.trim();
    if secret.is_empty() {
        return Err(AuthRouteError::Internal(
            "JWT secret is not set in the cmd_config.".to_string(),
        ));
    }
    Ok(secret)
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header};

    use astrbot_runtime::{RuntimeConfig, RuntimeConfigService};

    use super::{
        AuthRejectionReason, DashboardAuthDecision, DashboardAuthPolicy, extract_bearer_token,
    };

    #[test]
    fn dashboard_auth_policy_accepts_matching_bearer_token() {
        let policy = DashboardAuthPolicy::new("secret");

        assert!(policy.authenticate(Some("secret")).is_allowed());
        assert!(matches!(
            policy.authenticate(Some("wrong")),
            DashboardAuthDecision::Denied(AuthRejectionReason::InvalidToken)
        ));
        assert!(matches!(
            policy.authenticate(None),
            DashboardAuthDecision::Denied(AuthRejectionReason::MissingToken)
        ));
    }

    #[test]
    fn bearer_token_extractor_requires_authorization_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer secret"),
        );

        assert_eq!(extract_bearer_token(&headers).as_deref(), Some("secret"));
    }

    #[test]
    fn runtime_policy_rejects_expired_tokens() {
        let path = std::env::temp_dir().join(format!(
            "astrbot-web-auth-policy-{}-expired.json",
            std::process::id()
        ));
        let mut config = RuntimeConfig::default();
        config.dashboard_auth.token_ttl_seconds = 0;
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&config).expect("auth config should serialize"),
        )
        .expect("auth fixture should write");
        let policy = DashboardAuthPolicy::from_config_service(RuntimeConfigService::new(&path))
            .with_clock(|| 1);
        let token = policy
            .login(super::DashboardLoginRequest {
                username: "astrbot".to_string(),
                password: "77b90590a8945a7d36c963981a307dc9".to_string(),
            })
            .expect("login should issue token")
            .token;

        assert!(matches!(
            policy.authenticate(Some(&token)),
            DashboardAuthDecision::Denied(AuthRejectionReason::ExpiredToken)
        ));
        let _ = std::fs::remove_file(path);
    }
}
