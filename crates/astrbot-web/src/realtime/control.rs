use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use astrbot_mcp::{McpElicitationRequest, McpElicitationResult};
use astrbot_session::{ActiveEventInterruption, ActiveEventRecord, ActiveEventRegistry};
use serde::{Deserialize, Serialize};

use super::OpenApiChatSubscriptionPlan;

#[derive(Clone, Default)]
pub struct RealtimeControlState {
    inner: Arc<Mutex<RealtimeControlInner>>,
}

#[derive(Default)]
struct RealtimeControlInner {
    subscriptions: BTreeMap<String, RealtimeChatSubscriptionRecord>,
    active_events: ActiveEventRegistry,
    elicitations: BTreeMap<String, RealtimeElicitationRecord>,
    next_elicitation_id: u64,
}

impl RealtimeControlState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_subscription(
        &self,
        plan: OpenApiChatSubscriptionPlan,
        event_id: impl Into<String>,
        key_id: impl Into<String>,
    ) -> Result<RealtimeChatSubscriptionRecord, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|error| format!("realtime control state lock: {error}"))?;
        let event_id = event_id.into();
        let record = RealtimeChatSubscriptionRecord {
            conversation_id: plan.conversation_id.clone(),
            request_id: plan.request_id.clone(),
            event_id: event_id.clone(),
            key_id: key_id.into(),
            status: RealtimeChatSubscriptionStatus::Queued,
            stop_requested: false,
        };
        inner
            .active_events
            .register(event_id, plan.conversation_id.clone());
        inner.subscriptions.insert(plan.request_id, record.clone());
        Ok(record)
    }

    pub fn subscriptions(&self) -> Result<Vec<RealtimeChatSubscriptionRecord>, String> {
        let inner = self
            .inner
            .lock()
            .map_err(|error| format!("realtime control state lock: {error}"))?;
        Ok(inner.subscriptions.values().cloned().collect())
    }

    pub fn subscription(
        &self,
        request_id: &str,
    ) -> Result<Option<RealtimeChatSubscriptionRecord>, String> {
        let inner = self
            .inner
            .lock()
            .map_err(|error| format!("realtime control state lock: {error}"))?;
        Ok(inner.subscriptions.get(request_id.trim()).cloned())
    }

    pub fn request_stop(
        &self,
        request: RealtimeStopRequest,
    ) -> Result<RealtimeStopResponse, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|error| format!("realtime control state lock: {error}"))?;
        let conversation_id = request.conversation_id.trim().to_string();
        let request_id = request.request_id.and_then(non_empty_string);
        let interruption = request
            .interruption
            .unwrap_or(ActiveEventInterruption::RequestAgentStop);
        let mut matched = 0usize;

        for record in inner.subscriptions.values_mut() {
            if record.conversation_id == conversation_id
                && request_id
                    .as_deref()
                    .map_or(true, |request_id| request_id == record.request_id)
            {
                record.status = RealtimeChatSubscriptionStatus::StopRequested;
                record.stop_requested = true;
                matched += 1;
            }
        }

        let interrupted_events =
            inner
                .active_events
                .interrupt_session(&conversation_id, interruption, None);
        Ok(RealtimeStopResponse {
            conversation_id,
            request_id,
            matched_subscriptions: matched,
            interrupted_events,
            status: if matched > 0 || interrupted_events > 0 {
                "stop_requested".to_string()
            } else {
                "not_found".to_string()
            },
        })
    }

    pub fn active_event_record(&self, event_id: &str) -> Result<Option<ActiveEventRecord>, String> {
        let inner = self
            .inner
            .lock()
            .map_err(|error| format!("realtime control state lock: {error}"))?;
        Ok(inner.active_events.record(event_id).cloned())
    }

    pub fn create_elicitation(
        &self,
        request: RealtimeElicitationCreateRequest,
    ) -> Result<RealtimeElicitationRecord, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|error| format!("realtime control state lock: {error}"))?;
        let elicitation_id = request
            .elicitation_id
            .and_then(non_empty_string)
            .unwrap_or_else(|| {
                inner.next_elicitation_id += 1;
                format!("elicitation-{}", inner.next_elicitation_id)
            });
        let record = RealtimeElicitationRecord {
            elicitation_id: elicitation_id.clone(),
            conversation_id: request.conversation_id.and_then(non_empty_string),
            request_id: request.request_id.and_then(non_empty_string),
            request: request.request,
            result: None,
            status: RealtimeElicitationStatus::Pending,
        };
        inner.elicitations.insert(elicitation_id, record.clone());
        Ok(record)
    }

    pub fn respond_elicitation(
        &self,
        request: RealtimeElicitationRespondRequest,
    ) -> Result<Option<RealtimeElicitationRecord>, String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|error| format!("realtime control state lock: {error}"))?;
        let Some(record) = inner.elicitations.get_mut(request.elicitation_id.trim()) else {
            return Ok(None);
        };
        record.status = RealtimeElicitationStatus::Responded;
        record.result = Some(request.result);
        Ok(Some(record.clone()))
    }

    pub fn elicitations(&self) -> Result<Vec<RealtimeElicitationRecord>, String> {
        let inner = self
            .inner
            .lock()
            .map_err(|error| format!("realtime control state lock: {error}"))?;
        Ok(inner.elicitations.values().cloned().collect())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RealtimeChatSubscriptionStatus {
    Queued,
    StopRequested,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RealtimeChatSubscriptionRecord {
    pub conversation_id: String,
    pub request_id: String,
    pub event_id: String,
    pub key_id: String,
    pub status: RealtimeChatSubscriptionStatus,
    pub stop_requested: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RealtimeSubscriptionCatalogResponse {
    pub subscriptions: Vec<RealtimeChatSubscriptionRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RealtimeStopRequest {
    pub conversation_id: String,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub interruption: Option<ActiveEventInterruption>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RealtimeStopResponse {
    pub conversation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub matched_subscriptions: usize,
    pub interrupted_events: usize,
    pub status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RealtimeElicitationCreateRequest {
    #[serde(default)]
    pub elicitation_id: Option<String>,
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub request_id: Option<String>,
    pub request: McpElicitationRequest,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RealtimeElicitationRespondRequest {
    pub elicitation_id: String,
    pub result: McpElicitationResult,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RealtimeElicitationRecord {
    pub elicitation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub request: McpElicitationRequest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<McpElicitationResult>,
    pub status: RealtimeElicitationStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RealtimeElicitationStatus {
    Pending,
    Responded,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct RealtimeElicitationCatalogResponse {
    pub elicitations: Vec<RealtimeElicitationRecord>,
}

fn non_empty_string(value: impl Into<String>) -> Option<String> {
    let value = value.into();
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
