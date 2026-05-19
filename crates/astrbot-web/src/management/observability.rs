use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use astrbot_metrics::{MetricEvent, MetricEventKind};
use astrbot_observability::{
    InMemoryLogBuffer, LogBufferSnapshot, LogEntry, LogEntryId, LogLevel, TraceEvent,
};
use astrbot_storage::{PlatformStatsRecord, PlatformStatsRepository};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::sse::{Event, Sse},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_stream::wrappers::ReceiverStream;

use crate::ErrorResponse;

use super::ManagementApiState;

#[derive(Clone)]
pub struct ManagementObservabilityState {
    logs: Arc<InMemoryLogBuffer>,
    traces: Arc<Mutex<Vec<TraceEvent>>>,
    metrics: Arc<Mutex<Vec<MetricEvent>>>,
    log_store: Option<Arc<ManagementLogFileStore>>,
    metric_store: Option<Arc<ManagementMetricFileStore>>,
    platform_stats: Option<Arc<dyn PlatformStatsRepository>>,
    trace_settings_store: Option<Arc<ManagementTraceSettingsFileStore>>,
    trace_settings: Arc<Mutex<ManagementTraceSettings>>,
    started_at: SystemTime,
}

impl std::fmt::Debug for ManagementObservabilityState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManagementObservabilityState")
            .field("log_store", &self.log_store)
            .field("metric_store", &self.metric_store)
            .field("has_platform_stats", &self.platform_stats.is_some())
            .field("trace_settings_store", &self.trace_settings_store)
            .field("started_at", &self.started_at)
            .finish_non_exhaustive()
    }
}

impl ManagementObservabilityState {
    pub fn new(logs: Arc<InMemoryLogBuffer>, traces: Vec<TraceEvent>) -> Self {
        Self {
            logs,
            traces: Arc::new(Mutex::new(traces)),
            metrics: Arc::new(Mutex::new(Vec::new())),
            log_store: None,
            metric_store: None,
            platform_stats: None,
            trace_settings_store: None,
            trace_settings: Arc::new(Mutex::new(ManagementTraceSettings::default())),
            started_at: SystemTime::now(),
        }
    }

    pub fn with_metrics(self, metrics: Vec<MetricEvent>) -> Self {
        if let Ok(mut current) = self.metrics.lock() {
            *current = metrics;
        }
        self
    }

    pub async fn with_log_file(mut self, path: impl Into<PathBuf>) -> Result<Self, String> {
        let store = Arc::new(ManagementLogFileStore::new(path.into()));
        let stored_logs = store.load()?;
        if !stored_logs.is_empty() {
            self.logs.restore(stored_logs).await;
        }
        self.log_store = Some(store);
        Ok(self)
    }

    pub fn with_metric_file(mut self, path: impl Into<PathBuf>) -> Self {
        let store = Arc::new(ManagementMetricFileStore::new(path.into()));
        if let Ok(stored_metrics) = store.load()
            && !stored_metrics.is_empty()
            && let Ok(mut current) = self.metrics.lock()
        {
            *current = stored_metrics;
        }
        self.metric_store = Some(store);
        self
    }

    pub fn with_platform_stats_repository(
        mut self,
        repository: Arc<dyn PlatformStatsRepository>,
    ) -> Self {
        self.platform_stats = Some(repository);
        self
    }

    pub fn with_trace_settings_file(mut self, path: impl Into<PathBuf>) -> Result<Self, String> {
        let store = Arc::new(ManagementTraceSettingsFileStore::new(path.into()));
        if let Some(settings) = store.load()? {
            *self
                .trace_settings
                .lock()
                .map_err(|error| format!("trace settings lock: {error}"))? = settings;
        }
        self.trace_settings_store = Some(store);
        Ok(self)
    }

    pub fn logs(&self) -> Arc<InMemoryLogBuffer> {
        self.logs.clone()
    }

    pub fn traces(&self) -> Result<Vec<TraceEvent>, String> {
        self.traces
            .lock()
            .map_err(|error| format!("trace state lock: {error}"))
            .map(|traces| traces.clone())
    }

    pub fn metrics(&self) -> Result<Vec<MetricEvent>, String> {
        self.metrics
            .lock()
            .map_err(|error| format!("metric state lock: {error}"))
            .map(|metrics| metrics.clone())
    }

    pub fn push_metric(&self, event: MetricEvent) -> Result<(), String> {
        self.metrics
            .lock()
            .map_err(|error| format!("metric state lock: {error}"))?
            .push(event.clone());
        if let Some(store) = &self.metric_store {
            store.append(&event)?;
        }
        Ok(())
    }

