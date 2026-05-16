use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use astrbot_core::{
    EventExecutor, MessageChain, MessageEvent, MessageSender, MessageSession, Result,
};
use astrbot_pipeline::{
    PipelineContext, PipelineScheduler, RateLimitConfig, RateLimitStrategy, SessionStatusPort,
    WhitelistPolicyConfig,
    stages::{
        ProviderStage, RateLimitStage, RespondStage, SessionStatusCheckStage, WhitelistCheckStage,
    },
};
use astrbot_platform::RecordingSink;
use astrbot_provider::{ChatProvider, ChatRequest, ChatResponse};
use async_trait::async_trait;
use tokio::sync::Mutex;

#[tokio::test]
async fn whitelist_stage_stops_non_allowed_sessions_before_provider() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone()).with_whitelist_policy(
            WhitelistPolicyConfig::enabled(["allowed-session"])
                .with_bypass_platform_ids(["webchat"]),
        ),
    )
    .with_stage(WhitelistCheckStage)
    .with_stage(ProviderStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event(
            "mock",
            "blocked-session",
            "user-1",
            sink.clone(),
        ))
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert!(sink.messages().await.is_empty());
}

#[tokio::test]
async fn whitelist_stage_bypasses_configured_platforms() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone()).with_whitelist_policy(
            WhitelistPolicyConfig::enabled(["allowed-session"])
                .with_bypass_platform_ids(["webchat"]),
        ),
    )
    .with_stage(WhitelistCheckStage)
    .with_stage(ProviderStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event(
            "webchat",
            "blocked-session",
            "user-1",
            sink.clone(),
        ))
        .await
        .expect("scheduler should execute");

    assert_eq!(provider.requests.lock().await.len(), 1);
    assert_eq!(sink.messages().await[0].chain.plain_text(), "mock-response");
}

#[tokio::test]
async fn session_status_stage_stops_disabled_sessions_before_provider() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let session_status = Arc::new(StaticSessionStatusPort::disabled(["blocked-session"]));
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone())
            .with_session_status_port(session_status),
    )
    .with_stage(SessionStatusCheckStage)
    .with_stage(ProviderStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event(
            "mock",
            "blocked-session",
            "user-1",
            sink.clone(),
        ))
        .await
        .expect("scheduler should execute");

    assert!(provider.requests.lock().await.is_empty());
    assert!(sink.messages().await.is_empty());
}

#[tokio::test]
async fn rate_limit_stage_discards_events_over_the_window_limit() {
    let provider = Arc::new(CapturingProvider::default());
    let sink = Arc::new(RecordingSink::default());
    let scheduler = PipelineScheduler::new(
        PipelineContext::with_chat_provider(provider.clone()).with_rate_limit(
            RateLimitConfig::fixed_window(1, Duration::from_secs(60), RateLimitStrategy::Discard),
        ),
    )
    .with_stage(RateLimitStage::default())
    .with_stage(ProviderStage)
    .with_stage(RespondStage);

    scheduler
        .execute(direct_event(
            "mock",
            "conversation-1",
            "user-1",
            sink.clone(),
        ))
        .await
        .expect("first event should execute");
    scheduler
        .execute(direct_event(
            "mock",
            "conversation-1",
            "user-1",
            sink.clone(),
        ))
        .await
        .expect("second event should execute");

    assert_eq!(provider.requests.lock().await.len(), 1);
    assert_eq!(sink.messages().await.len(), 1);
}

fn direct_event(
    platform_id: impl Into<String>,
    conversation_id: impl Into<String>,
    sender_id: impl Into<String>,
    sink: Arc<RecordingSink>,
) -> MessageEvent {
    let platform_id = platform_id.into();
    MessageEvent::new(
        "event-1",
        platform_id.clone(),
        "Test Platform",
        MessageSession::new(platform_id, conversation_id),
        MessageSender::new(sender_id, None),
        MessageChain::plain("hello"),
        sink,
    )
}

#[derive(Default)]
struct CapturingProvider {
    requests: Mutex<Vec<ChatRequest>>,
}

#[async_trait]
impl ChatProvider for CapturingProvider {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        self.requests.lock().await.push(request);
        Ok(ChatResponse::text("mock-response"))
    }
}

struct StaticSessionStatusPort {
    disabled: HashSet<String>,
}

impl StaticSessionStatusPort {
    fn disabled<I, S>(sessions: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            disabled: sessions.into_iter().map(Into::into).collect(),
        }
    }
}

#[async_trait]
impl SessionStatusPort for StaticSessionStatusPort {
    async fn is_session_enabled(&self, event: &MessageEvent) -> Result<bool> {
        Ok(!self.disabled.contains(&event.session.conversation_id))
    }
}
