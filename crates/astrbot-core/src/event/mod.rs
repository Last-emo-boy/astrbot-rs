use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::{MessageEvent, Result};

mod logging;
mod router;

pub use logging::{EventLogRecord, EventLogger, NoopEventLogger};
pub use router::{EventRouter, SingleExecutorRouter};

#[async_trait]
pub trait EventExecutor: Send + Sync {
    async fn execute(&self, event: MessageEvent) -> Result<()>;
}

pub struct EventBus {
    receiver: mpsc::Receiver<MessageEvent>,
    router: Arc<dyn EventRouter>,
    logger: Arc<dyn EventLogger>,
}

impl EventBus {
    pub fn new(receiver: mpsc::Receiver<MessageEvent>, executor: Arc<dyn EventExecutor>) -> Self {
        Self::with_router(receiver, Arc::new(SingleExecutorRouter::new(executor)))
    }

    pub fn with_router(
        receiver: mpsc::Receiver<MessageEvent>,
        router: Arc<dyn EventRouter>,
    ) -> Self {
        Self {
            receiver,
            router,
            logger: Arc::new(NoopEventLogger),
        }
    }

    pub fn with_logger(mut self, logger: Arc<dyn EventLogger>) -> Self {
        self.logger = logger;
        self
    }

    pub async fn run_once(&mut self) -> Result<bool> {
        let Some(event) = self.receiver.recv().await else {
            return Ok(false);
        };

        let record = EventLogRecord::from_event(&event);
        self.logger.log_event(&record);
        let executor = self.router.route(&event)?;
        executor.execute(event).await?;
        Ok(true)
    }

    pub async fn run(mut self) -> Result<()> {
        while self.run_once().await? {}
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use tokio::sync::mpsc;

    use crate::{
        MessageChain, MessageComponent, MessageEvent, MessageSender, MessageSession, MessageSink,
        MessageStream, Result,
    };

    use super::{
        EventBus, EventExecutor, EventLogRecord, EventLogger, EventRouter, SingleExecutorRouter,
    };

    struct NoopSink;

    #[async_trait]
    impl MessageSink for NoopSink {
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

    #[derive(Default)]
    struct RecordingExecutor {
        events: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl EventExecutor for RecordingExecutor {
        async fn execute(&self, event: MessageEvent) -> Result<()> {
            self.events.lock().expect("events lock").push(event.id);
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordingLogger {
        records: Mutex<Vec<EventLogRecord>>,
    }

    impl EventLogger for RecordingLogger {
        fn log_event(&self, record: &EventLogRecord) {
            self.records
                .lock()
                .expect("records lock")
                .push(record.clone());
        }
    }

    struct RecordingRouter {
        executor: Arc<dyn EventExecutor>,
        routed_events: Mutex<Vec<String>>,
    }

    impl RecordingRouter {
        fn new(executor: Arc<dyn EventExecutor>) -> Self {
            Self {
                executor,
                routed_events: Mutex::new(Vec::new()),
            }
        }
    }

    impl EventRouter for RecordingRouter {
        fn route(&self, event: &MessageEvent) -> Result<Arc<dyn EventExecutor>> {
            self.routed_events
                .lock()
                .expect("routed events lock")
                .push(event.id.clone());
            Ok(self.executor.clone())
        }
    }

    fn message_event(id: &str) -> MessageEvent {
        MessageEvent::new(
            id,
            "mock",
            "Mock Platform",
            MessageSession::new("mock", "conversation-1"),
            MessageSender::new("user-1", Some("Alice".to_string())),
            MessageChain::new(vec![MessageComponent::plain("hello")]),
            Arc::new(NoopSink),
        )
    }

    #[test]
    fn event_log_record_captures_platform_session_sender_and_outline() {
        let record = EventLogRecord::from_event(&message_event("event-1"));

        assert_eq!(record.event_id, "event-1");
        assert_eq!(record.platform_id, "mock");
        assert_eq!(record.platform_name, "Mock Platform");
        assert_eq!(record.conversation_id, "conversation-1");
        assert_eq!(record.sender_id, "user-1");
        assert_eq!(record.sender_name.as_deref(), Some("Alice"));
        assert_eq!(record.message_outline, "hello");
        assert_eq!(
            record.display_line(),
            "[mock(Mock Platform)] Alice/user-1: hello"
        );
    }

    #[test]
    fn single_executor_router_returns_the_configured_executor() {
        let executor: Arc<dyn EventExecutor> = Arc::new(RecordingExecutor::default());
        let router = SingleExecutorRouter::new(executor.clone());

        let routed = router
            .route(&message_event("event-1"))
            .expect("single executor should route");

        assert!(Arc::ptr_eq(&executor, &routed));
    }

    #[tokio::test]
    async fn event_bus_logs_routes_and_executes_one_event() {
        let (tx, rx) = mpsc::channel(1);
        let executor = Arc::new(RecordingExecutor::default());
        let router = Arc::new(RecordingRouter::new(executor.clone()));
        let logger = Arc::new(RecordingLogger::default());
        let mut bus = EventBus::with_router(rx, router.clone()).with_logger(logger.clone());

        tx.send(message_event("event-1"))
            .await
            .expect("event should be queued");
        drop(tx);

        assert!(bus.run_once().await.expect("event should run"));
        assert_eq!(
            router
                .routed_events
                .lock()
                .expect("routed events lock")
                .as_slice(),
            ["event-1"]
        );
        assert_eq!(
            executor.events.lock().expect("events lock").as_slice(),
            ["event-1"]
        );
        assert_eq!(
            logger.records.lock().expect("records lock")[0].display_line(),
            "[mock(Mock Platform)] Alice/user-1: hello"
        );
        assert!(!bus.run_once().await.expect("closed bus should stop"));
    }
}
