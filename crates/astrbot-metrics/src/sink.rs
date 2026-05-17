use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use astrbot_core::{AstrbotError, Result};
use astrbot_storage::{PlatformStatsRecord, PlatformStatsRepository};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::event::{MetricEvent, MetricEventKind};

#[async_trait]
pub trait MetricSink: Send + Sync {
    async fn record(&self, event: MetricEvent) -> Result<()>;
}

#[derive(Clone, Debug, Default)]
pub struct NoopMetricSink;

#[async_trait]
impl MetricSink for NoopMetricSink {
    async fn record(&self, _event: MetricEvent) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct InMemoryMetricSink {
    events: Arc<RwLock<Vec<MetricEvent>>>,
}

impl InMemoryMetricSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> Vec<MetricEvent> {
        self.events.read().expect("metric event lock").clone()
    }
}

#[async_trait]
impl MetricSink for InMemoryMetricSink {
    async fn record(&self, event: MetricEvent) -> Result<()> {
        self.events
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("metric event lock: {err}")))?
            .push(event);
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct FanoutMetricSink {
    sinks: Vec<Arc<dyn MetricSink>>,
}

impl FanoutMetricSink {
    pub fn new(sinks: impl IntoIterator<Item = Arc<dyn MetricSink>>) -> Self {
        Self {
            sinks: sinks.into_iter().collect(),
        }
    }
}

impl std::fmt::Debug for FanoutMetricSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FanoutMetricSink")
            .field("sink_count", &self.sinks.len())
            .finish()
    }
}

