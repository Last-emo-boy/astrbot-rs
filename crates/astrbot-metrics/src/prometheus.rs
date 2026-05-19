//! Prometheus exposition-format exporter.
//!
//! Converts in-memory [`MetricEvent`] aggregates into the canonical
//! `text/plain; version=0.0.4` format Prometheus' scrape protocol expects.
//!
//! ```text
//! # HELP astrbot_platform_messages_total Number of inbound platform messages
//! # TYPE astrbot_platform_messages_total counter
//! astrbot_platform_messages_total{platform_id="tg1",platform_type="telegram"} 42
//! ```
//!
//! The exporter is intentionally synchronous: callers feed it a slice of
//! [`MetricEvent`] and receive a string they can write straight to an HTTP
//! response body.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::event::{MetricEvent, MetricEventKind};

/// Render a snapshot of events into Prometheus exposition format.
pub fn render_prometheus(events: &[MetricEvent]) -> String {
    let mut platform = LabeledCounter::new(
        "astrbot_platform_messages_total",
        "Number of inbound platform messages observed.",
    );
    let mut llm = LabeledCounter::new(
        "astrbot_llm_responses_total",
        "Number of LLM responses produced.",
    );
    let mut tts = LabeledCounter::new(
        "astrbot_tts_playbacks_total",
        "Number of TTS audio playbacks dispatched.",
    );
    let mut llm_latency = LabeledSum::new(
        "astrbot_llm_response_latency_ms_sum",
        "Cumulative LLM response latency in milliseconds.",
    );
    let mut llm_ttft = LabeledSum::new(
        "astrbot_llm_time_to_first_token_ms_sum",
        "Cumulative LLM time-to-first-token in milliseconds.",
    );
    let mut prompt_tokens = LabeledSum::new(
        "astrbot_llm_prompt_tokens_total",
        "Cumulative prompt tokens consumed by LLM calls.",
    );
    let mut completion_tokens = LabeledSum::new(
        "astrbot_llm_completion_tokens_total",
        "Cumulative completion tokens produced by LLM calls.",
    );

    for event in events {
        let labels = primary_labels(event);
        match event.kind {
            MetricEventKind::PlatformMessage => platform.add(labels.clone(), event.count as f64),
            MetricEventKind::LlmResponse => {
                llm.add(labels.clone(), event.count as f64);
                if let Some(duration_ms) = event.duration_ms {
                    llm_latency.add(labels.clone(), duration_ms as f64);
                }
                if let Some(ttft_ms) = event.time_to_first_token_ms {
                    llm_ttft.add(labels.clone(), ttft_ms as f64);
                }
                if let Some(usage) = &event.usage {
                    prompt_tokens.add(labels.clone(), usage.input_tokens() as f64);
                    completion_tokens.add(labels.clone(), usage.output_tokens as f64);
                }
            }
            MetricEventKind::TtsPlayback => tts.add(labels.clone(), event.count as f64),
            MetricEventKind::Custom => {}
        }
    }

    let mut out = String::new();
    platform.write(&mut out, "counter");
    llm.write(&mut out, "counter");
    tts.write(&mut out, "counter");
    llm_latency.write(&mut out, "counter");
    llm_ttft.write(&mut out, "counter");
    prompt_tokens.write(&mut out, "counter");
    completion_tokens.write(&mut out, "counter");
    out
}

fn primary_labels(event: &MetricEvent) -> Vec<(String, String)> {
    let mut labels: Vec<(String, String)> = Vec::new();
    if let Some(value) = &event.platform_id {
        labels.push(("platform_id".into(), value.clone()));
    }
    if let Some(value) = &event.platform_type {
        labels.push(("platform_type".into(), value.clone()));
    }
    if let Some(value) = &event.provider_id {
        labels.push(("provider_id".into(), value.clone()));
    }
    if let Some(value) = &event.provider_model {
        labels.push(("provider_model".into(), value.clone()));
    }
    labels
}

struct LabeledCounter {
    name: &'static str,
    help: &'static str,
    series: BTreeMap<String, f64>,
}

impl LabeledCounter {
    fn new(name: &'static str, help: &'static str) -> Self {
        Self {
            name,
            help,
            series: BTreeMap::new(),
        }
    }

