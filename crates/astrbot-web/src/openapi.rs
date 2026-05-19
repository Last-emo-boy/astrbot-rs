use std::sync::Arc;

use astrbot_platform::WebChatPlatform;
use axum::{
    Json, Router,
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::error::map_submit_error;
use crate::management::{
    ApiKeyAuthDecision, ApiKeyRejectionReason, ManagementApiKeyState, extract_presented_api_key,
};
use crate::{
    ErrorResponse, OpenApiChatAuthContext, OpenApiChatGateway, OpenApiChatGatewayError,
    OpenApiChatMessageRequest, OpenApiChatResponseMode, OpenApiChatSubscriptionPlan,
    RealtimeControlState, RealtimeElicitationCatalogResponse, RealtimeElicitationCreateRequest,
    RealtimeElicitationRespondRequest, RealtimeStopRequest, RealtimeStopResponse,
    RealtimeSubscriptionCatalogResponse, required_openapi_chat_scopes,
};

#[derive(Clone)]
struct OpenApiChatHttpState {
    webchat: Arc<WebChatPlatform>,
    api_keys: Option<ManagementApiKeyState>,
    gateway: OpenApiChatGateway,
    realtime: RealtimeControlState,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenApiChatHttpResponse {
    pub accepted: bool,
    pub event_id: String,
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub response_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription: Option<OpenApiChatHttpSubscription>,
    pub key_id: String,
    pub key_prefix: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenApiChatHttpSubscription {
    pub conversation_id: String,
    pub request_id: String,
}

impl From<OpenApiChatSubscriptionPlan> for OpenApiChatHttpSubscription {
    fn from(plan: OpenApiChatSubscriptionPlan) -> Self {
        Self {
            conversation_id: plan.conversation_id,
            request_id: plan.request_id,
        }
    }
}

pub fn openapi_chat_router(
    webchat: Arc<WebChatPlatform>,
    api_keys: Option<ManagementApiKeyState>,
) -> Router {
    openapi_chat_router_with_realtime(webchat, api_keys, RealtimeControlState::new())
}

pub fn openapi_chat_router_with_realtime(
    webchat: Arc<WebChatPlatform>,
    api_keys: Option<ManagementApiKeyState>,
    realtime: RealtimeControlState,
) -> Router {
    Router::new()
        .route("/api/openapi/chat", post(chat))
        .route("/api/v1/chat", post(chat))
        .route("/api/openapi/chat/subscriptions", get(subscriptions))
        .route("/api/v1/chat/sessions", get(v1_chat_sessions))
        .route(
            "/api/openapi/chat/subscriptions/{request_id}",
            get(subscription),
        )
        .route("/api/v1/chat/ws", get(v1_chat_ws))
        .route("/api/openapi/chat/stop", post(stop_chat))
        .route("/api/openapi/elicitation", get(elicitations))
        .route("/api/openapi/elicitation", post(create_elicitation))
        .route(
            "/api/openapi/elicitation/respond",
            post(respond_elicitation),
        )
        .with_state(OpenApiChatHttpState {
            webchat,
            api_keys,
            gateway: OpenApiChatGateway::default(),
            realtime,
        })
}

async fn chat(
    State(state): State<OpenApiChatHttpState>,
    headers: HeaderMap,
    Json(request): Json<OpenApiChatMessageRequest>,
) -> Result<Json<OpenApiChatHttpResponse>, (StatusCode, Json<ErrorResponse>)> {
    let auth_record = authorize_openapi_chat(&state, &headers).await?;

    let plan = state
        .gateway
        .prepare_enqueue(
            OpenApiChatAuthContext::from_api_key_record(&auth_record),
            request,
        )
        .map_err(map_gateway_error)?;
    let response_mode = response_mode_label(plan.request.response_mode).to_string();
    let conversation_id = plan.request.conversation_id.clone();
    let sender_id = plan.request.sender_id.clone();
    let request_id = plan.request.request_id.clone();
    let subscription_plan = plan.subscription.clone();
    let subscription = subscription_plan
        .clone()
        .map(OpenApiChatHttpSubscription::from);
    let key_id = plan.request.auth.key_id.clone();
    let key_prefix = plan.request.auth.key_prefix.clone();
    let event_id = state
        .webchat
        .submit_chain(conversation_id.clone(), sender_id, plan.request.message)
        .await
        .map_err(map_submit_error)?;
    if let Some(subscription_plan) = subscription_plan {
        state
            .realtime
            .record_subscription(subscription_plan, event_id.clone(), key_id.clone())
            .map_err(realtime_error)?;
    }

    Ok(Json(OpenApiChatHttpResponse {
        accepted: true,
        event_id,
        conversation_id,
        request_id,
        response_mode,
        subscription,
        key_id,
        key_prefix,
    }))
}

async fn subscriptions(
    State(state): State<OpenApiChatHttpState>,
    headers: HeaderMap,
) -> Result<Json<RealtimeSubscriptionCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let _auth_record = authorize_openapi_chat(&state, &headers).await?;
    Ok(Json(RealtimeSubscriptionCatalogResponse {
        subscriptions: state.realtime.subscriptions().map_err(realtime_error)?,
    }))
}

async fn subscription(
    State(state): State<OpenApiChatHttpState>,
    headers: HeaderMap,
    Path(request_id): Path<String>,
) -> Result<Json<crate::RealtimeChatSubscriptionRecord>, (StatusCode, Json<ErrorResponse>)> {
    let _auth_record = authorize_openapi_chat(&state, &headers).await?;
    let Some(subscription) = state
        .realtime
        .subscription(&request_id)
        .map_err(realtime_error)?
    else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "realtime subscription is not found".to_string(),
            }),
        ));
    };
    Ok(Json(subscription))
}

