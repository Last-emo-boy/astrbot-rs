use std::sync::{Arc, Mutex};

use astrbot_core::{
    AstrbotError, MessageChain, MessageEvent, MessageSender, MessageSession, MessageSessionKind,
    MessageSink, Result,
};
use async_trait::async_trait;
use serde_json::{Value, json};

use crate::{ActiveAgentCronPayload, CronJob};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProactiveAgentWakeRequest {
    pub job_id: String,
    pub job_name: String,
    pub session: MessageSession,
    pub sender: MessageSender,
    pub note: String,
    pub extras: Value,
}

impl ProactiveAgentWakeRequest {
    pub fn from_job(job: &CronJob) -> Result<Self> {
        let payload = job.active_agent_payload().ok_or_else(|| {
            AstrbotError::Pipeline(format!(
                "cron job {} is missing active-agent payload",
                job.job_id
            ))
        })?;
        Self::from_payload(job, payload)
    }

    fn from_payload(job: &CronJob, payload: ActiveAgentCronPayload) -> Result<Self> {
        let session = parse_session(&payload.session)?;
        let sender_id = payload
            .sender_id
            .filter(|sender_id| !sender_id.trim().is_empty())
            .unwrap_or_else(|| "astrbot".to_string());
        let note = if payload.note.trim().is_empty() {
            job.description
                .clone()
                .filter(|description| !description.trim().is_empty())
                .unwrap_or_else(|| job.name.clone())
        } else {
            payload.note
        };

        Ok(Self {
            job_id: job.job_id.clone(),
            job_name: job.name.clone(),
            session,
            sender: MessageSender::new(sender_id, Some("Scheduler".to_string())),
            note,
            extras: json!({
                "cron_job": {
                    "id": job.job_id,
                    "name": job.name,
                    "type": "active_agent",
                    "run_once": job.schedule.is_run_once(),
                    "description": job.description,
                },
                "cron_payload": job.payload,
            }),
        })
    }
}

#[async_trait]
pub trait CronEventSink: Send + Sync {
    async fn submit(&self, event: MessageEvent) -> Result<()>;
}

#[derive(Clone)]
pub struct ProactiveAgentWakeService {
    sink: Arc<dyn CronEventSink>,
    message_sink: Arc<dyn MessageSink>,
}

impl ProactiveAgentWakeService {
    pub fn new(sink: Arc<dyn CronEventSink>, message_sink: Arc<dyn MessageSink>) -> Self {
        Self { sink, message_sink }
    }

    pub async fn wake(&self, request: ProactiveAgentWakeRequest) -> Result<()> {
        let event = self.build_event(request);
        self.sink.submit(event).await
    }

    pub fn build_event(&self, request: ProactiveAgentWakeRequest) -> MessageEvent {
        let mut event = MessageEvent::new(
            format!("cron:{}", request.job_id),
            request.session.platform_id.clone(),
            "cron",
            request.session,
            request.sender,
            MessageChain::plain(request.note),
            self.message_sink.clone(),
        );
        event.mark_wake(true);
        event
    }
}

#[derive(Default)]
pub struct RecordingCronEventSink {
    events: Mutex<Vec<MessageEvent>>,
}

impl RecordingCronEventSink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> Vec<MessageEvent> {
        self.events.lock().expect("cron events lock").clone()
    }
}

#[async_trait]
impl CronEventSink for RecordingCronEventSink {
    async fn submit(&self, event: MessageEvent) -> Result<()> {
        self.events.lock().expect("cron events lock").push(event);
        Ok(())
    }
}

fn parse_session(session: &str) -> Result<MessageSession> {
    let mut parts = session.splitn(3, ':');
    let platform_id = parts.next().unwrap_or_default();
    let conversation_id = parts.next().unwrap_or_default();
    let kind = parts.next();
    if platform_id.trim().is_empty() || conversation_id.trim().is_empty() {
        return Err(AstrbotError::Pipeline(format!(
            "invalid cron session: {session}"
        )));
    }

    let session = MessageSession::new(platform_id, conversation_id).with_kind(match kind {
        Some("group") => MessageSessionKind::Group,
        _ => MessageSessionKind::Direct,
    });
    Ok(session)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use astrbot_core::{MessageChain, MessageSession, MessageSink, MessageStream};
    use async_trait::async_trait;

    use crate::{ActiveAgentCronPayload, CronJob, CronJobSchedule};

    use super::{
        ProactiveAgentWakeRequest, ProactiveAgentWakeService, RecordingCronEventSink, Result,
    };

    struct NoopMessageSink;

    #[async_trait]
    impl MessageSink for NoopMessageSink {
        async fn send(&self, _session: &MessageSession, _chain: MessageChain) -> Result<()> {
            Ok(())
        }

        async fn send_streaming(
            &self,
            _session: &MessageSession,
            _stream: MessageStream,
        ) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn proactive_wake_builds_wake_event_for_pipeline_boundary() {
        let job = CronJob::active_agent(
            "job-1",
            "daily report",
            CronJobSchedule::cron("0 8 * * *"),
            ActiveAgentCronPayload::new("webchat:conversation-1:group", "prepare report"),
        );
        let request = ProactiveAgentWakeRequest::from_job(&job).expect("request should build");
        let event_sink = Arc::new(RecordingCronEventSink::new());
        let service = ProactiveAgentWakeService::new(event_sink.clone(), Arc::new(NoopMessageSink));

        service.wake(request).await.expect("event should submit");

        let events = event_sink.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, "cron:job-1");
        assert_eq!(events[0].platform_name, "cron");
        assert_eq!(events[0].session.conversation_id, "conversation-1");
        assert!(events[0].session.is_group());
        assert!(events[0].is_wake());
        assert!(events[0].is_at_or_wake_command());
        assert_eq!(events[0].message_outline(), "prepare report");
    }

    #[test]
    fn proactive_request_rejects_invalid_session() {
        let job = CronJob::active_agent(
            "job-1",
            "bad",
            CronJobSchedule::cron("0 8 * * *"),
            ActiveAgentCronPayload::new("missing-conversation", "prepare report"),
        );

        let err =
            ProactiveAgentWakeRequest::from_job(&job).expect_err("invalid session should fail");
        assert!(err.to_string().contains("invalid cron session"));
    }
}
