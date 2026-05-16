use astrbot_storage::ConversationMessageRecord;

use crate::WebChatMessagesResponse;
use crate::message_parts::webchat_message_response_from_chain;

pub(crate) fn webchat_message_records_response(
    conversation_id: String,
    messages: Vec<ConversationMessageRecord>,
) -> WebChatMessagesResponse {
    WebChatMessagesResponse {
        conversation_id,
        messages: messages
            .into_iter()
            .map(|record| webchat_message_response_from_chain(&record.chain))
            .collect(),
    }
}