async fn stop_chat(
    State(state): State<OpenApiChatHttpState>,
    headers: HeaderMap,
    Json(request): Json<RealtimeStopRequest>,
) -> Result<Json<RealtimeStopResponse>, (StatusCode, Json<ErrorResponse>)> {
    let _auth_record = authorize_openapi_chat(&state, &headers).await?;
    Ok(Json(
        state
            .realtime
            .request_stop(request)
            .map_err(realtime_error)?,
    ))
}

async fn elicitations(
    State(state): State<OpenApiChatHttpState>,
    headers: HeaderMap,
) -> Result<Json<RealtimeElicitationCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let _auth_record = authorize_openapi_chat(&state, &headers).await?;
    Ok(Json(RealtimeElicitationCatalogResponse {
        elicitations: state.realtime.elicitations().map_err(realtime_error)?,
    }))
}

async fn v1_chat_sessions(
    State(state): State<OpenApiChatHttpState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let _auth_record = authorize_openapi_chat(&state, &headers).await?;
    Ok(Json(serde_json::json!({
        "status": "ok",
        "message": "",
        "data": {
            "sessions": state.realtime.subscriptions().map_err(realtime_error)?
        }
    })))
}

async fn v1_chat_ws(
    State(state): State<OpenApiChatHttpState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    match authorize_openapi_chat(&state, &headers).await {
        Ok(record) => {
            let key_id = record.key_id;
            ws.on_upgrade(move |socket| handle_v1_chat_socket(socket, key_id))
        }
        Err(error) => error.into_response(),
    }
}

async fn handle_v1_chat_socket(mut socket: WebSocket, key_id: String) {
    let _ = socket
        .send(Message::Text(
            serde_json::json!({ "type": "ready", "key_id": key_id })
                .to_string()
                .into(),
        ))
        .await;
    while let Some(Ok(message)) = socket.recv().await {
        match message {
            Message::Text(text) => {
                let payload = serde_json::from_str::<serde_json::Value>(&text)
                    .unwrap_or_else(|_| serde_json::json!({ "raw": text.to_string() }));
                let _ = socket
                    .send(Message::Text(
                        serde_json::json!({
                            "type": "ack",
                            "payload": payload
                        })
                        .to_string()
                        .into(),
                    ))
                    .await;
            }
            Message::Close(_) => break,
            _ => {}
        }
    }
}

