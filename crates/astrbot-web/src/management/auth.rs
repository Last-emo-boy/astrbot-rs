use axum::{
    body::Body,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::ErrorResponse;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DashboardAuthPolicy {
    session_token: String,
}

impl DashboardAuthPolicy {
    pub fn new(session_token: impl Into<String>) -> Self {
        Self {
            session_token: session_token.into(),
        }
    }

    pub fn authenticate(&self, presented_token: Option<&str>) -> DashboardAuthDecision {
        match presented_token {
            Some(token) if token == self.session_token => DashboardAuthDecision::Allowed,
            Some(_) => DashboardAuthDecision::Denied(AuthRejectionReason::InvalidToken),
            None => DashboardAuthDecision::Denied(AuthRejectionReason::MissingToken),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DashboardAuthDecision {
    Allowed,
    Denied(AuthRejectionReason),
}

impl DashboardAuthDecision {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthRejectionReason {
    MissingToken,
    InvalidToken,
}

impl AuthRejectionReason {
    fn message(self) -> &'static str {
        match self {
            Self::MissingToken => "missing management authorization token",
            Self::InvalidToken => "invalid management authorization token",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ManagementAuthState {
    policy: DashboardAuthPolicy,
}

impl ManagementAuthState {
    pub fn new(policy: DashboardAuthPolicy) -> Self {
        Self { policy }
    }

    pub fn policy(&self) -> &DashboardAuthPolicy {
        &self.policy
    }
}

pub async fn require_management_auth(
    State(auth): State<ManagementAuthState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    match auth
        .policy()
        .authenticate(extract_bearer_token(request.headers()).as_deref())
    {
        DashboardAuthDecision::Allowed => next.run(request).await,
        DashboardAuthDecision::Denied(reason) => (
            StatusCode::UNAUTHORIZED,
            axum::Json(ErrorResponse {
                error: reason.message().to_string(),
            }),
        )
            .into_response(),
    }
}

pub fn extract_bearer_token(headers: &header::HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?.trim();
    let token = value.strip_prefix("Bearer ")?;
    let token = token.trim();
    (!token.is_empty()).then(|| token.to_string())
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header};

    use super::{DashboardAuthDecision, DashboardAuthPolicy, extract_bearer_token};

    #[test]
    fn dashboard_auth_policy_accepts_matching_bearer_token() {
        let policy = DashboardAuthPolicy::new("secret");

        assert!(policy.authenticate(Some("secret")).is_allowed());
        assert!(matches!(
            policy.authenticate(Some("wrong")),
            DashboardAuthDecision::Denied(_)
        ));
        assert!(matches!(
            policy.authenticate(None),
            DashboardAuthDecision::Denied(_)
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
}
