use astrbot_core::{MessageChain, MessageComponent};

pub(super) fn message_chain_from_text_and_images(
    text: impl Into<String>,
    image_urls: Vec<String>,
) -> MessageChain {
    let text = text.into();
    let mut message = MessageChain::default();
    if !text.trim().is_empty() {
        message.push(MessageComponent::plain(text));
    }
    for image_url in image_urls {
        let image_url = image_url.trim();
        if !image_url.is_empty() {
            message.push(MessageComponent::image(image_url.to_string()));
        }
    }

    message
}