    fn add(&mut self, labels: Vec<(String, String)>, value: f64) {
        let key = label_key(&labels);
        *self.series.entry(key).or_insert(0.0) += value;
    }

    fn write(&self, out: &mut String, metric_type: &str) {
        if self.series.is_empty() {
            return;
        }
        let _ = writeln!(out, "# HELP {} {}", self.name, self.help);
        let _ = writeln!(out, "# TYPE {} {}", self.name, metric_type);
        for (key, value) in &self.series {
            let _ = writeln!(out, "{}{} {}", self.name, key, format_float(*value));
        }
    }
}

struct LabeledSum {
    name: &'static str,
    help: &'static str,
    series: BTreeMap<String, f64>,
}

impl LabeledSum {
    fn new(name: &'static str, help: &'static str) -> Self {
        Self {
            name,
            help,
            series: BTreeMap::new(),
        }
    }

    fn add(&mut self, labels: Vec<(String, String)>, value: f64) {
        let key = label_key(&labels);
        *self.series.entry(key).or_insert(0.0) += value;
    }

    fn write(&self, out: &mut String, metric_type: &str) {
        if self.series.is_empty() {
            return;
        }
        let _ = writeln!(out, "# HELP {} {}", self.name, self.help);
        let _ = writeln!(out, "# TYPE {} {}", self.name, metric_type);
        for (key, value) in &self.series {
            let _ = writeln!(out, "{}{} {}", self.name, key, format_float(*value));
        }
    }
}

fn label_key(labels: &[(String, String)]) -> String {
    if labels.is_empty() {
        return String::new();
    }
    let mut sorted: Vec<&(String, String)> = labels.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = String::from("{");
    for (i, (key, value)) in sorted.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{}=\"{}\"", key, escape_label_value(value));
    }
    out.push('}');
    out
}

fn escape_label_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out
}

fn format_float(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::UsageRecord;

    fn now() -> String {
        "2026-05-20T00:00:00Z".to_string()
    }

    #[test]
    fn empty_event_list_renders_empty_string() {
        assert!(render_prometheus(&[]).is_empty());
    }

    #[test]
    fn platform_messages_aggregate_per_label_set() {
        let events = vec![
            MetricEvent::platform_message(now(), "tg1", "telegram", 1),
            MetricEvent::platform_message(now(), "tg1", "telegram", 2),
            MetricEvent::platform_message(now(), "wx1", "weixin_official_account", 1),
        ];
        let rendered = render_prometheus(&events);
        assert!(rendered.contains(
            "astrbot_platform_messages_total{platform_id=\"tg1\",platform_type=\"telegram\"} 3"
        ));
        assert!(rendered.contains(
            "astrbot_platform_messages_total{platform_id=\"wx1\",platform_type=\"weixin_official_account\"} 1"
        ));
    }

    #[test]
    fn llm_response_renders_latency_and_tokens() {
        let mut event = MetricEvent {
            kind: MetricEventKind::LlmResponse,
            timestamp: now(),
            platform_id: None,
            platform_type: None,
            provider_id: Some("openai-1".into()),
            provider_model: Some("gpt-4".into()),
            conversation_id: None,
            session_id: None,
            count: 1,
            usage: Some(UsageRecord::new(100, 0, 50)),
            duration_ms: Some(1200),
            time_to_first_token_ms: Some(180),
            tts: None,
            labels: BTreeMap::new(),
        };
        event.labels.insert("region".into(), "us".into());

        let rendered = render_prometheus(&[event]);
        assert!(rendered.contains("astrbot_llm_responses_total{"));
        assert!(rendered.contains("astrbot_llm_response_latency_ms_sum"));
        assert!(rendered.contains("1200"));
        assert!(rendered.contains("astrbot_llm_prompt_tokens_total"));
        assert!(rendered.contains("100"));
        assert!(rendered.contains("astrbot_llm_completion_tokens_total"));
        assert!(rendered.contains("50"));
    }

    #[test]
    fn label_values_are_escaped() {
        let mut event = MetricEvent::platform_message(now(), "tg \"1\"", "telegram", 1);
        event.platform_id = Some("tg \"1\"".into());
        let rendered = render_prometheus(&[event]);
        assert!(rendered.contains("platform_id=\"tg \\\"1\\\"\""));
    }
}
