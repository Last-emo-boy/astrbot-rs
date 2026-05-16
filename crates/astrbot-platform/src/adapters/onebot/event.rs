use std::sync::Arc;

use astrbot_core::{MessageChain, MessageEvent, MessageSender, MessageSession};

use crate::RecordingSink;

pub(super) fn build_onebot_event(
    event_id: String,
    platform_id: String,
    platform_name: String,
    session: MessageSession,
    sender_id: impl Into<String>,
    message: MessageChain,
    sink: Arc<RecordingSink>,
) -> MessageEvent {
    MessageEvent::new(
        event_id,
        platform_id,
        platform_name,
        session,
        MessageSender::new(sender_id, None),
        message,
        sink,
    )
}
