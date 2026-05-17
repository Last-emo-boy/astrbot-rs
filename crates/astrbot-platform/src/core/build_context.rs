use astrbot_core::MessageEvent;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct PlatformBuildContext {
    event_sender: mpsc::Sender<MessageEvent>,
}

impl PlatformBuildContext {
    pub fn new(event_sender: mpsc::Sender<MessageEvent>) -> Self {
        Self { event_sender }
    }

    pub fn event_sender(&self) -> mpsc::Sender<MessageEvent> {
        self.event_sender.clone()
    }
}
