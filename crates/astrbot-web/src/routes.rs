use std::sync::Arc;

use astrbot_platform::WebChatPlatform;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};

use crate::error::{map_storage_error, map_submit_error};
use crate::history::webchat_message_records_response;
use crate::message_parts::message_chain_from_submit_payload;
use crate::{ErrorResponse, SubmitTextRequest, SubmitTextResponse, WebChatMessagesResponse};

#[derive(Clone)]
struct WebChatHttpState {
    webchat: Arc<WebChatPlatform>,
}

pub fn webchat_router(webchat: Arc<WebChatPlatform>) -> Router {
    Router::new()
        .route("/api/webchat/{conversation_id}", post(submit_text))
        .route(
            "/api/webchat/{conversation_id}/messages",
            get(list_messages),
        )
        .with_state(WebChatHttpState { webchat })
}

async fn submit_text(
    State(state): State<WebChatHttpState>,
    Path(conversation_id): Path<String>,
    Json(request): Json<SubmitTextRequest>,
) -> Result<Json<SubmitTextResponse>, (StatusCode, Json<ErrorResponse>)> {
    let SubmitTextRequest {
        sender_id,
        text,
        image_urls,
        message_parts,
    } = request;
    let message = message_chain_from_submit_payload(text, message_parts, image_urls);
    let event_id = state
        .webchat
        .submit_chain(conversation_id, sender_id, message)
        .await
        .map_err(map_submit_error)?;

    Ok(Json(SubmitTextResponse { event_id }))
}

async fn list_messages(
    State(state): State<WebChatHttpState>,
    Path(conversation_id): Path<String>,
) -> Result<Json<WebChatMessagesResponse>, (StatusCode, Json<ErrorResponse>)> {
    let messages = state
        .webchat
        .conversation_history()
        .messages_for_conversation(&conversation_id)
        .await
        .map_err(map_storage_error)?;

    Ok(Json(webchat_message_records_response(
        conversation_id,
        messages,
    )))
}
