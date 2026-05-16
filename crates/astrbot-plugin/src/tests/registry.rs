use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use astrbot_core::{MessageEvent, Result};
use async_trait::async_trait;

use crate::{
    HandlerMetadata, PluginControl, PluginEventType, PluginHandler, PluginRegistry,
    RegisteredHandler,
};

use super::event;

struct CountingHandler {
    calls: Arc<AtomicUsize>,
    control: PluginControl,
}

#[async_trait]
impl PluginHandler for CountingHandler {
    async fn handle(&self, _event: &mut MessageEvent) -> Result<PluginControl> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.control)
    }
}

struct TerminatingHandler {
    terminate_count: Arc<AtomicUsize>,
}

#[async_trait]
impl PluginHandler for TerminatingHandler {
    async fn handle(&self, _event: &mut MessageEvent) -> Result<PluginControl> {
        Ok(PluginControl::Continue)
    }

    async fn terminate(&self) -> Result<()> {
        self.terminate_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn registry_orders_handlers_and_stops_on_stop_control() {
    let low_calls = Arc::new(AtomicUsize::new(0));
    let high_calls = Arc::new(AtomicUsize::new(0));
    let mut registry = PluginRegistry::new();
    registry.register_handler(RegisteredHandler::new(
        HandlerMetadata::new("plugin", "low", PluginEventType::AdapterMessage).with_priority(0),
        Arc::new(CountingHandler {
            calls: low_calls.clone(),
            control: PluginControl::Continue,
        }),
    ));
    registry.register_handler(RegisteredHandler::new(
        HandlerMetadata::new("plugin", "high", PluginEventType::AdapterMessage).with_priority(10),
        Arc::new(CountingHandler {
            calls: high_calls.clone(),
            control: PluginControl::Stop,
        }),
    ));

    let mut event = event("/ping");
    let control = registry
        .handle_event(PluginEventType::AdapterMessage, &mut event)
        .await
        .expect("registry should handle event");

    assert_eq!(control, PluginControl::Stop);
    assert_eq!(high_calls.load(Ordering::SeqCst), 1);
    assert_eq!(low_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn registry_terminates_registered_handlers() {
    let terminate_count = Arc::new(AtomicUsize::new(0));
    let mut registry = PluginRegistry::new();
    registry.register_handler(RegisteredHandler::new(
        HandlerMetadata::new("plugin", "handler", PluginEventType::AdapterMessage),
        Arc::new(TerminatingHandler {
            terminate_count: terminate_count.clone(),
        }),
    ));

    registry
        .terminate()
        .await
        .expect("registry should terminate handlers");

    assert_eq!(terminate_count.load(Ordering::SeqCst), 1);
}
