use astrbot_core::{AstrbotError, MessageChain, MessageComponent, MessageSession, Result};
use serde_json::{Value, json};

pub(super) fn plain_text_message(text: impl Into<String>) -> MessageChain {
    MessageChain::plain(text)
}

pub(super) fn parse_onebot_message_chain(value: &Value) -> MessageChain {
    if let Some(segments) = value.as_array() {
        let mut components = Vec::new();
        for segment in segments {
            let Some(segment_type) = segment.get("type").and_then(Value::as_str) else {
                continue;
            };
            let data = segment.get("data").unwrap_or(&Value::Null);
            match segment_type {
                "text" | "plain" => {
                    if let Some(text) = data.get("text").and_then(value_as_non_empty_string) {
                        components.push(MessageComponent::plain(text));
                    }
                }
                "at" => {
                    if data.get("qq").and_then(Value::as_str) == Some("all") {
                        components.push(MessageComponent::mention_all());
                    } else if let Some(user_id) = data.get("qq").and_then(value_as_non_empty_string)
                    {
                        components.push(MessageComponent::mention(user_id));
                    }
                }
                "reply" => {
                    if let Some(message_id) = data.get("id").and_then(value_as_non_empty_string) {
                        components.push(MessageComponent::reply(message_id, ""));
                    }
                }
                "image" => {
                    if let Some(url) = onebot_media_url(data) {
                        components.push(MessageComponent::image(url));
                    }
                }
                "record" => {
                    if let Some(url) = onebot_media_url(data) {
                        components.push(MessageComponent::record(url));
                    }
                }
                "file" => {
                    if let Some(url) = onebot_media_url(data) {
                        let name = data
                            .get("name")
                            .or_else(|| data.get("file_name"))
                            .or_else(|| data.get("file"))
                            .and_then(value_as_non_empty_string)
                            .unwrap_or_else(|| "file".to_string());
                        components.push(MessageComponent::file(name, url));
                    }
                }
                _ => {}
            }
        }
        return MessageChain::new(components);
    }

    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(MessageChain::plain)
        .unwrap_or_default()
}

pub(super) fn onebot_send_actions(
    session: &MessageSession,
    chain: MessageChain,
) -> Result<Vec<Value>> {
    let mut message_segments = Vec::new();
    let mut file_actions = Vec::new();
    for component in chain.components() {
        match component {
            MessageComponent::Plain { text } => {
                if !text.trim().is_empty() {
                    message_segments.push(json!({"type": "text", "data": {"text": text}}));
                }
            }
            MessageComponent::Image { url } => {
                if !url.trim().is_empty() {
                    message_segments.push(json!({"type": "image", "data": {"file": url}}));
                }
            }
            MessageComponent::Record { url } => {
                if !url.trim().is_empty() {
                    message_segments.push(json!({"type": "record", "data": {"file": url}}));
                }
            }
            MessageComponent::File { name, url } => {
                if !url.trim().is_empty() {
                    file_actions.push(file_action(session, name, url)?);
                }
            }
            MessageComponent::Mention { user_id } => {
                if !user_id.trim().is_empty() {
                    message_segments.push(json!({"type": "at", "data": {"qq": user_id}}));
                }
            }
            MessageComponent::MentionAll => {
                message_segments.push(json!({"type": "at", "data": {"qq": "all"}}));
            }
            MessageComponent::Reply { message_id, .. } => {
                if !message_id.trim().is_empty() {
                    message_segments.push(json!({"type": "reply", "data": {"id": message_id}}));
                }
            }
            MessageComponent::Video { .. } => {
                return Err(AstrbotError::Platform(
                    "onebot outbound video messages are not supported yet".to_string(),
                ));
            }
        }
    }

    let mut actions = Vec::new();
    if !message_segments.is_empty() {
        actions.push(message_action(session, message_segments)?);
    }
    actions.extend(file_actions);
    Ok(actions)
}

fn message_action(session: &MessageSession, message: Vec<Value>) -> Result<Value> {
    if session.is_group() {
        let group_id = numeric_conversation_id(&session.conversation_id, "group:")?;
        Ok(json!({
            "action": "send_group_msg",
            "params": {
                "group_id": group_id,
                "message": message
            }
        }))
    } else {
        let user_id = numeric_conversation_id(&session.conversation_id, "private:")?;
        Ok(json!({
            "action": "send_private_msg",
            "params": {
                "user_id": user_id,
                "message": message
            }
        }))
    }
}

fn file_action(session: &MessageSession, name: &str, url: &str) -> Result<Value> {
    if session.is_group() {
        let group_id = numeric_conversation_id(&session.conversation_id, "group:")?;
        Ok(json!({
            "action": "upload_group_file",
            "params": {
                "group_id": group_id,
                "file": url,
                "name": name
            }
        }))
    } else {
        let user_id = numeric_conversation_id(&session.conversation_id, "private:")?;
        Ok(json!({
            "action": "upload_private_file",
            "params": {
                "user_id": user_id,
                "file": url,
                "name": name
            }
        }))
    }
}

fn numeric_conversation_id(conversation_id: &str, prefix: &str) -> Result<u64> {
    let value = conversation_id
        .strip_prefix(prefix)
        .unwrap_or(conversation_id);
    value.parse::<u64>().map_err(|_| {
        AstrbotError::Platform(format!(
            "onebot outbound session id must be numeric, got {conversation_id}"
        ))
    })
}

fn onebot_media_url(data: &Value) -> Option<String> {
    data.get("url")
        .or_else(|| data.get("file"))
        .and_then(value_as_non_empty_string)
}

fn value_as_non_empty_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        }
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use astrbot_core::{MessageChain, MessageComponent, MessageSession};
    use serde_json::json;

    use super::{onebot_send_actions, parse_onebot_message_chain};

    #[test]
    fn parses_onebot_message_segments() {
        let chain = parse_onebot_message_chain(&json!([
            {"type": "text", "data": {"text": "hello"}},
            {"type": "at", "data": {"qq": "1001"}},
            {"type": "image", "data": {"url": "https://example.test/a.png"}},
            {"type": "record", "data": {"file": "voice.amr"}},
            {"type": "file", "data": {"file": "file:///tmp/a.txt", "name": "a.txt"}}
        ]));

        assert_eq!(chain.plain_text(), "hello");
        assert!(matches!(
            &chain.components()[1],
            MessageComponent::Mention { user_id } if user_id == "1001"
        ));
        assert_eq!(chain.image_urls(), vec!["https://example.test/a.png"]);
        assert!(matches!(
            &chain.components()[3],
            MessageComponent::Record { url } if url == "voice.amr"
        ));
        assert!(matches!(
            &chain.components()[4],
            MessageComponent::File { name, url } if name == "a.txt" && url == "file:///tmp/a.txt"
        ));
    }

    #[test]
    fn serializes_onebot_outbound_actions() {
        let actions = onebot_send_actions(
            &MessageSession::group("onebot", "group:2001"),
            MessageChain::new(vec![
                MessageComponent::plain("hello"),
                MessageComponent::image("https://example.test/a.png"),
                MessageComponent::file("a.txt", "file:///tmp/a.txt"),
            ]),
        )
        .expect("actions should serialize");

        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0]["action"], "send_group_msg");
        assert_eq!(actions[0]["params"]["group_id"], 2001);
        assert_eq!(actions[1]["action"], "upload_group_file");
    }
}
