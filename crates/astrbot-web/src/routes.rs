use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use astrbot_platform::WebChatPlatform;
use axum::{
    Json, Router,
    extract::{
        Multipart, Path, Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{HeaderMap, StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::error::{map_storage_error, map_submit_error};
use crate::history::webchat_message_records_response;
use crate::message_parts::{
    message_chain_from_submit_payload, webchat_message_response_from_chain,
};
use crate::{
    ErrorResponse, SubmitTextRequest, SubmitTextResponse, WebChatMessagePart,
    WebChatMessagesResponse,
};

#[derive(Clone)]
struct WebChatHttpState {
    webchat: Arc<WebChatPlatform>,
    legacy: Arc<LegacyChatState>,
}

#[derive(Default)]
struct LegacyChatState {
    sessions: Mutex<BTreeMap<String, LegacyChatSession>>,
    attachments: Mutex<BTreeMap<String, LegacyAttachment>>,
    counter: AtomicU64,
}

#[derive(Clone, Debug, Serialize)]
struct LegacyChatSession {
    session_id: String,
    display_name: Option<String>,
    updated_at: String,
    platform_id: String,
    creator: String,
    is_group: u8,
    created_at: String,
}

#[derive(Clone, Debug)]
struct LegacyAttachment {
    filename: String,
    original_name: String,
    content_type: String,
    bytes: Vec<u8>,
}

pub fn webchat_router(webchat: Arc<WebChatPlatform>) -> Router {
    let state = WebChatHttpState {
        webchat,
        legacy: Arc::new(LegacyChatState::default()),
    };
    Router::new()
        .route("/api/webchat/{conversation_id}", post(submit_text))
        .route(
            "/api/webchat/{conversation_id}/messages",
            get(list_messages),
        )
        .route("/api/chat/send", post(legacy_send))
        .route("/api/chat/new_session", get(legacy_new_session))
        .route("/api/chat/sessions", get(legacy_sessions))
        .route("/api/chat/get_session", get(legacy_get_session))
        .route("/api/chat/delete_session", get(legacy_delete_session))
        .route(
            "/api/chat/batch_delete_sessions",
            post(legacy_batch_delete_sessions),
        )
        .route(
            "/api/chat/update_session_display_name",
            post(legacy_update_session_display_name),
        )
        .route("/api/chat/stop", post(legacy_stop))
        .route(
            "/api/chat/respond_elicitation",
            post(legacy_respond_elicitation),
        )
        .route("/api/chat/post_file", post(legacy_post_file))
        .route("/api/chat/get_attachment", get(legacy_get_attachment))
        .route("/api/chat/get_file", get(legacy_get_file))
        .route("/api/live_chat/ws", get(live_chat_ws))
        .route("/api/unified_chat/ws", get(unified_chat_ws))
        .with_state(state)
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

async fn legacy_send(
    State(state): State<WebChatHttpState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let session_id = string_field(&payload, "session_id")
        .or_else(|| string_field(&payload, "conversation_id"))
        .unwrap_or_else(|| "demo".to_string());
    let sender_id = string_field(&payload, "sender_id").unwrap_or_else(|| "user".to_string());
    let (text, message_parts, image_urls) = legacy_chat_payload(payload);
    let message = message_chain_from_submit_payload(text, message_parts, image_urls);
    let event_id = state
        .webchat
        .submit_chain(session_id.clone(), sender_id, message)
        .await
        .map_err(map_submit_error)?;
    ensure_legacy_session(&state, &session_id, None).await;

    Ok(source_ok(json!({
        "event_id": event_id,
        "session_id": session_id,
        "is_running": false
    })))
}

async fn legacy_new_session(State(state): State<WebChatHttpState>) -> Json<Value> {
    let id = format!(
        "webchat-{}",
        state.legacy.counter.fetch_add(1, Ordering::Relaxed) + 1
    );
    let session = ensure_legacy_session(&state, &id, None).await;
    source_ok(json!(session))
}

async fn legacy_sessions(State(state): State<WebChatHttpState>) -> Json<Value> {
    let mut sessions = state.legacy.sessions.lock().await;
    for sent in state.webchat.sent_messages().await {
        sessions
            .entry(sent.session.conversation_id.clone())
            .or_insert_with(|| legacy_session(sent.session.conversation_id, None));
    }
    let mut values = sessions.values().cloned().collect::<Vec<_>>();
    values.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    source_ok(json!(values))
}

async fn legacy_get_session(
    State(state): State<WebChatHttpState>,
    Query(query): Query<SessionQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let session_id = query.session_id.unwrap_or_else(|| "demo".to_string());
    ensure_legacy_session(&state, &session_id, None).await;
    let messages = state
        .webchat
        .conversation_history()
        .messages_for_conversation(&session_id)
        .await
        .map_err(map_storage_error)?;
    let history = messages
        .iter()
        .enumerate()
        .map(|(index, record)| {
            let response = webchat_message_response_from_chain(&record.chain);
            json!({
                "id": index + 1,
                "created_at": now_timestamp(),
                "content": {
                    "type": "bot",
                    "message": response.message_parts,
                    "reasoning": "",
                    "refs": []
                }
            })
        })
        .collect::<Vec<_>>();

    Ok(source_ok(json!({
        "session_id": session_id,
        "history": history,
        "is_running": false,
        "project": Value::Null
    })))
}

async fn legacy_delete_session(
    State(state): State<WebChatHttpState>,
    Query(query): Query<SessionQuery>,
) -> Json<Value> {
    let session_id = query.session_id.unwrap_or_default();
    let deleted = state
        .legacy
        .sessions
        .lock()
        .await
        .remove(&session_id)
        .is_some();
    source_ok(json!({ "deleted": deleted, "session_id": session_id }))
}

async fn legacy_batch_delete_sessions(
    State(state): State<WebChatHttpState>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    let session_ids = payload
        .get("session_ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let mut sessions = state.legacy.sessions.lock().await;
    let mut deleted_count = 0usize;
    for session_id in &session_ids {
        if sessions.remove(session_id).is_some() {
            deleted_count += 1;
        }
    }
    source_ok(json!({
        "deleted_count": deleted_count,
        "failed_count": 0,
        "failed_items": [],
        "currentSessionDeleted": false
    }))
}

async fn legacy_update_session_display_name(
    State(state): State<WebChatHttpState>,
    Json(payload): Json<Value>,
) -> Json<Value> {
    let session_id = string_field(&payload, "session_id").unwrap_or_else(|| "demo".to_string());
    let display_name = string_field(&payload, "display_name");
    let session = ensure_legacy_session(&state, &session_id, display_name).await;
    source_ok(json!(session))
}

async fn legacy_stop(Json(payload): Json<Value>) -> Json<Value> {
    let session_id = string_field(&payload, "session_id").unwrap_or_else(|| "demo".to_string());
    source_ok(json!({
        "session_id": session_id,
        "stopped": true,
        "interrupted_events": 1
    }))
}

async fn legacy_respond_elicitation(Json(payload): Json<Value>) -> Json<Value> {
    let session_id = string_field(&payload, "session_id").unwrap_or_else(|| "demo".to_string());
    let display_text = string_field(&payload, "display_text")
        .or_else(|| string_field(&payload, "reply_text"))
        .unwrap_or_else(|| "confirmed".to_string());
    source_ok(json!({
        "session_id": session_id,
        "saved_message": {
            "id": format!("elicitation-{}", now_timestamp()),
            "created_at": now_timestamp(),
            "content": {
                "type": "user",
                "message": [{ "type": "plain", "text": display_text }]
            }
        }
    }))
}

async fn legacy_post_file(
    State(state): State<WebChatHttpState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    while let Some(field) = multipart.next_field().await.map_err(multipart_error)? {
        let original_name = field.file_name().unwrap_or("upload.bin").to_string();
        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = field.bytes().await.map_err(multipart_error)?.to_vec();
        let attachment_id = format!(
            "attachment-{}",
            state.legacy.counter.fetch_add(1, Ordering::Relaxed) + 1
        );
        let filename = format!("{}-{}", attachment_id, original_name);
        state.legacy.attachments.lock().await.insert(
            attachment_id.clone(),
            LegacyAttachment {
                filename: filename.clone(),
                original_name: original_name.clone(),
                content_type: content_type.clone(),
                bytes,
            },
        );
        return Ok(source_ok(json!({
            "attachment_id": attachment_id,
            "filename": filename,
            "original_name": original_name,
            "type": media_type_from_content_type(&content_type),
            "url": format!("/api/chat/get_attachment?attachment_id={attachment_id}")
        })));
    }

    Err((
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: "multipart file is required".to_string(),
        }),
    ))
}

async fn legacy_get_attachment(
    State(state): State<WebChatHttpState>,
    Query(query): Query<AttachmentQuery>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let attachment_id = query.attachment_id.unwrap_or_default();
    let attachments = state.legacy.attachments.lock().await;
    let Some(attachment) = attachments.get(&attachment_id) else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "attachment is not found".to_string(),
            }),
        ));
    };
    Ok(bytes_response(
        attachment.bytes.clone(),
        &attachment.content_type,
        Some(&attachment.original_name),
    ))
}

