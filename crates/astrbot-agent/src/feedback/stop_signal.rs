use astrbot_core::{MessageEvent, Result};
use async_trait::async_trait;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentStopSignalReason {
    EventStopped,
    UserRequested,
    Cancelled,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AgentStopSignal {
    pub event_stopped: bool,
    pub user_requested: bool,
    pub cancelled: bool,
}

impl AgentStopSignal {
    pub fn from_event(event: &MessageEvent) -> Self {
        Self {
            event_stopped: event.is_stopped(),
            user_requested: false,
            cancelled: false,
        }
    }

    pub fn user_requested(mut self) -> Self {
        self.user_requested = true;
        self
    }

    pub fn cancelled(mut self) -> Self {
        self.cancelled = true;
        self
    }

    pub fn should_stop(&self) -> bool {
        self.reason().is_some()
    }

    pub fn reason(&self) -> Option<AgentStopSignalReason> {
        if self.cancelled {
            Some(AgentStopSignalReason::Cancelled)
        } else if self.user_requested {
            Some(AgentStopSignalReason::UserRequested)
        } else if self.event_stopped {
            Some(AgentStopSignalReason::EventStopped)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentStopSignalPolicy {
    pub allow_user_stop: bool,
    pub allow_cancellation: bool,
}

impl AgentStopSignalPolicy {
    pub fn new() -> Self {
        Self {
            allow_user_stop: true,
            allow_cancellation: true,
        }
    }

    pub fn allow_user_stop(mut self, allow_user_stop: bool) -> Self {
        self.allow_user_stop = allow_user_stop;
        self
    }

    pub fn allow_cancellation(mut self, allow_cancellation: bool) -> Self {
        self.allow_cancellation = allow_cancellation;
        self
    }

    pub fn evaluate(&self, signal: &AgentStopSignal) -> Option<AgentStopSignalReason> {
        if self.allow_cancellation && signal.cancelled {
            return Some(AgentStopSignalReason::Cancelled);
        }
        if self.allow_user_stop && signal.user_requested {
            return Some(AgentStopSignalReason::UserRequested);
        }
        signal
            .event_stopped
            .then_some(AgentStopSignalReason::EventStopped)
    }
}

impl Default for AgentStopSignalPolicy {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
pub trait AgentStopSignalPort: Send + Sync {
    async fn stop_signal(&self, event: &MessageEvent) -> Result<AgentStopSignal>;
}

#[derive(Clone, Debug, Default)]
pub struct EventStopSignalPort;

#[async_trait]
impl AgentStopSignalPort for EventStopSignalPort {
    async fn stop_signal(&self, event: &MessageEvent) -> Result<AgentStopSignal> {
        Ok(AgentStopSignal::from_event(event))
    }
}

#[derive(Clone, Debug, Default)]
pub struct StaticStopSignalPort {
    signal: AgentStopSignal,
}

impl StaticStopSignalPort {
    pub fn new(signal: AgentStopSignal) -> Self {
        Self { signal }
    }
}

#[async_trait]
impl AgentStopSignalPort for StaticStopSignalPort {
    async fn stop_signal(&self, _event: &MessageEvent) -> Result<AgentStopSignal> {
        Ok(self.signal.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use astrbot_core::{MessageChain, MessageEvent, MessageSender, MessageSession, MessageSink};
    use async_trait::async_trait;

    use super::{
        AgentStopSignal, AgentStopSignalPolicy, AgentStopSignalPort, AgentStopSignalReason,
        EventStopSignalPort, StaticStopSignalPort,
    };

    struct NoopSink;

    #[async_trait]
    impl MessageSink for NoopSink {
        async fn send(
            &self,
            _session: &MessageSession,
            _chain: MessageChain,
        ) -> astrbot_core::Result<()> {
            Ok(())
        }
    }

    fn event() -> MessageEvent {
        MessageEvent::new(
            "event-1",
            "webchat",
            "WebChat",
            MessageSession::new("webchat", "conversation-1"),
            MessageSender::new("user-1", None),
            MessageChain::plain("hello"),
            Arc::new(NoopSink),
        )
    }

    #[test]
    fn stop_signal_policy_keeps_user_stop_outside_runner_core() {
        let signal = AgentStopSignal::default().user_requested();

        assert_eq!(
            AgentStopSignalPolicy::default().evaluate(&signal),
            Some(AgentStopSignalReason::UserRequested)
        );
        assert_eq!(
            AgentStopSignalPolicy::new()
                .allow_user_stop(false)
                .evaluate(&signal),
            None
        );
    }

    #[tokio::test]
    async fn stop_signal_ports_are_testable_without_provider_or_respond_stage() {
        let mut event = event();
        event.stop();

        let signal = EventStopSignalPort
            .stop_signal(&event)
            .await
            .expect("event stop signal should resolve");
        assert_eq!(signal.reason(), Some(AgentStopSignalReason::EventStopped));

        let signal = StaticStopSignalPort::new(AgentStopSignal::default().cancelled())
            .stop_signal(&event)
            .await
            .expect("static stop signal should resolve");
        assert_eq!(signal.reason(), Some(AgentStopSignalReason::Cancelled));
    }
}
