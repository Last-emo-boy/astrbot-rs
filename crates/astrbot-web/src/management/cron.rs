use std::sync::Arc;
use std::time::{Duration, SystemTime};

use astrbot_cron::{
    ActiveAgentCronPayload, CronJob, CronJobKind, CronJobSchedule, CronScheduler,
    CronSchedulerState, CronTickReport, SchedulerJobSnapshot,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone)]
pub struct ManagementCronState {
    scheduler: Arc<CronScheduler>,
}

impl ManagementCronState {
    pub fn new(scheduler: Arc<CronScheduler>) -> Self {
        Self { scheduler }
    }

    pub fn scheduler(&self) -> Arc<CronScheduler> {
        self.scheduler.clone()
    }
}

impl std::fmt::Debug for ManagementCronState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagementCronState")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementCronListRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<CronJobKind>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementCronJobRequest {
    pub job_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementCronUpsertRequest {
    pub job: CronJob,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementCronTickRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now_unix: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct SourceCronListQuery {
    #[serde(rename = "type", default)]
    pub job_type: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementCronCatalogResponse {
    pub state: String,
    pub jobs: Vec<CronJob>,
    pub scheduled_jobs: Vec<SchedulerJobSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementCronMutationResponse {
    pub ok: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementCronTickResponse {
    pub report: CronTickReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementCronDeleteResponse {
    pub deleted: bool,
}

pub async fn list(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementCronListRequest>,
) -> Result<Json<ManagementCronCatalogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let cron = state.cron().ok_or_else(cron_unavailable)?;
    let scheduler = cron.scheduler();
    let jobs = scheduler
        .list_jobs(request.kind)
        .await
        .map_err(internal_error)?;

    Ok(Json(ManagementCronCatalogResponse {
        state: scheduler_state_label(scheduler.state()).to_string(),
        jobs,
        scheduled_jobs: scheduler.scheduled_jobs(),
    }))
}

pub async fn upsert(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementCronUpsertRequest>,
) -> Result<Json<ManagementCronMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let cron = state.cron().ok_or_else(cron_unavailable)?;
    if request.job.job_id.trim().is_empty() || request.job.name.trim().is_empty() {
        return Err(bad_request("cron job_id and name are required"));
    }
    cron.scheduler()
        .add_job(request.job)
        .await
        .map_err(internal_error)?;

    Ok(Json(ManagementCronMutationResponse { ok: true }))
}

pub async fn start(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementCronMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let cron = state.cron().ok_or_else(cron_unavailable)?;
    cron.scheduler().start().await.map_err(internal_error)?;

    Ok(Json(ManagementCronMutationResponse { ok: true }))
}

pub async fn shutdown(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementCronMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let cron = state.cron().ok_or_else(cron_unavailable)?;
    cron.scheduler().shutdown().await.map_err(internal_error)?;

    Ok(Json(ManagementCronMutationResponse { ok: true }))
}

pub async fn run(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementCronJobRequest>,
) -> Result<Json<ManagementCronMutationResponse>, (StatusCode, Json<ErrorResponse>)> {
    let cron = state.cron().ok_or_else(cron_unavailable)?;
    cron.scheduler()
        .run_job(&request.job_id)
        .await
        .map_err(map_cron_error)?;

    Ok(Json(ManagementCronMutationResponse { ok: true }))
}

pub async fn tick(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementCronTickRequest>,
) -> Result<Json<ManagementCronTickResponse>, (StatusCode, Json<ErrorResponse>)> {
    let cron = state.cron().ok_or_else(cron_unavailable)?;
    let now = request
        .now_unix
        .map(|seconds| SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
        .unwrap_or_else(SystemTime::now);
    let report = cron
        .scheduler()
        .tick_due(now)
        .await
        .map_err(map_cron_error)?;

    Ok(Json(ManagementCronTickResponse { report }))
}

pub async fn delete(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementCronJobRequest>,
) -> Result<Json<ManagementCronDeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    let cron = state.cron().ok_or_else(cron_unavailable)?;
    let deleted = cron
        .scheduler()
        .delete_job(&request.job_id)
        .await
        .map_err(internal_error)?;

    Ok(Json(ManagementCronDeleteResponse { deleted }))
}

pub async fn legacy_list(
    State(state): State<ManagementApiState>,
    Query(query): Query<SourceCronListQuery>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let cron = state.cron().ok_or_else(cron_unavailable)?;
    let scheduled_jobs = cron.scheduler().scheduled_jobs();
    let kind = match query.job_type.as_deref() {
        Some("basic") => Some(CronJobKind::Basic),
        Some("active_agent") | Some("active-agent") => Some(CronJobKind::ActiveAgent),
        _ => None,
    };
    let jobs = cron
        .scheduler()
        .list_jobs(kind)
        .await
        .map_err(internal_error)?
        .into_iter()
        .map(|job| job_to_source(job, &scheduled_jobs))
        .collect::<Vec<_>>();
    Ok(source_ok(json!(jobs)))
}

pub async fn legacy_create(
    State(state): State<ManagementApiState>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let cron = state.cron().ok_or_else(cron_unavailable)?;
    let job = source_job_from_payload(None, &payload)?;
    cron.scheduler()
        .add_job(job.clone())
        .await
        .map_err(internal_error)?;
    let scheduled_jobs = cron.scheduler().scheduled_jobs();
    Ok(source_ok(job_to_source(job, &scheduled_jobs)))
}

pub async fn legacy_update(
    State(state): State<ManagementApiState>,
    Path(job_id): Path<String>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let cron = state.cron().ok_or_else(cron_unavailable)?;
    let existing = cron
        .scheduler()
        .job(&job_id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| not_found("cron job not found"))?;
    let job = source_job_from_payload(Some(existing), &payload)?;
    cron.scheduler()
        .add_job(job.clone())
        .await
        .map_err(internal_error)?;
    let scheduled_jobs = cron.scheduler().scheduled_jobs();
    Ok(source_ok(job_to_source(job, &scheduled_jobs)))
}

pub async fn legacy_delete(
    State(state): State<ManagementApiState>,
    Path(job_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let cron = state.cron().ok_or_else(cron_unavailable)?;
    cron.scheduler()
        .delete_job(&job_id)
        .await
        .map_err(internal_error)?;
    Ok(source_ok_with_message(json!({}), "deleted"))
}

pub async fn legacy_run(
    State(state): State<ManagementApiState>,
    Path(job_id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let cron = state.cron().ok_or_else(cron_unavailable)?;
    cron.scheduler()
        .run_job(&job_id)
        .await
        .map_err(map_cron_error)?;
    Ok(source_ok_with_message(json!({ "job_id": job_id }), "run"))
}

fn scheduler_state_label(state: CronSchedulerState) -> &'static str {
    match state {
        CronSchedulerState::Stopped => "stopped",
        CronSchedulerState::Running => "running",
    }
}

fn source_job_from_payload(
    existing: Option<CronJob>,
    payload: &Value,
) -> Result<CronJob, (StatusCode, Json<ErrorResponse>)> {
    let job_id = existing
        .as_ref()
        .map(|job| job.job_id.clone())
        .unwrap_or_else(|| format!("cron-{}", now_millis()));
    let name = string_field(payload, "name")
        .or_else(|| existing.as_ref().map(|job| job.name.clone()))
        .unwrap_or_else(|| "active_agent_task".to_string());
    let run_once = payload
        .get("run_once")
        .and_then(Value::as_bool)
        .or_else(|| existing.as_ref().map(|job| job.schedule.is_run_once()))
        .unwrap_or(false);
    let run_at = string_field(payload, "run_at")
        .or_else(|| {
            payload
                .get("payload")
                .and_then(|payload| string_field(payload, "run_at"))
        })
        .or_else(|| {
            existing
                .as_ref()
                .and_then(|job| job.schedule.run_at().map(str::to_string))
        });
    let cron_expression = string_field(payload, "cron_expression").or_else(|| {
        existing
            .as_ref()
            .and_then(|job| job.schedule.cron_expression().map(str::to_string))
    });
    let mut schedule = if run_once {
        CronJobSchedule::run_once_at(
            run_at.ok_or_else(|| bad_request("run_at is required when run_once=true"))?,
        )
    } else {
        CronJobSchedule::cron(
            cron_expression
                .ok_or_else(|| bad_request("cron_expression is required when run_once=false"))?,
        )
    };
    if let Some(timezone) = string_field(payload, "timezone").or_else(|| {
        existing
            .as_ref()
            .and_then(|job| job.schedule.timezone.clone())
    }) {
        schedule = schedule.with_timezone(timezone);
    }

    let session = string_field(payload, "session")
        .or_else(|| {
            payload
                .get("payload")
                .and_then(|payload| string_field(payload, "session"))
        })
        .or_else(|| {
            existing
                .as_ref()
                .and_then(|job| job.payload.get("session").and_then(Value::as_str))
                .map(ToString::to_string)
        })
        .ok_or_else(|| bad_request("session is required"))?;
    let note = string_field(payload, "note")
        .or_else(|| string_field(payload, "description"))
        .or_else(|| {
            payload
                .get("payload")
                .and_then(|payload| string_field(payload, "note"))
        })
        .or_else(|| existing.as_ref().and_then(|job| job.description.clone()))
        .unwrap_or_else(|| name.clone());
    let persona_id = payload_or_existing_string(payload, existing.as_ref(), "persona_id");
    let provider_id = payload_or_existing_string(payload, existing.as_ref(), "provider_id");
    let sender_id = payload_or_existing_string(payload, existing.as_ref(), "sender_id");
    let origin = payload_or_existing_string(payload, existing.as_ref(), "origin")
        .unwrap_or_else(|| "api".to_string());

    let mut job = CronJob::active_agent(
        job_id,
        name,
        schedule,
        active_agent_payload(session, note.clone(), sender_id, origin),
    )
    .with_description(note)
    .persistent(true);
    if let Some(object) = job.payload.as_object_mut() {
        if let Some(run_at) = job.schedule.run_at().map(ToString::to_string) {
            object.insert("run_at".to_string(), json!(run_at));
        }
        if let Some(persona_id) = persona_id {
            object.insert("persona_id".to_string(), json!(persona_id));
        }
        if let Some(provider_id) = provider_id {
            object.insert("provider_id".to_string(), json!(provider_id));
        }
    }
    job.enabled = payload
        .get("enabled")
        .and_then(Value::as_bool)
        .or_else(|| existing.as_ref().map(|job| job.enabled))
        .unwrap_or(true);
    if let Some(existing) = existing {
        job.status = existing.status;
        job.last_error = existing.last_error;
    }
    Ok(job)
}

fn job_to_source(job: CronJob, scheduled_jobs: &[SchedulerJobSnapshot]) -> Value {
    let note = job
        .payload
        .get("note")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| job.description.clone())
        .unwrap_or_default();
    let session = job
        .payload
        .get("session")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let run_at = job.schedule.run_at().map(ToString::to_string).or_else(|| {
        job.payload
            .get("run_at")
            .and_then(Value::as_str)
            .map(ToString::to_string)
    });
    let next_run_time = if job.enabled && job.schedule.is_run_once() {
        run_at.clone()
    } else {
        scheduled_jobs
            .iter()
            .find(|snapshot| snapshot.job_id == job.job_id)
            .and_then(|snapshot| {
                let key = snapshot.schedule_key.trim();
                (key.contains('T') || key.parse::<u64>().is_ok()).then_some(key.to_string())
            })
    };
    let job_type = match job.kind {
        CronJobKind::Basic => "basic",
        CronJobKind::ActiveAgent => "active_agent",
    };
    json!({
        "job_id": job.job_id,
        "name": job.name,
        "type": job_type,
        "job_type": job_type,
        "kind": job.kind,
        "schedule": job.schedule,
        "cron_expression": job.schedule.cron_expression(),
        "timezone": job.schedule.timezone,
        "session": session,
        "enabled": job.enabled,
        "persistent": job.persistent,
        "description": job.description,
        "payload": job.payload,
        "note": note,
        "run_at": run_at,
        "run_once": job.schedule.is_run_once(),
        "last_run_at": Value::Null,
        "next_run_time": next_run_time,
        "last_error": job.last_error,
        "created_at": Value::Null,
        "updated_at": Value::Null
    })
}

fn active_agent_payload(
    session: String,
    note: String,
    sender_id: Option<String>,
    origin: String,
) -> ActiveAgentCronPayload {
    let mut payload = ActiveAgentCronPayload::new(session, note).with_origin(origin);
    if let Some(sender_id) = sender_id {
        payload = payload.with_sender_id(sender_id);
    }
    payload
}

fn payload_or_existing_string(
    payload: &Value,
    existing: Option<&CronJob>,
    key: &str,
) -> Option<String> {
    string_field(payload, key)
        .or_else(|| {
            payload
                .get("payload")
                .and_then(|payload| string_field(payload, key))
        })
        .or_else(|| {
            existing
                .and_then(|job| job.payload.get(key).and_then(Value::as_str))
                .map(ToString::to_string)
        })
}

fn string_field(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(Value::as_str).and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn source_ok(data: Value) -> Json<Value> {
    source_ok_with_message(data, "")
}

fn source_ok_with_message(data: Value, message: impl Into<String>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": message.into(),
        "data": data
    }))
}

fn not_found(message: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
}

fn cron_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "cron management state is not configured".to_string(),
        }),
    )
}

fn bad_request(message: &'static str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
}

fn map_cron_error(error: astrbot_core::AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    let message = error.to_string();
    let status = if message.contains("not found") {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };
    (status, Json(ErrorResponse { error: message }))
}

fn internal_error(error: astrbot_core::AstrbotError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}