async fn legacy_get_file(
    State(state): State<WebChatHttpState>,
    Query(query): Query<FileQuery>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let filename = query.filename.unwrap_or_default();
    let attachments = state.legacy.attachments.lock().await;
    let Some(attachment) = attachments
        .values()
        .find(|item| item.filename == filename || item.original_name == filename)
    else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "file is not found".to_string(),
            }),
        ));
    };
    Ok(bytes_response(
        attachment.bytes.clone(),
        &attachment.content_type,
        Some(&attachment.original_name),
    ))
}

async fn live_chat_ws(
    ws: WebSocketUpgrade,
    Query(query): Query<BTreeMap<String, String>>,
) -> Response {
    if !query.contains_key("token") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "live chat token is required".to_string(),
            }),
        )
            .into_response();
    }
    ws.on_upgrade(handle_live_socket)
}

async fn unified_chat_ws(State(state): State<WebChatHttpState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| handle_unified_socket(socket, state))
}

async fn handle_live_socket(mut socket: WebSocket) {
    let _ = socket
        .send(Message::Text(
            json!({ "t": "ready", "mode": "live" }).to_string().into(),
        ))
        .await;
    while let Some(Ok(message)) = socket.recv().await {
        match message {
            Message::Text(text) => {
                let event = serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({}));
                let event_type = string_field(&event, "t").unwrap_or_else(|| "message".to_string());
                let _ = socket
                    .send(Message::Text(
                        json!({ "t": "ack", "ack": event_type }).to_string().into(),
                    ))
                    .await;
            }
            Message::Binary(bytes) => {
                let _ = socket
                    .send(Message::Text(
                        json!({ "t": "ack", "bytes": bytes.len() })
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

async fn handle_unified_socket(mut socket: WebSocket, state: WebChatHttpState) {
    let _ = socket
        .send(Message::Text(
            json!({ "ct": "chat", "t": "ready" }).to_string().into(),
        ))
        .await;
    while let Some(Ok(message)) = socket.recv().await {
        let Message::Text(text) = message else {
            continue;
        };
        let payload = serde_json::from_str::<Value>(&text).unwrap_or_else(|_| json!({}));
        let message_type = string_field(&payload, "t").unwrap_or_default();
        if message_type == "interrupt" {
            let _ = socket
                .send(Message::Text(
                    json!({ "ct": "chat", "t": "interrupted" })
                        .to_string()
                        .into(),
                ))
                .await;
            continue;
        }
        if message_type != "send" {
            let _ = socket
                .send(Message::Text(
                    json!({ "ct": "chat", "t": "ack", "ack": message_type })
                        .to_string()
                        .into(),
                ))
                .await;
            continue;
        }
        let session_id = string_field(&payload, "session_id").unwrap_or_else(|| "demo".to_string());
        let message_id =
            string_field(&payload, "message_id").unwrap_or_else(|| "ws-message".to_string());
        let (_, message_parts, _) = legacy_chat_payload(json!({
            "message": payload.get("message").cloned().unwrap_or(Value::Null)
        }));
        let chain = message_chain_from_submit_payload(String::new(), message_parts, Vec::new());
        match state
            .webchat
            .submit_chain(session_id.clone(), "websocket", chain)
            .await
        {
            Ok(event_id) => {
                ensure_legacy_session(&state, &session_id, None).await;
                let _ = socket
                    .send(Message::Text(
                        json!({
                            "ct": "chat",
                            "t": "chunk",
                            "message_id": message_id,
                            "event_id": event_id,
                            "streaming": false
                        })
                        .to_string()
                        .into(),
                    ))
                    .await;
                let _ = socket
                    .send(Message::Text(
                        json!({ "ct": "chat", "t": "end", "message_id": message_id })
                            .to_string()
                            .into(),
                    ))
                    .await;
            }
            Err(error) => {
                let _ = socket
                    .send(Message::Text(
                        json!({ "ct": "chat", "t": "error", "data": error.to_string() })
                            .to_string()
                            .into(),
                    ))
                    .await;
            }
        }
    }
}

#[derive(Deserialize)]
struct SessionQuery {
    session_id: Option<String>,
}

#[derive(Deserialize)]
struct AttachmentQuery {
    attachment_id: Option<String>,
}

#[derive(Deserialize)]
struct FileQuery {
    filename: Option<String>,
}

fn legacy_chat_payload(payload: Value) -> (String, Vec<WebChatMessagePart>, Vec<String>) {
    let mut text = string_field(&payload, "text").unwrap_or_default();
    let mut image_urls = payload
        .get("image_urls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let mut parts = Vec::new();

    if let Some(message) = payload.get("message") {
        if let Some(message_text) = message.as_str() {
            text = message_text.to_string();
        } else if let Some(message_parts) = message.as_array() {
            parts.extend(message_parts.iter().filter_map(legacy_part_from_value));
        }
    }
    if let Some(message_parts) = payload.get("message_parts").and_then(Value::as_array) {
        parts.extend(message_parts.iter().filter_map(legacy_part_from_value));
    }

    image_urls.retain(|url| !url.is_empty());
    (text, parts, image_urls)
}

fn legacy_part_from_value(value: &Value) -> Option<WebChatMessagePart> {
    let part_type = value.get("type").and_then(Value::as_str)?;
    match part_type {
        "plain" | "text" => Some(WebChatMessagePart::Plain {
            text: string_field(value, "text").unwrap_or_default(),
        }),
        "image" => Some(WebChatMessagePart::Image {
            url: media_url_field(value, ["url", "image_url", "embedded_url"]),
        }),
        "reply" => Some(WebChatMessagePart::Reply {
            message_id: value
                .get("message_id")
                .and_then(|id| {
                    id.as_str()
                        .map(ToOwned::to_owned)
                        .or_else(|| Some(id.to_string()))
                })
                .unwrap_or_default(),
            selected_text: string_field(value, "selected_text").unwrap_or_default(),
        }),
        "record" | "audio" => Some(WebChatMessagePart::Record {
            url: media_url_field(value, ["url", "record_url", "embedded_url"]),
        }),
        "video" => Some(WebChatMessagePart::Video {
            url: media_url_field(value, ["url", "video_url", "embedded_url"]),
        }),
        "file" => Some(WebChatMessagePart::File {
            name: string_field(value, "name")
                .or_else(|| string_field(value, "filename"))
                .unwrap_or_else(|| "file".to_string()),
            url: media_url_field(value, ["url", "file_url"]),
        }),
        _ => None,
    }
}

fn media_url_field<const N: usize>(value: &Value, keys: [&str; N]) -> String {
    for key in keys {
        if let Some(url) = string_field(value, key).filter(|url| !url.trim().is_empty()) {
            return url;
        }
    }
    string_field(value, "attachment_id")
        .map(|id| format!("/api/chat/get_attachment?attachment_id={id}"))
        .unwrap_or_default()
}

async fn ensure_legacy_session(
    state: &WebChatHttpState,
    session_id: &str,
    display_name: Option<String>,
) -> LegacyChatSession {
    let mut sessions = state.legacy.sessions.lock().await;
    let entry = sessions
        .entry(session_id.to_string())
        .or_insert_with(|| legacy_session(session_id.to_string(), display_name.clone()));
    if display_name.is_some() {
        entry.display_name = display_name;
    }
    entry.updated_at = now_timestamp();
    entry.clone()
}

fn legacy_session(session_id: String, display_name: Option<String>) -> LegacyChatSession {
    let now = now_timestamp();
    LegacyChatSession {
        display_name,
        session_id,
        updated_at: now.clone(),
        platform_id: "webchat".to_string(),
        creator: "dashboard".to_string(),
        is_group: 0,
        created_at: now,
    }
}

fn source_ok(data: Value) -> Json<Value> {
    Json(json!({ "status": "ok", "message": "", "data": data }))
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn now_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    format!("unix:{seconds}")
}

fn multipart_error(
    error: axum::extract::multipart::MultipartError,
) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

fn media_type_from_content_type(content_type: &str) -> &'static str {
    if content_type.starts_with("image/") {
        "image"
    } else if content_type.starts_with("audio/") {
        "record"
    } else if content_type.starts_with("video/") {
        "video"
    } else {
        "file"
    }
}

fn bytes_response(bytes: Vec<u8>, content_type: &str, filename: Option<&str>) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        content_type
            .parse()
            .unwrap_or_else(|_| "application/octet-stream".parse().unwrap()),
    );
    if let Some(filename) = filename {
        if let Ok(value) = format!("inline; filename=\"{filename}\"").parse() {
            headers.insert("content-disposition", value);
        }
    }
    (headers, bytes).into_response()
}