#[async_trait]
impl MetricSink for FanoutMetricSink {
    async fn record(&self, event: MetricEvent) -> Result<()> {
        for sink in &self.sinks {
            sink.record(event.clone()).await?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct LocalPlatformStatsSink {
    repository: Arc<dyn PlatformStatsRepository>,
}

impl LocalPlatformStatsSink {
    pub fn new(repository: Arc<dyn PlatformStatsRepository>) -> Self {
        Self { repository }
    }
}

impl std::fmt::Debug for LocalPlatformStatsSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LocalPlatformStatsSink")
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl MetricSink for LocalPlatformStatsSink {
    async fn record(&self, event: MetricEvent) -> Result<()> {
        if event.kind != MetricEventKind::PlatformMessage {
            return Ok(());
        }
        let Some(platform_id) = event.platform_id else {
            return Err(AstrbotError::Pipeline(
                "platform metric event missing platform_id".to_string(),
            ));
        };
        let Some(platform_type) = event.platform_type else {
            return Err(AstrbotError::Pipeline(
                "platform metric event missing platform_type".to_string(),
            ));
        };
        self.repository
            .increment_platform_stats(PlatformStatsRecord::new(
                event.timestamp,
                platform_id,
                platform_type,
                event.count,
            ))
            .await
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallationIdentity {
    pub installation_id: String,
}

impl InstallationIdentity {
    pub fn new(installation_id: impl Into<String>) -> Self {
        let installation_id = installation_id.into().trim().to_string();
        Self {
            installation_id: if installation_id.is_empty() {
                "null".to_string()
            } else {
                installation_id
            },
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MetricRedactionPolicy {
    runtime_version: Option<String>,
    os: Option<String>,
    hostname: Option<String>,
    installation_id: Option<InstallationIdentity>,
}

impl MetricRedactionPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_runtime_version(mut self, runtime_version: impl Into<String>) -> Self {
        self.runtime_version = non_empty_option(runtime_version);
        self
    }

    pub fn with_os(mut self, os: impl Into<String>) -> Self {
        self.os = non_empty_option(os);
        self
    }

    pub fn with_hostname(mut self, hostname: impl Into<String>) -> Self {
        self.hostname = non_empty_option(hostname);
        self
    }

    pub fn with_installation_id(mut self, installation_id: InstallationIdentity) -> Self {
        self.installation_id = Some(installation_id);
        self
    }

    pub fn payload_for(&self, event: &MetricEvent) -> MetricUploadPayload {
        let mut data = BTreeMap::new();
        data.insert("event".to_string(), json!(event.kind));
        data.insert("timestamp".to_string(), json!(event.timestamp));
        data.insert("count".to_string(), json!(event.count));
        if let Some(version) = &self.runtime_version {
            data.insert("v".to_string(), json!(version));
        }
        if let Some(os) = &self.os {
            data.insert("os".to_string(), json!(os));
        }
        if let Some(hostname) = &self.hostname {
            data.insert("hn".to_string(), json!(hostname));
        }
        if let Some(identity) = &self.installation_id {
            data.insert("iid".to_string(), json!(identity.installation_id));
        }
        if let Some(platform_id) = &event.platform_id {
            data.insert("adapter_name".to_string(), json!(platform_id));
        }
        if let Some(platform_type) = &event.platform_type {
            data.insert("adapter_type".to_string(), json!(platform_type));
        }
        if let Some(provider_id) = &event.provider_id {
            data.insert("provider_id".to_string(), json!(provider_id));
        }
        if let Some(model) = &event.provider_model {
            data.insert("model".to_string(), json!(model));
        }
        if let Some(usage) = event.usage {
            data.insert("input_tokens".to_string(), json!(usage.input_tokens()));
            data.insert("output_tokens".to_string(), json!(usage.output_tokens));
            data.insert("total_tokens".to_string(), json!(usage.total_tokens()));
        }
        if let Some(duration_ms) = event.duration_ms {
            data.insert("duration_ms".to_string(), json!(duration_ms));
        }
        if let Some(time_to_first_token_ms) = event.time_to_first_token_ms {
            data.insert(
                "time_to_first_token_ms".to_string(),
                json!(time_to_first_token_ms),
            );
        }
        if let Some(tts) = &event.tts {
            data.insert("tts_total_time_ms".to_string(), json!(tts.total_time_ms));
            data.insert(
                "tts_first_frame_time_ms".to_string(),
                json!(tts.first_frame_time_ms),
            );
        }
        MetricUploadPayload { metrics_data: data }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricUploadPayload {
    pub metrics_data: BTreeMap<String, serde_json::Value>,
}

#[async_trait]
pub trait RemoteMetricUploader: Send + Sync {
    async fn upload_metric(&self, payload: MetricUploadPayload) -> Result<()>;
}

#[derive(Clone)]
pub struct RemoteMetricSink {
    uploader: Arc<dyn RemoteMetricUploader>,
    redaction: MetricRedactionPolicy,
    enabled: bool,
    ignore_upload_errors: bool,
}

impl RemoteMetricSink {
    pub fn new(uploader: Arc<dyn RemoteMetricUploader>) -> Self {
        Self {
            uploader,
            redaction: MetricRedactionPolicy::new(),
            enabled: true,
            ignore_upload_errors: true,
        }
    }

    pub fn with_redaction(mut self, redaction: MetricRedactionPolicy) -> Self {
        self.redaction = redaction;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn with_strict_errors(mut self) -> Self {
        self.ignore_upload_errors = false;
        self
    }
}

impl std::fmt::Debug for RemoteMetricSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RemoteMetricSink")
            .field("enabled", &self.enabled)
            .field("ignore_upload_errors", &self.ignore_upload_errors)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl MetricSink for RemoteMetricSink {
    async fn record(&self, event: MetricEvent) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }
        let payload = self.redaction.payload_for(&event);
        match self.uploader.upload_metric(payload).await {
            Ok(()) => Ok(()),
            Err(error) if self.ignore_upload_errors => {
                let _ = error;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into().trim().to_string();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use astrbot_core::Result;
    use astrbot_storage::{InMemoryPlatformStatsRepository, PlatformStatsRepository};
    use async_trait::async_trait;

    use super::{
        InstallationIdentity, LocalPlatformStatsSink, MetricRedactionPolicy, MetricSink,
        MetricUploadPayload, RemoteMetricSink, RemoteMetricUploader,
    };
    use crate::{MetricEvent, UsageRecord};

    #[tokio::test]
    async fn local_platform_stats_sink_updates_storage_repository_from_metric_event() {
        let repository = Arc::new(InMemoryPlatformStatsRepository::new());
        let sink = LocalPlatformStatsSink::new(repository.clone());

        sink.record(MetricEvent::platform_message(
            "2026-05-17T08:00:00Z",
            "webchat",
            "webchat",
            2,
        ))
        .await
        .expect("metric should record");
        sink.record(MetricEvent::platform_message(
            "2026-05-17T08:00:00Z",
            "webchat",
            "webchat",
            3,
        ))
        .await
        .expect("metric should record");

        assert_eq!(
            repository
                .total_message_count()
                .await
                .expect("stats should load"),
            5
        );
    }

    #[tokio::test]
    async fn remote_metric_sink_uses_redacted_astrbot_payload_shape() {
        let uploader = Arc::new(RecordingUploader::default());
        let sink = RemoteMetricSink::new(uploader.clone()).with_redaction(
            MetricRedactionPolicy::new()
                .with_runtime_version("4.0.0")
                .with_os("windows")
                .with_hostname("host")
                .with_installation_id(InstallationIdentity::new("install-1")),
        );

        sink.record(
            MetricEvent::llm_response("2026-05-17T08:00:00Z", "openai", UsageRecord::new(3, 1, 2))
                .with_conversation_id("conversation-secret")
                .with_session_id("session-secret"),
        )
        .await
        .expect("remote metric should upload");

        let payload = uploader.payloads()[0].clone();
        assert_eq!(payload.metrics_data["v"], "4.0.0");
        assert_eq!(payload.metrics_data["os"], "windows");
        assert_eq!(payload.metrics_data["iid"], "install-1");
        assert_eq!(payload.metrics_data["provider_id"], "openai");
        assert_eq!(payload.metrics_data["total_tokens"], 6);
        assert!(!payload.metrics_data.contains_key("conversation_id"));
        assert!(!payload.metrics_data.contains_key("session_id"));
    }

    #[derive(Default)]
    struct RecordingUploader {
        payloads: std::sync::RwLock<Vec<MetricUploadPayload>>,
    }

    impl RecordingUploader {
        fn payloads(&self) -> Vec<MetricUploadPayload> {
            self.payloads.read().expect("payload lock").clone()
        }
    }

    #[async_trait]
    impl RemoteMetricUploader for RecordingUploader {
        async fn upload_metric(&self, payload: MetricUploadPayload) -> Result<()> {
            self.payloads.write().expect("payload lock").push(payload);
            Ok(())
        }
    }
}