async fn create_elicitation(
    State(state): State<OpenApiChatHttpState>,
    headers: HeaderMap,
    Json(request): Json<RealtimeElicitationCreateRequest>,
) -> Result<Json<crate::RealtimeElicitationRecord>, (StatusCode, Json<ErrorResponse>)> {
    let _auth_record = authorize_openapi_chat(&state, &headers).await?;
    Ok(Json(
        state
            .realtime
            .create_elicitation(request)
            .map_err(realtime_error)?,
    ))
}

async fn respond_elicitation(
    State(state): State<OpenApiChatHttpState>,
    headers: HeaderMap,
    Json(request): Json<RealtimeElicitationRespondRequest>,
) -> Result<Json<crate::RealtimeElicitationRecord>, (StatusCode, Json<ErrorResponse>)> {
    let _auth_record = authorize_openapi_chat(&state, &headers).await?;
    let Some(record) = state
        .realtime
        .respond_elicitation(request)
        .map_err(realtime_error)?
    else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "elicitation request is not found".to_string(),
            }),
        ));
    };
    Ok(Json(record))
}

async fn authorize_openapi_chat(
    state: &OpenApiChatHttpState,
    headers: &HeaderMap,
) -> Result<astrbot_storage::ApiKeyRecord, (StatusCode, Json<ErrorResponse>)> {
    let api_keys = state.api_keys.as_ref().ok_or_else(api_keys_unavailable)?;
    let presented = extract_presented_api_key(headers);
    match api_keys
        .authorize_presented(presented.as_ref(), &required_openapi_chat_scopes())
        .await
        .map_err(map_api_key_error)?
    {
        ApiKeyAuthDecision::Allowed(record) => Ok(record),
        ApiKeyAuthDecision::Denied(reason) => Err(map_api_key_rejection(reason)),
    }
}

fn response_mode_label(mode: OpenApiChatResponseMode) -> &'static str {
    match mode {
        OpenApiChatResponseMode::Blocking => "blocking",
        OpenApiChatResponseMode::Streaming => "streaming",
    }
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
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

fn map_api_key_rejection(reason: ApiKeyRejectionReason) -> (StatusCode, Json<ErrorResponse>) {
    let status = match reason {
        ApiKeyRejectionReason::MissingKey | ApiKeyRejectionReason::UnknownKey => {
            StatusCode::UNAUTHORIZED
        }
        ApiKeyRejectionReason::Revoked
        | ApiKeyRejectionReason::Expired
        | ApiKeyRejectionReason::MissingScope => StatusCode::FORBIDDEN,
        ApiKeyRejectionReason::RateLimited => StatusCode::TOO_MANY_REQUESTS,
    };
    (
        status,
        Json(ErrorResponse {
            error: api_key_rejection_message(reason).to_string(),
        }),
    )
}

fn api_key_rejection_message(reason: ApiKeyRejectionReason) -> &'static str {
    match reason {
        ApiKeyRejectionReason::MissingKey => "openapi api key is required",
        ApiKeyRejectionReason::UnknownKey => "openapi api key is unknown",
        ApiKeyRejectionReason::Revoked => "openapi api key is revoked",
        ApiKeyRejectionReason::Expired => "openapi api key is expired",
        ApiKeyRejectionReason::MissingScope => "chat",
        ApiKeyRejectionReason::RateLimited => "openapi api key rate limit exceeded",
    }
}

fn map_gateway_error(error: OpenApiChatGatewayError) -> (StatusCode, Json<ErrorResponse>) {
    let status = match error {
        OpenApiChatGatewayError::MissingChatScope => StatusCode::FORBIDDEN,
        OpenApiChatGatewayError::EmptyConversationId | OpenApiChatGatewayError::EmptyMessage => {
            StatusCode::BAD_REQUEST
        }
    };
    (
        status,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

fn realtime_error(message: String) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: message }),
    )
}
