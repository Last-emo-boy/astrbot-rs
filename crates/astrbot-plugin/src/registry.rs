use astrbot_core::{MessageEvent, Result};

use crate::event::{PluginControl, PluginEventType};
use crate::handler::RegisteredHandler;

#[derive(Clone, Default)]
pub struct PluginRegistry {
    handlers: Vec<RegisteredHandler>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_handler(&mut self, handler: RegisteredHandler) {
        self.handlers.push(handler);
        self.handlers
            .sort_by_key(|handler| -handler.metadata().priority);
    }

    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn handlers(&self) -> &[RegisteredHandler] {
        &self.handlers
    }

    pub async fn handle_event(
        &self,
        event_type: PluginEventType,
        event: &mut MessageEvent,
    ) -> Result<PluginControl> {
        for registered in &self.handlers {
            if !registered.matches(event_type, event) {
                continue;
            }

            let control = registered.handle(event).await?;
            if control == PluginControl::Stop || event.is_stopped() {
                return Ok(PluginControl::Stop);
            }
        }

        Ok(PluginControl::Continue)
    }

    pub async fn terminate(&self) -> Result<()> {
        for registered in &self.handlers {
            registered.terminate().await?;
        }
        Ok(())
    }
}