    pub async fn push_log(&self, entry: LogEntry) -> Result<LogEntryId, String> {
        let entry = self.redact_log_entry(entry)?;
        let id = self.logs.push(entry.clone()).await;
        if let Some(store) = &self.log_store {
            let mut stored = entry;
            stored.id = id;
            store.append(&stored)?;
        }
        Ok(id)
    }

    fn redact_log_entry(&self, mut entry: LogEntry) -> Result<LogEntry, String> {
        let settings = self.trace_settings()?;
        for field in settings.redact_fields {
            entry.message = redact_named_value(&entry.message, &field);
        }
        Ok(entry)
    }

    pub fn trace_settings(&self) -> Result<ManagementTraceSettings, String> {
        self.trace_settings
            .lock()
            .map_err(|error| format!("trace settings lock: {error}"))
            .map(|settings| settings.clone())
    }

    pub fn has_log_store(&self) -> bool {
        self.log_store.is_some()
    }

    pub fn has_trace_settings_store(&self) -> bool {
        self.trace_settings_store.is_some()
    }

    pub fn update_trace_settings(
        &self,
        request: ManagementTraceSettingsUpdateRequest,
    ) -> Result<ManagementTraceSettings, String> {
        let mut settings = self
            .trace_settings
            .lock()
            .map_err(|error| format!("trace settings lock: {error}"))?;
        if let Some(enabled) = request.enabled {
            settings.enabled = enabled;
        }
        if let Some(capture_message_outline) = request.capture_message_outline {
            settings.capture_message_outline = capture_message_outline;
        }
        if let Some(max_events) = request.max_events {
            settings.max_events = max_events.clamp(1, 10_000);
        }
        if let Some(redact_fields) = request.redact_fields {
            settings.redact_fields = redact_fields
                .into_iter()
                .map(|field| field.trim().to_string())
                .filter(|field| !field.is_empty())
                .collect();
        }
        if let Some(store) = &self.trace_settings_store {
            store.save(&settings)?;
        }
        Ok(settings.clone())
    }

    pub async fn stats_since(
        &self,
        since: Option<String>,
    ) -> Result<Vec<PlatformStatsRecord>, String> {
        let Some(repository) = &self.platform_stats else {
            return Ok(Vec::new());
        };
        repository
            .platform_stats_since(since.as_deref().unwrap_or(""))
            .await
            .map_err(|error| error.to_string())
    }

    pub fn uptime_seconds(&self) -> u64 {
        self.started_at
            .elapsed()
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0)
    }
}

#[derive(Clone, Debug)]
pub struct ManagementMetricFileStore {
    path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct ManagementLogFileStore {
    path: PathBuf,
}

impl ManagementLogFileStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<Vec<LogEntry>, String> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let file = fs::File::open(&self.path)
            .map_err(|error| format!("log store open {}: {error}", self.path.display()))?;
        let mut logs = Vec::new();
        for line in BufReader::new(file).lines() {
            let line =
                line.map_err(|error| format!("log store read {}: {error}", self.path.display()))?;
            if line.trim().is_empty() {
                continue;
            }
            logs.push(
                serde_json::from_str(&line)
                    .map_err(|error| format!("log store parse {}: {error}", self.path.display()))?,
            );
        }
        Ok(logs)
    }

    pub fn append(&self, entry: &LogEntry) -> Result<(), String> {
        append_jsonl(&self.path, entry, "log store")
    }
}

#[derive(Clone, Debug)]
pub struct ManagementTraceSettingsFileStore {
    path: PathBuf,
}

impl ManagementTraceSettingsFileStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<Option<ManagementTraceSettings>, String> {
        if !self.path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&self.path)
            .map_err(|error| format!("trace settings read {}: {error}", self.path.display()))?;
        if content.trim().is_empty() {
            return Ok(None);
        }
        serde_json::from_str(&content)
            .map(Some)
            .map_err(|error| format!("trace settings parse {}: {error}", self.path.display()))
    }

    pub fn save(&self, settings: &ManagementTraceSettings) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("trace settings create {}: {error}", parent.display()))?;
        }
        let content = serde_json::to_string_pretty(settings)
            .map_err(|error| format!("trace settings serialize: {error}"))?;
        fs::write(&self.path, content)
            .map_err(|error| format!("trace settings write {}: {error}", self.path.display()))
    }
}

