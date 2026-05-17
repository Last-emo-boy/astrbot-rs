use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::usage::UsageRecord;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricEventKind {
    PlatformMessage,
    LlmResponse,
    TtsPlayback,
    Custom,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricEvent {
    pub kind: MetricEventKind,
    pub timestamp: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub count: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<UsageRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_to_first_token_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tts: Option<MetricTtsStats>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
}

impl MetricEvent {
    pub fn platform_message(
        timestamp: impl Into<String>,
        platform_id: impl Into<String>,
        platform_type: impl Into<String>,
        count: i64,
    ) -> Self {
        Self {
            kind: MetricEventKind::PlatformMessage,
            timestamp: timestamp.into(),
            platform_id: non_empty_option(platform_id),
            platform_type: non_empty_option(platform_type),
            provider_id: None,
            provider_model: None,
            conversation_id: None,
            session_id: None,
            count,
            usage: None,
            duration_ms: None,
            time_to_first_token_ms: None,
            tts: None,
            labels: BTreeMap::new(),
        }
    }

    pub fn llm_response(
        timestamp: impl Into<String>,
        provider_id: impl Into<String>,
        usage: UsageRecord,
    ) -> Self {
        Self {
            kind: MetricEventKind::LlmResponse,
            timestamp: timestamp.into(),
            platform_id: None,
            platform_type: None,
            provider_id: non_empty_option(provider_id),
            provider_model: None,
            conversation_id: None,
            session_id: None,
            count: 1,
            usage: Some(usage),
            duration_ms: None,
            time_to_first_token_ms: None,
            tts: None,
            labels: BTreeMap::new(),
        }
    }

    pub fn tts_playback(
        timestamp: impl Into<String>,
        provider_id: impl Into<String>,
        stats: MetricTtsStats,
    ) -> Self {
        Self {
            kind: MetricEventKind::TtsPlayback,
            timestamp: timestamp.into(),
            platform_id: None,
            platform_type: None,
            provider_id: non_empty_option(provider_id),
            provider_model: None,
            conversation_id: None,
            session_id: None,
            count: 1,
            usage: None,
            duration_ms: None,
            time_to_first_token_ms: None,
            tts: Some(stats),
            labels: BTreeMap::new(),
        }
    }

    pub fn custom(timestamp: impl Into<String>) -> Self {
        Self {
            kind: MetricEventKind::Custom,
            timestamp: timestamp.into(),
            platform_id: None,
            platform_type: None,
            provider_id: None,
            provider_model: None,
            conversation_id: None,
            session_id: None,
            count: 1,
            usage: None,
            duration_ms: None,
            time_to_first_token_ms: None,
            tts: None,
            labels: BTreeMap::new(),
        }
    }

    pub fn with_provider_model(mut self, model: impl Into<String>) -> Self {
        self.provider_model = non_empty_option(model);
        self
    }

    pub fn with_conversation_id(mut self, conversation_id: impl Into<String>) -> Self {
        self.conversation_id = non_empty_option(conversation_id);
        self
    }

    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = non_empty_option(session_id);
        self
    }

    pub fn with_duration_ms(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }

    pub fn with_time_to_first_token_ms(mut self, time_to_first_token_ms: u64) -> Self {
        self.time_to_first_token_ms = Some(time_to_first_token_ms);
        self
    }

    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if let Some(key) = non_empty_option(key)
            && let Some(value) = non_empty_option(value)
        {
            self.labels.insert(key, value);
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricTtsStats {
    pub total_time_ms: u64,
    pub first_frame_time_ms: u64,
}

impl MetricTtsStats {
    pub fn new(total_time_ms: u64, first_frame_time_ms: u64) -> Self {
        Self {
            total_time_ms,
            first_frame_time_ms,
        }
    }
}

fn non_empty_option(value: impl Into<String>) -> Option<String> {
    let value = value.into().trim().to_string();
    (!value.is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::{MetricEvent, MetricEventKind, MetricTtsStats};
    use crate::UsageRecord;

    #[test]
    fn metric_events_model_platform_llm_and_tts_without_payload_text() {
        let platform =
            MetricEvent::platform_message("2026-05-17T08:00:00Z", "webchat", "webchat", 2);
        assert_eq!(platform.kind, MetricEventKind::PlatformMessage);
        assert_eq!(platform.platform_id.as_deref(), Some("webchat"));
        assert_eq!(platform.count, 2);

        let llm =
            MetricEvent::llm_response("2026-05-17T08:00:01Z", "openai", UsageRecord::new(10, 2, 5))
                .with_provider_model("gpt")
                .with_conversation_id("conversation-1")
                .with_duration_ms(120)
                .with_time_to_first_token_ms(30);
        assert_eq!(llm.kind, MetricEventKind::LlmResponse);
        assert_eq!(llm.usage.expect("usage").total_tokens(), 17);
        assert_eq!(llm.provider_model.as_deref(), Some("gpt"));

        let tts =
            MetricEvent::tts_playback("2026-05-17T08:00:02Z", "tts", MetricTtsStats::new(250, 40));
        assert_eq!(tts.tts.expect("tts").first_frame_time_ms, 40);
    }
}
