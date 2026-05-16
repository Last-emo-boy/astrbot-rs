use std::sync::Arc;

use astrbot_core::{MessageChain, MessageEvent, MessageSender, MessageSession};

use crate::RecordingSink;

pub(super) fn build_webchat_event(
    event_id: String,
    platform_id: String,
    platform_name: String,
    conversation_id: impl Into<String>,
    sender_id: impl Into<String>,
    message: MessageChain,
    sink: Arc<RecordingSink>,
) -> MessageEvent {
    MessageEvent::new(
        event_id,
        platform_id.clone(),
        platform_name,
        MessageSession::new(platform_id, conversation_id),
        MessageSender::new(sender_id, None),
        message,
        sink,
    )
}