impl ManagementMetricFileStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn load(&self) -> Result<Vec<MetricEvent>, String> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let file = fs::File::open(&self.path)
            .map_err(|error| format!("metric store open {}: {error}", self.path.display()))?;
        let mut metrics = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line
                .map_err(|error| format!("metric store read {}: {error}", self.path.display()))?;
            if line.trim().is_empty() {
                continue;
            }
            metrics.push(
                serde_json::from_str(&line).map_err(|error| {
                    format!("metric store parse {}: {error}", self.path.display())
                })?,
            );
        }
        Ok(metrics)
    }

    pub fn append(&self, event: &MetricEvent) -> Result<(), String> {
        append_jsonl(&self.path, event, "metric store")
    }
}

fn append_jsonl<T: Serialize>(path: &PathBuf, value: &T, label: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("{label} create {}: {error}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("{label} open {}: {error}", path.display()))?;
    let line =
        serde_json::to_string(value).map_err(|error| format!("{label} serialize: {error}"))?;
    writeln!(file, "{line}").map_err(|error| format!("{label} write {}: {error}", path.display()))
}

fn redact_named_value(input: &str, field: &str) -> String {
    if field.trim().is_empty() {
        return input.to_string();
    }
    let lower_input = input.to_ascii_lowercase();
    let lower_field = field.to_ascii_lowercase();
    let mut output = String::with_capacity(input.len());
    let mut start = 0usize;
    let mut search_from = 0usize;
    while let Some(relative_index) = lower_input[search_from..].find(&lower_field) {
        let field_start = search_from + relative_index;
        let field_end = field_start + field.len();
        let Some(separator) = input[field_end..].chars().next() else {
            search_from = field_end;
            continue;
        };
        if separator != '=' && separator != ':' {
            search_from = field_end;
            continue;
        }
        let value_start = field_end + separator.len_utf8();
        let value_end = input[value_start..]
            .find(|ch: char| ch.is_whitespace() || ch == ',' || ch == ';')
            .map(|offset| value_start + offset)
            .unwrap_or(input.len());
        output.push_str(&input[start..value_start]);
        output.push_str("[REDACTED]");
        start = value_end;
        search_from = value_end;
    }
    if start == 0 {
        input.to_string()
    } else {
        output.push_str(&input[start..]);
        output
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementLogQuery {
    pub after: Option<u64>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementLogStreamQuery {
    pub after: Option<u64>,
    pub last_event_id: Option<u64>,
    pub limit: Option<usize>,
    pub interval_ms: Option<u64>,
    pub max_ticks: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementLogResponse {
    pub snapshot: LogBufferSnapshot,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementTraceSettings {
    pub enabled: bool,
    pub capture_message_outline: bool,
    pub max_events: usize,
    pub redact_fields: Vec<String>,
}

impl Default for ManagementTraceSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            capture_message_outline: true,
            max_events: 500,
            redact_fields: vec!["api_key".to_string(), "authorization".to_string()],
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ManagementTraceSettingsUpdateRequest {
    pub enabled: Option<bool>,
    pub capture_message_outline: Option<bool>,
    pub max_events: Option<usize>,
    pub redact_fields: Option<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementTraceResponse {
    pub events: Vec<TraceEvent>,
    pub settings: ManagementTraceSettings,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagementStatsResponse {
    pub generated_at_unix: u64,
    pub uptime_seconds: u64,
    pub log_count: usize,
    pub trace_count: usize,
    pub total_messages: i64,
    pub total_llm_calls: i64,
    pub total_tokens: u64,
    pub total_tts_events: i64,
    pub platform_counts: Vec<ManagementPlatformStatsSummary>,
    pub provider_usage: Vec<ManagementProviderUsageSummary>,
    pub recent_events: Vec<MetricEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementPlatformStatsSummary {
    pub platform_id: String,
    pub platform_type: String,
    pub count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManagementProviderUsageSummary {
    pub provider_id: String,
    pub calls: i64,
    pub total_tokens: u64,
}

pub async fn logs(
    State(state): State<ManagementApiState>,
    Query(query): Query<ManagementLogQuery>,
) -> Result<Json<ManagementLogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let observability = state
        .observability()
        .ok_or_else(observability_unavailable)?;
    let snapshot = observability
        .logs()
        .snapshot(
            query.after.map(LogEntryId),
            query.limit.unwrap_or(100).clamp(1, 500),
        )
        .await;

    Ok(Json(ManagementLogResponse { snapshot }))
}

pub async fn logs_stream(
    State(state): State<ManagementApiState>,
    Query(query): Query<ManagementLogStreamQuery>,
) -> Result<
    Sse<ReceiverStream<std::result::Result<Event, Infallible>>>,
    (StatusCode, Json<ErrorResponse>),
> {
    let observability = state
        .observability()
        .ok_or_else(observability_unavailable)?;
    let logs = observability.logs();
    let limit = query.limit.unwrap_or(25).clamp(1, 100);
    let interval = Duration::from_millis(query.interval_ms.unwrap_or(1_000).clamp(100, 30_000));
    let max_ticks = query.max_ticks.unwrap_or(usize::MAX);
    let (tx, rx) = tokio::sync::mpsc::channel(16);

    tokio::spawn(async move {
        let mut cursor = query.after.or(query.last_event_id).map(LogEntryId);
        let mut ticks = 0usize;
        loop {
            let snapshot = logs.snapshot(cursor, limit).await;
            if let Some(next_cursor) = snapshot.next_cursor {
                cursor = Some(next_cursor);
            }

            if snapshot.entries.is_empty() {
                if tx
                    .send(Ok(Event::default().event("heartbeat").data("ok")))
                    .await
                    .is_err()
                {
                    break;
                }
            } else {
                for entry in snapshot.entries {
                    let Ok(payload) = serde_json::to_string(&entry) else {
                        continue;
                    };
                    if tx
                        .send(Ok(Event::default()
                            .event("log")
                            .id(entry.id.0.to_string())
                            .data(payload)))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            }

            ticks += 1;
            if ticks >= max_ticks {
                break;
            }
            tokio::time::sleep(interval).await;
        }
    });

    Ok(Sse::new(ReceiverStream::new(rx)))
}

pub async fn trace(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementTraceResponse>, (StatusCode, Json<ErrorResponse>)> {
    let observability = state
        .observability()
        .ok_or_else(observability_unavailable)?;
    let settings = observability
        .trace_settings()
        .map_err(observability_error)?;
    let events = apply_trace_settings(
        observability.traces().map_err(observability_error)?,
        &settings,
    );

    Ok(Json(ManagementTraceResponse { events, settings }))
}

pub async fn trace_settings(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementTraceSettings>, (StatusCode, Json<ErrorResponse>)> {
    let observability = state
        .observability()
        .ok_or_else(observability_unavailable)?;
    Ok(Json(
        observability
            .trace_settings()
            .map_err(observability_error)?,
    ))
}

pub async fn update_trace_settings(
    State(state): State<ManagementApiState>,
    Json(request): Json<ManagementTraceSettingsUpdateRequest>,
) -> Result<Json<ManagementTraceSettings>, (StatusCode, Json<ErrorResponse>)> {
    let observability = state
        .observability()
        .ok_or_else(observability_unavailable)?;
    Ok(Json(
        observability
            .update_trace_settings(request)
            .map_err(observability_error)?,
    ))
}

pub async fn stats(
    State(state): State<ManagementApiState>,
) -> Result<Json<ManagementStatsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let observability = state
        .observability()
        .ok_or_else(observability_unavailable)?;
    let metrics = observability.metrics().map_err(observability_error)?;
    let traces = observability.traces().map_err(observability_error)?;
    let log_count = observability.logs().len().await;
    let persisted_platform_stats = observability
        .stats_since(None)
        .await
        .map_err(observability_error)?;
    Ok(Json(stats_response(
        metrics,
        persisted_platform_stats,
        traces.len(),
        log_count,
        observability.uptime_seconds(),
    )))
}

pub async fn push_metric(
    State(state): State<ManagementApiState>,
    Json(event): Json<MetricEvent>,
) -> Result<Json<ManagementStatsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let observability = state
        .observability()
        .ok_or_else(observability_unavailable)?;
    observability
        .push_metric(event)
        .map_err(observability_error)?;
    let metrics = observability.metrics().map_err(observability_error)?;
    let traces = observability.traces().map_err(observability_error)?;
    let log_count = observability.logs().len().await;
    let persisted_platform_stats = observability
        .stats_since(None)
        .await
        .map_err(observability_error)?;
    Ok(Json(stats_response(
        metrics,
        persisted_platform_stats,
        traces.len(),
        log_count,
        observability.uptime_seconds(),
    )))
}

pub async fn push_log(
    State(state): State<ManagementApiState>,
    Json(entry): Json<LogEntry>,
) -> Result<Json<ManagementLogResponse>, (StatusCode, Json<ErrorResponse>)> {
    let observability = state
        .observability()
        .ok_or_else(observability_unavailable)?;
    observability
        .push_log(entry)
        .await
        .map_err(observability_error)?;
    let snapshot = observability.logs().snapshot(None, 100).await;

    Ok(Json(ManagementLogResponse { snapshot }))
}

pub async fn legacy_live_log(
    State(state): State<ManagementApiState>,
    Query(query): Query<ManagementLogStreamQuery>,
) -> Result<
    Sse<ReceiverStream<std::result::Result<Event, Infallible>>>,
    (StatusCode, Json<ErrorResponse>),
> {
    let observability = state
        .observability()
        .ok_or_else(observability_unavailable)?;
    let logs = observability.logs();
    let limit = query.limit.unwrap_or(25).clamp(1, 100);
    let interval = Duration::from_millis(query.interval_ms.unwrap_or(1_000).clamp(100, 30_000));
    let max_ticks = query.max_ticks.unwrap_or(usize::MAX);
    let (tx, rx) = tokio::sync::mpsc::channel(16);

    tokio::spawn(async move {
        let mut cursor = query.after.or(query.last_event_id).map(LogEntryId);
        let mut ticks = 0usize;
        loop {
            let snapshot = logs.snapshot(cursor, limit).await;
            if let Some(next_cursor) = snapshot.next_cursor {
                cursor = Some(next_cursor);
            }

            for entry in snapshot.entries {
                let payload = source_log_entry(&entry);
                let Ok(payload) = serde_json::to_string(&payload) else {
                    continue;
                };
                if tx
                    .send(Ok(Event::default()
                        .id(entry.id.0.to_string())
                        .data(payload)))
                    .await
                    .is_err()
                {
                    return;
                }
            }

            ticks += 1;
            if ticks >= max_ticks {
                break;
            }
            tokio::time::sleep(interval).await;
        }
    });

    Ok(Sse::new(ReceiverStream::new(rx)))
}

pub async fn legacy_log_history(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let observability = state
        .observability()
        .ok_or_else(observability_unavailable)?;
    let snapshot = observability.logs().snapshot(None, 500).await;
    let settings = observability
        .trace_settings()
        .map_err(observability_error)?;
    let traces = apply_trace_settings(
        observability.traces().map_err(observability_error)?,
        &settings,
    );
    let mut rows = snapshot
        .entries
        .iter()
        .map(|entry| SourceHistoryRow {
            time: system_time_secs(entry.occurred_at),
            value: source_log_entry(entry),
        })
        .chain(traces.iter().map(|event| SourceHistoryRow {
            time: system_time_secs(event.occurred_at),
            value: source_trace_event(event),
        }))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.time
            .partial_cmp(&right.time)
            .unwrap_or(Ordering::Equal)
    });
    let logs = rows.into_iter().map(|row| row.value).collect::<Vec<_>>();
    Ok(source_ok(json!({ "logs": logs })))
}

pub async fn legacy_trace_settings(
    State(state): State<ManagementApiState>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let observability = state
        .observability()
        .ok_or_else(observability_unavailable)?;
    let settings = observability
        .trace_settings()
        .map_err(observability_error)?;
    Ok(source_ok(json!({ "trace_enable": settings.enabled })))
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacyTraceSettingsUpdateRequest {
    pub trace_enable: Option<bool>,
}

pub async fn legacy_update_trace_settings(
    State(state): State<ManagementApiState>,
    Json(request): Json<LegacyTraceSettingsUpdateRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    let observability = state
        .observability()
        .ok_or_else(observability_unavailable)?;
    let settings = observability
        .update_trace_settings(ManagementTraceSettingsUpdateRequest {
            enabled: request.trace_enable,
            ..ManagementTraceSettingsUpdateRequest::default()
        })
        .map_err(observability_error)?;
    Ok(source_ok(json!({ "trace_enable": settings.enabled })))
}

fn stats_response(
    metrics: Vec<MetricEvent>,
    persisted_platform_stats: Vec<PlatformStatsRecord>,
    trace_count: usize,
    log_count: usize,
    uptime_seconds: u64,
) -> ManagementStatsResponse {
    let mut total_messages = 0;
    let mut total_llm_calls = 0;
    let mut total_tokens = 0;
    let mut total_tts_events = 0;
    let mut platform_counts: BTreeMap<(String, String), i64> = BTreeMap::new();
    let mut provider_usage: BTreeMap<String, (i64, u64)> = BTreeMap::new();

    for event in &metrics {
        match event.kind {
            MetricEventKind::PlatformMessage => {
                total_messages += event.count;
                if let (Some(platform_id), Some(platform_type)) =
                    (&event.platform_id, &event.platform_type)
                {
                    *platform_counts
                        .entry((platform_id.clone(), platform_type.clone()))
                        .or_default() += event.count;
                }
            }
            MetricEventKind::LlmResponse => {
                total_llm_calls += event.count;
                let tokens = event.usage.map(|usage| usage.total_tokens()).unwrap_or(0);
                total_tokens += tokens;
                if let Some(provider_id) = &event.provider_id {
                    let usage = provider_usage.entry(provider_id.clone()).or_default();
                    usage.0 += event.count;
                    usage.1 += tokens;
                }
            }
            MetricEventKind::TtsPlayback => {
                total_tts_events += event.count;
            }
            MetricEventKind::Custom => {}
        }
    }

    for record in persisted_platform_stats {
        total_messages += record.count;
        *platform_counts
            .entry((record.platform_id, record.platform_type))
            .or_default() += record.count;
    }

    let recent_events = metrics
        .iter()
        .rev()
        .take(20)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    ManagementStatsResponse {
        generated_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0),
        uptime_seconds,
        log_count,
        trace_count,
        total_messages,
        total_llm_calls,
        total_tokens,
        total_tts_events,
        platform_counts: platform_counts
            .into_iter()
            .map(
                |((platform_id, platform_type), count)| ManagementPlatformStatsSummary {
                    platform_id,
                    platform_type,
                    count,
                },
            )
            .collect(),
        provider_usage: provider_usage
            .into_iter()
            .map(
                |(provider_id, (calls, total_tokens))| ManagementProviderUsageSummary {
                    provider_id,
                    calls,
                    total_tokens,
                },
            )
            .collect(),
        recent_events,
    }
}

struct SourceHistoryRow {
    time: f64,
    value: Value,
}

fn source_log_entry(entry: &LogEntry) -> Value {
    let time = system_time_secs(entry.occurred_at);
    let level = source_log_level(entry.level);
    json!({
        "id": entry.id.0,
        "type": "log",
        "time": time,
        "level": level,
        "data": entry.message,
        "message": entry.message,
        "source": format!("{:?}", entry.source),
        "target": entry.target,
        "occurred_at_unix": time,
    })
}

fn source_trace_event(event: &TraceEvent) -> Value {
    let time = system_time_secs(event.occurred_at);
    let fields = event
        .fields
        .iter()
        .map(|(key, value)| (key.clone(), Value::String(value.clone())))
        .collect::<serde_json::Map<_, _>>();
    json!({
        "type": "trace",
        "time": time,
        "span_id": event.span_id,
        "name": event.span_name,
        "span_name": event.span_name,
        "action": event.action,
        "umo": event.message_origin,
        "message_origin": event.message_origin,
        "sender_name": event.sender_name,
        "message_outline": event.message_outline,
        "fields": fields,
        "elapsed_ms": event.elapsed.map(|elapsed| elapsed.as_millis()),
    })
}

fn source_log_level(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Trace => "TRACE",
        LogLevel::Debug => "DEBUG",
        LogLevel::Info => "INFO",
        LogLevel::Warn => "WARNING",
        LogLevel::Error => "ERROR",
    }
}

fn system_time_secs(time: SystemTime) -> f64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs_f64())
        .unwrap_or(0.0)
}

fn apply_trace_settings(
    mut events: Vec<TraceEvent>,
    settings: &ManagementTraceSettings,
) -> Vec<TraceEvent> {
    for event in &mut events {
        if !settings.capture_message_outline {
            event.message_outline = None;
        }
        redact_trace_event(event, settings);
    }
    if events.len() > settings.max_events {
        events.split_off(events.len() - settings.max_events)
    } else {
        events
    }
}

fn redact_trace_event(event: &mut TraceEvent, settings: &ManagementTraceSettings) {
    for field in &settings.redact_fields {
        let lower_field = field.to_ascii_lowercase();
        for (key, value) in &mut event.fields {
            if key.to_ascii_lowercase() == lower_field {
                *value = "[REDACTED]".to_string();
            } else {
                *value = redact_named_value(value, field);
            }
        }
        if let Some(outline) = &mut event.message_outline {
            *outline = redact_named_value(outline, field);
        }
    }
}

fn observability_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "management observability state is not configured".to_string(),
        }),
    )
}

fn observability_error(message: String) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse { error: message }),
    )
}

fn source_ok(data: Value) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "message": null,
        "data": data,
    }))
}
