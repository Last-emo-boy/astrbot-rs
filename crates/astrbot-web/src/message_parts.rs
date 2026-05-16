use astrbot_core::{MessageChain, MessageComponent};

use crate::{WebChatMessagePart, WebChatMessageResponse};

pub(crate) fn message_chain_from_submit_payload(
    text: String,
    message_parts: Vec<WebChatMessagePart>,
    image_urls: Vec<String>,
) -> MessageChain {
    let mut message = MessageChain::default();
    if !text.trim().is_empty() {
        message.push(MessageComponent::plain(text));
    }

    for part in message_parts {
        match part {
            WebChatMessagePart::Plain { text } => {
                if !text.trim().is_empty() {
                    message.push(MessageComponent::plain(text));
                }
            }
            WebChatMessagePart::Image { url } => {
                let url = url.trim();
                if !url.is_empty() {
                    message.push(MessageComponent::image(url.to_string()));
                }
            }
            WebChatMessagePart::Reply {
                message_id,
                selected_text,
            } => {
                let message_id = message_id.trim();
                if !message_id.is_empty() {
                    message.push(MessageComponent::reply(
                        message_id.to_string(),
                        selected_text,
                    ));
                }
            }
            WebChatMessagePart::Record { url } => {
                let url = url.trim();
                if !url.is_empty() {
                    message.push(MessageComponent::record(url.to_string()));
                }
            }
            WebChatMessagePart::Video { url } => {
                let url = url.trim();
                if !url.is_empty() {
                    message.push(MessageComponent::video(url.to_string()));
                }
            }
            WebChatMessagePart::File { name, url } => {
                let url = url.trim();
                if !url.is_empty() {
                    message.push(MessageComponent::file(name, url.to_string()));
                }
            }
        }
    }

    for image_url in image_urls {
        let image_url = image_url.trim();
        if !image_url.is_empty() {
            message.push(MessageComponent::image(image_url.to_string()));
        }
    }

    message
}

pub(crate) fn webchat_message_response_from_chain(chain: &MessageChain) -> WebChatMessageResponse {
    WebChatMessageResponse {
        text: chain.plain_text(),
        image_urls: chain.image_urls(),
        message_parts: message_parts_from_chain(chain),
    }
}

fn message_parts_from_chain(chain: &MessageChain) -> Vec<WebChatMessagePart> {
    chain
        .components()
        .iter()
        .filter_map(|component| match component {
            MessageComponent::Plain { text } if !text.trim().is_empty() => {
                Some(WebChatMessagePart::Plain { text: text.clone() })
            }
            MessageComponent::Image { url } if !url.trim().is_empty() => {
                Some(WebChatMessagePart::Image {
                    url: url.trim().to_string(),
                })
            }
            MessageComponent::Reply {
                message_id,
                selected_text,
                ..
            } if !message_id.trim().is_empty() => Some(WebChatMessagePart::Reply {
                message_id: message_id.trim().to_string(),
                selected_text: selected_text.clone(),
            }),
            MessageComponent::Record { url } if !url.trim().is_empty() => {
                Some(WebChatMessagePart::Record {
                    url: url.trim().to_string(),
                })
            }
            MessageComponent::Video { url } if !url.trim().is_empty() => {
                Some(WebChatMessagePart::Video {
                    url: url.trim().to_string(),
                })
            }
            MessageComponent::File { name, url } if !url.trim().is_empty() => {
                Some(WebChatMessagePart::File {
                    name: name.clone(),
                    url: url.trim().to_string(),
                })
            }
            _ => None,
        })
        .collect()
}
