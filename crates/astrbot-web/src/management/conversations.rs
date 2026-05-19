use astrbot_conversation::{ConversationRecord, ConversationService};
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone)]
pub struct ManagementConversationState {
    service: ConversationService,
}

impl ManagementConversationState {
    pub fn new(service: ConversationService) -> Self {
        Self { service }
    }

    pub fn service(&self) -> &ConversationService {
        &self.service
    }
}

impl std::fmt::Debug for ManagementConversationState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagementConversationState")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ManagementConversationListRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct SourceConversationListQuery {
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub page_size: Option<usize>,
    #[serde(default)]
    pub platforms: Option<String>,
    #[serde(default)]
    pub message_types: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub exclude_ids: Option<String>,
    #[serde(default)]
    pub exclude_platforms: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementConversationGetRequest {
    pub platform_id: String,
    pub conversation_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementConversationUpsertRequest {
    pub platform_id: String,
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona_id: Option<String>,
    #[serde(default)]
    pub set_current: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementConversationRenameRequest {
    pub platform_id: String,
    pub conversation_id: String,
    pub title: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementConversationCurrentRequest {
    pub platform_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementConversationDeleteRequest {
    pub platform_id: String,
    pub conversation_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementConversationBatchDeleteRequest {
    pub platform_id: String,
    #[serde(default)]
    pub conversation_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementConversationDescriptor {
    pub platform_id: String,
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona_id: Option<String>,
    pub current: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementConversationCatalogResponse {
    pub conversations: Vec<ManagementConversationDescriptor>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementConversationResponse {
    pub conversation: ManagementConversationDescriptor,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementConversationCurrentResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation: Option<ManagementConversationDescriptor>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementConversationMutationResponse {
    pub ok: bool,
    pub conversation: ManagementConversationDescriptor,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementConversationDeleteResponse {
    pub deleted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementConversationBatchDeleteResponse {
    pub deleted_count: usize,
    pub deleted_ids: Vec<String>,
    pub missing_ids: Vec<String>,
}

pub async fn list(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementConversationListRequest>,
) -> Result<Json<ManagementConversationCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let conversations = state
        .conversations()
        .ok_or_else(conversations_unavailable)?;
    let platform_id = trim_option(request.platform_id);
    let records = conversations
        .service()
        .list(platform_id.as_deref())
        .await
        .map_err(internal_error)?;
    let descriptors = descriptors_for_records(conversations.service(), records).await?;

    Ok(Json(ManagementConversationCatalogResponse {
        conversations: descriptors,
    }))
}

pub async fn get(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementConversationGetRequest>,
) -> Result<Json<ManagementConversationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let conversations = state
        .conversations()
        .ok_or_else(conversations_unavailable)?;
    let (platform_id, conversation_id) =
        normalize_key(request.platform_id, request.conversation_id)?;
    let record = conversations
        .service()
        .get(&platform_id, &conversation_id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("conversation not found"))?;
    let conversation = descriptor_for_record(conversations.service(), record).await?;

    Ok(Json(ManagementConversationResponse { conversation }))
}

pub async fn upsert(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementConversationUpsertRequest>,
) -> Result<Json<ManagementConversationMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let conversations = state
        .conversations()
        .ok_or_else(conversations_unavailable)?;
    let (platform_id, conversation_id) =
        normalize_key(request.platform_id, request.conversation_id)?;
    let mut record = conversations
        .service()
        .get(&platform_id, &conversation_id)
        .await
        .map_err(internal_error)?
        .unwrap_or_else(|| {
            ConversationRecord::new(platform_id.clone(), conversation_id.clone())
                .with_user_id(source_user_id(&platform_id, &conversation_id))
                .with_history("[]")
        });
    if let Some(user_id) = trim_option(request.user_id) {
        record = record.with_user_id(user_id);
    } else if record.user_id.is_none() {
        record = record.with_user_id(source_user_id(&platform_id, &conversation_id));
    }
    if let Some(title) = trim_option(request.title) {
        record = record.with_title(title);
    }
    if let Some(persona_id) = trim_option(request.persona_id) {
        record = record.with_persona_id(persona_id);
    }
    if record.history.is_none() {
        record = record.with_history("[]");
    }
    record = record.touch();
    conversations
        .service()
        .upsert(record.clone())
        .await
        .map_err(internal_error)?;
    if request.set_current {
        conversations
            .service()
            .switch_current(&platform_id, &conversation_id)
            .await
            .map_err(bad_request)?;
    }
    let conversation = descriptor_for_record(conversations.service(), record).await?;

    Ok(Json(ManagementConversationMutationResponse {
        ok: true,
        conversation,
    }))
}

pub async fn rename(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementConversationRenameRequest>,
) -> Result<Json<ManagementConversationMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let conversations = state
        .conversations()
        .ok_or_else(conversations_unavailable)?;
    let (platform_id, conversation_id) =
        normalize_key(request.platform_id, request.conversation_id)?;
    let title =
        non_empty_string(request.title).ok_or_else(|| bad_request_message("title is required"))?;
    let existing = conversations
        .service()
        .get(&platform_id, &conversation_id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("conversation not found"))?;
    let mut record = existing.with_title(title).touch();
    if record.user_id.is_none() {
        record = record.with_user_id(source_user_id(&platform_id, &conversation_id));
    }
    conversations
        .service()
        .upsert(record.clone())
        .await
        .map_err(internal_error)?;
    let conversation = descriptor_for_record(conversations.service(), record).await?;

    Ok(Json(ManagementConversationMutationResponse {
        ok: true,
        conversation,
    }))
}

pub async fn current(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementConversationCurrentRequest>,
) -> Result<Json<ManagementConversationCurrentResponse>, (StatusCode, Json<ErrorResponse>)> {
    let conversations = state
        .conversations()
        .ok_or_else(conversations_unavailable)?;
    let platform_id = non_empty_string(request.platform_id)
        .ok_or_else(|| bad_request_message("platform_id is required"))?;
    if let Some(conversation_id) = trim_option(request.conversation_id) {
        conversations
            .service()
            .switch_current(&platform_id, &conversation_id)
            .await
            .map_err(bad_request)?;
    }
    let record = conversations
        .service()
        .current(&platform_id)
        .await
        .map_err(internal_error)?;
    let conversation = match record {
        Some(record) => Some(descriptor_for_record(conversations.service(), record).await?),
        None => None,
    };

    Ok(Json(ManagementConversationCurrentResponse { conversation }))
}

pub async fn delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementConversationDeleteRequest>,
) -> Result<Json<ManagementConversationDeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let conversations = state
        .conversations()
        .ok_or_else(conversations_unavailable)?;
    let (platform_id, conversation_id) =
        normalize_key(request.platform_id, request.conversation_id)?;
    let deleted = conversations
        .service()
        .delete(&platform_id, &conversation_id)
        .await
        .map_err(internal_error)?;

    Ok(Json(ManagementConversationDeleteResponse { deleted }))
}

pub async fn batch_delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementConversationBatchDeleteRequest>,
) -> Result<Json<ManagementConversationBatchDeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let conversations = state
        .conversations()
        .ok_or_else(conversations_unavailable)?;
    let platform_id = non_empty_string(request.platform_id)
        .ok_or_else(|| bad_request_message("platform_id is required"))?;
    let mut deleted_ids = Vec::new();
    let mut missing_ids = Vec::new();
    for conversation_id in request
        .conversation_ids
        .into_iter()
        .filter_map(non_empty_string)
    {
        let deleted = conversations
            .service()
            .delete(&platform_id, &conversation_id)
            .await
            .map_err(internal_error)?;
        if deleted {
            deleted_ids.push(conversation_id);
        } else {
            missing_ids.push(conversation_id);
        }
    }

    Ok(Json(ManagementConversationBatchDeleteResponse {
        deleted_count: deleted_ids.len(),
        deleted_ids,
        missing_ids,
    }))
}

pub async fn source_list(
    State(state): State<ManagementApiState>,
    Query(query): Query<SourceConversationListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let conversations = state
        .conversations()
        .ok_or_else(conversations_unavailable)?;
    let mut records = conversations
        .service()
        .list(None)
        .await
        .map_err(internal_error)?;
    let platforms = split_csv(query.platforms);
    let message_types = split_csv(query.message_types);
    let search = query.search.unwrap_or_default().trim().to_lowercase();
    let exclude_ids = split_csv(query.exclude_ids);
    let exclude_platforms = split_csv(query.exclude_platforms);

    records.retain(|record| {
        let source = source_record(record);
        if !platforms.is_empty()
            && !platforms
                .iter()
                .any(|platform| platform == &record.platform_id)
        {
            return false;
        }
        if exclude_platforms
            .iter()
            .any(|platform| platform == &record.platform_id)
        {
            return false;
        }
        if exclude_ids
            .iter()
            .any(|id| id == &record.conversation_id || id == &source.user_id)
        {
            return false;
        }
        if !message_types.is_empty()
            && !message_types
                .iter()
                .any(|message_type| source.user_id.contains(&format!(":{message_type}:")))
        {
            return false;
        }
        if !search.is_empty() {
            let haystack = format!(
                "{} {} {} {} {}",
                source.title.as_deref().unwrap_or_default(),
                source.user_id,
                source.cid,
                source.platform_id,
                source.history
            )
            .to_lowercase();
            if !haystack.contains(&search) {
                return false;
            }
        }
        true
    });
    records.sort_by(|left, right| {
        right
            .created_at
            .unwrap_or_default()
            .cmp(&left.created_at.unwrap_or_default())
            .then_with(|| left.conversation_id.cmp(&right.conversation_id))
    });

    let total = records.len();
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let start = (page - 1).saturating_mul(page_size);
    let page_records = records
        .into_iter()
        .skip(start)
        .take(page_size)
        .map(|record| json!(source_record(&record)))
        .collect::<Vec<_>>();
    let total_pages = if total == 0 {
        1
    } else {
        total.div_ceil(page_size)
    };

    Ok(source_ok(json!({
        "conversations": page_records,
        "pagination": {
            "page": page,
            "page_size": page_size,
            "total": total,
            "total_pages": total_pages
        }
    })))
}

pub async fn source_detail(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let conversations = state
        .conversations()
        .ok_or_else(conversations_unavailable)?;
    let record = source_find_record(
        conversations.service(),
        string_field(&payload, "user_id").as_deref(),
        string_field(&payload, "cid").as_deref(),
    )
    .await?
    .ok_or_else(|| not_found("conversation not found"))?;
    Ok(source_ok(json!(source_record(&record))))
}

pub async fn source_update(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let conversations = state
        .conversations()
        .ok_or_else(conversations_unavailable)?;
    let mut record = source_find_record(
        conversations.service(),
        string_field(&payload, "user_id").as_deref(),
        string_field(&payload, "cid").as_deref(),
    )
    .await?
    .ok_or_else(|| not_found("conversation not found"))?;
    if let Some(title) = payload.get("title").and_then(Value::as_str) {
        record.title = Some(title.to_string());
    }
    if let Some(persona_id) = payload.get("persona_id").and_then(Value::as_str) {
        record.persona_id = Some(persona_id.to_string());
    }
    record = record.touch();
    conversations
        .service()
        .upsert(record)
        .await
        .map_err(internal_error)?;
    Ok(source_ok(json!({ "message": "对话信息更新成功" })))
}

pub async fn source_update_history(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let conversations = state
        .conversations()
        .ok_or_else(conversations_unavailable)?;
    let mut record = source_find_record(
        conversations.service(),
        string_field(&payload, "user_id").as_deref(),
        string_field(&payload, "cid").as_deref(),
    )
    .await?
    .ok_or_else(|| not_found("conversation not found"))?;
    let history = payload
        .get("history")
        .ok_or_else(|| bad_request_message("history is required"))?;
    let history_value = if let Some(text) = history.as_str() {
        serde_json::from_str::<Value>(text)
            .map_err(|_| bad_request_message("history must be valid JSON"))?
    } else {
        history.clone()
    };
    if !history_value.is_array() {
        return Err(bad_request_message("history must be a JSON array"));
    }
    record.history = Some(history_value.to_string());
    record = record.touch();
    conversations
        .service()
        .upsert(record)
        .await
        .map_err(internal_error)?;
    Ok(source_ok(json!({ "message": "对话历史更新成功" })))
}

pub async fn source_delete(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let conversations = state
        .conversations()
        .ok_or_else(conversations_unavailable)?;
    let requested = if let Some(items) = payload.get("conversations").and_then(Value::as_array) {
        items
            .iter()
            .map(|item| (string_field(item, "user_id"), string_field(item, "cid")))
            .collect::<Vec<_>>()
    } else {
        vec![(
            string_field(&payload, "user_id"),
            string_field(&payload, "cid"),
        )]
    };
    let mut deleted_count = 0usize;
    let mut failed_items = Vec::new();
    for (user_id, cid) in requested {
        let Some(cid) = cid.filter(|value| !value.trim().is_empty()) else {
            failed_items.push("cid is required".to_string());
            continue;
        };
        let found =
            source_find_record(conversations.service(), user_id.as_deref(), Some(&cid)).await?;
        let Some(record) = found else {
            failed_items.push(format!("cid:{cid} - conversation not found"));
            continue;
        };
        if conversations
            .service()
            .delete(&record.platform_id, &record.conversation_id)
            .await
            .map_err(internal_error)?
        {
            deleted_count += 1;
        } else {
            failed_items.push(format!("cid:{cid} - conversation not found"));
        }
    }
    Ok(source_ok(json!({
        "message": format!("成功删除 {deleted_count} 个对话"),
        "deleted_count": deleted_count,
        "failed_count": failed_items.len(),
        "failed_items": failed_items
    })))
}

pub async fn source_export(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let conversations = state
        .conversations()
        .ok_or_else(conversations_unavailable)?;
    let items = payload
        .get("conversations")
        .and_then(Value::as_array)
        .ok_or_else(|| bad_request_message("conversations is required"))?;
    let mut lines = Vec::new();
    for item in items {
        let record = source_find_record(
            conversations.service(),
            string_field(item, "user_id").as_deref(),
            string_field(item, "cid").as_deref(),
        )
        .await?
        .ok_or_else(|| not_found("conversation not found"))?;
        let source = source_record(&record);
        let content = serde_json::from_str::<Value>(&source.history).unwrap_or_else(|_| json!([]));
        lines.push(
            json!({
                "cid": source.cid,
                "user_id": source.user_id,
                "platform_id": source.platform_id,
                "title": source.title,
                "persona_id": source.persona_id,
                "created_at": source.created_at,
                "updated_at": source.updated_at,
                "content": content
            })
            .to_string(),
        );
    }
    if lines.is_empty() {
        return Err(bad_request_message("no conversations were exported"));
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        "application/jsonl; charset=utf-8"
            .parse()
            .expect("valid content type"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        "attachment; filename=\"astrbot_conversations_export.jsonl\""
            .parse()
            .expect("valid content disposition"),
    );
    Ok((headers, lines.join("\n")).into_response())
}

async fn descriptors_for_records(
    service: &ConversationService,
    records: Vec<ConversationRecord>,
) -> Result<Vec<ManagementConversationDescriptor>, (StatusCode, Json<ErrorResponse>)> {
    let mut descriptors = Vec::with_capacity(records.len());
    for record in records {
        descriptors.push(descriptor_for_record(service, record).await?);
    }
    Ok(descriptors)
}

async fn descriptor_for_record(
    service: &ConversationService,
    record: ConversationRecord,
) -> Result<ManagementConversationDescriptor, (StatusCode, Json<ErrorResponse>)> {
    let current = service
        .current(&record.platform_id)
        .await
        .map_err(internal_error)?
        .is_some_and(|current| current.conversation_id == record.conversation_id);
    Ok(ManagementConversationDescriptor {
        platform_id: record.platform_id,
        conversation_id: record.conversation_id,
        user_id: record.user_id,
        title: record.title,
        persona_id: record.persona_id,
        current,
    })
}

fn normalize_key(
    platform_id: String,
    conversation_id: String,
) -> Result<(String, String), (StatusCode, Json<ErrorResponse>)> {
    let platform_id = non_empty_string(platform_id)
        .ok_or_else(|| bad_request_message("platform_id is required"))?;
    let conversation_id = non_empty_string(conversation_id)
        .ok_or_else(|| bad_request_message("conversation_id is required"))?;
    Ok((platform_id, conversation_id))
}

fn trim_option(value: Option<String>) -> Option<String> {
    value.and_then(non_empty_string)
}

fn non_empty_string(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[derive(Clone, Debug, Serialize)]
struct SourceConversationRecord {
    user_id: String,
    cid: String,
    platform_id: String,
    history: String,
    title: Option<String>,
    persona_id: Option<String>,
    created_at: i64,
    updated_at: i64,
    token_usage: u64,
}

fn source_record(record: &ConversationRecord) -> SourceConversationRecord {
    let user_id = record
        .user_id
        .clone()
        .unwrap_or_else(|| source_user_id(&record.platform_id, &record.conversation_id));
    SourceConversationRecord {
        user_id,
        cid: record.conversation_id.clone(),
        platform_id: record.platform_id.clone(),
        history: record.history.clone().unwrap_or_else(|| "[]".to_string()),
        title: record.title.clone(),
        persona_id: record.persona_id.clone(),
        created_at: record.created_at.unwrap_or_default(),
        updated_at: record.updated_at.unwrap_or_default(),
        token_usage: record.token_usage.unwrap_or_default(),
    }
}

async fn source_find_record(
    service: &ConversationService,
    user_id: Option<&str>,
    cid: Option<&str>,
) -> Result<Option<ConversationRecord>, (StatusCode, Json<ErrorResponse>)> {
    let cid = cid
        .and_then(|value| non_empty_string(value.to_string()))
        .ok_or_else(|| bad_request_message("cid is required"))?;
    let user_id = user_id.and_then(|value| non_empty_string(value.to_string()));
    let records = service.list(None).await.map_err(internal_error)?;
    Ok(records.into_iter().find(|record| {
        if record.conversation_id != cid {
            return false;
        }
        match user_id.as_deref() {
            Some(user_id) => source_record(record).user_id == user_id,
            None => true,
        }
    }))
}

fn source_user_id(platform_id: &str, conversation_id: &str) -> String {
    format!("{platform_id}:FriendMessage:{conversation_id}")
}

fn split_csv(value: Option<String>) -> Vec<String> {
    value
        .unwrap_or_default()
        .split(',')
        .filter_map(|item| non_empty_string(item.to_string()))
        .collect()
}

fn string_field(payload: &Value, key: &str) -> Option<String> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .and_then(non_empty_string)
}

fn source_ok(data: Value) -> Json<Value> {
    Json(json!({ "status": "ok", "message": "", "data": data }))
}

fn conversations_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "conversation management state is not configured".to_string(),
        }),
    )
}

fn bad_request(error: astrbot_core::AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

fn bad_request_message(message: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
}

fn not_found(message: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
}

fn internal_error(error: astrbot_core::AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}
