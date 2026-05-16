use astrbot_core::MessageChain;

pub(super) fn plain_text_message(text: impl Into<String>) -> MessageChain {
    MessageChain::plain(text)
}
