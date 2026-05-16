use astrbot_core::{
    ForwardMessageNode, ForwardMessageReference, QuotedImageReference, QuotedMessage,
};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OneBotForwardParser {
    pub max_forward_node_depth: usize,
}

impl OneBotForwardParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_forward_node_depth(mut self, max_forward_node_depth: usize) -> Self {
        self.max_forward_node_depth = max_forward_node_depth.max(1);
        self
    }

    pub fn parse_get_msg_payload(&self, payload: &Value) -> QuotedMessage {
        let data = unwrap_onebot_data(payload);
        let segments = data
            .get("message")
            .or_else(|| data.get("messages"))
            .and_then(Value::as_array);
        if let Some(segments) = segments {
            return self.parse_segments(segments);
        }

        data.get("message")
            .or_else(|| data.get("messages"))
            .or_else(|| data.get("raw_message"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(|text| QuotedMessage::new().with_text(text))
            .unwrap_or_default()
    }

    pub fn parse_get_forward_payload(&self, payload: &Value) -> OneBotForwardParseResult {
        let data = unwrap_onebot_data(payload);
        let nodes = data
            .get("messages")
            .or_else(|| data.get("message"))
            .or_else(|| data.get("nodes"))
            .or_else(|| data.get("nodeList"))
            .and_then(Value::as_array);

        nodes
            .map(|nodes| self.parse_forward_nodes(nodes, 0).into_result())
            .unwrap_or_default()
    }

    pub fn parse_segments(&self, segments: &[Value]) -> QuotedMessage {
        self.parse_segments_inner(segments, 0).into_quote()
    }

    fn parse_segments_inner(&self, segments: &[Value], depth: usize) -> ParsedQuoteParts {
        let mut parsed = ParsedQuoteParts::default();
        if depth > self.max_forward_node_depth {
            return parsed;
        }

        for segment in segments {
            let Some(segment_type) = segment.get("type").and_then(Value::as_str) else {
                continue;
            };
            let data = segment.get("data").unwrap_or(&Value::Null);

            match segment_type {
                "text" | "plain" => {
                    parsed.push_text_value(data.get("text"));
                }
                "at" => {
                    let mention = data
                        .get("name")
                        .or_else(|| data.get("qq"))
                        .and_then(value_as_non_empty_string);
                    if let Some(mention) = mention {
                        parsed.push_text(format!("@{mention}"));
                    }
                }
                "image" => {
                    parsed.push_text("[Image]");
                    if let Some(image_ref) = image_ref_from_data(data) {
                        parsed.push_image_ref(image_ref);
                    }
                }
                "video" => parsed.push_text("[Video]"),
                "file" => {
                    let file_name = data
                        .get("name")
                        .or_else(|| data.get("file_name"))
                        .or_else(|| data.get("file"))
                        .and_then(value_as_non_empty_string)
                        .unwrap_or_else(|| "file".to_string());
                    parsed.push_text(format!("[File:{file_name}]"));
                    if let Some(image_ref) = file_image_ref_from_data(data) {
                        parsed.push_image_ref(image_ref);
                    }
                }
                "forward" | "forward_msg" => {
                    if let Some(forward_id) = data
                        .get("id")
                        .or_else(|| data.get("message_id"))
                        .and_then(value_as_non_empty_string)
                    {
                        parsed.push_forward_ref(ForwardMessageReference::new(forward_id));
                    } else if let Some(content) = data.get("content").and_then(Value::as_array) {
                        parsed.merge(self.parse_forward_nodes(content, depth + 1));
                    }
                }
                "node" | "nodes" => {
                    if let Some(content) = data.get("content").and_then(Value::as_array) {
                        parsed.merge(self.parse_forward_nodes(content, depth + 1));
                    }
                }
                "json" => {
                    if let Some(raw_json) = data.get("data").and_then(Value::as_str)
                        && let Some(text) = extract_multimsg_text(&raw_json.replace("&#44;", ","))
                    {
                        parsed.push_text(text);
                    }
                }
                _ => {}
            }
        }

        parsed
    }

    fn parse_forward_nodes(&self, nodes: &[Value], depth: usize) -> ParsedQuoteParts {
        let mut parsed = ParsedQuoteParts::default();
        if depth > self.max_forward_node_depth {
            return parsed;
        }

        for node in nodes {
            let sender = node.get("sender").unwrap_or(&Value::Null);
            let sender_name = sender
                .get("nickname")
                .or_else(|| sender.get("card"))
                .or_else(|| sender.get("user_id"))
                .and_then(value_as_non_empty_string)
                .unwrap_or_else(|| "Unknown User".to_string());
            let sender_id = sender.get("user_id").and_then(value_as_non_empty_string);

            let raw_content = node
                .get("message")
                .or_else(|| node.get("content"))
                .unwrap_or(&Value::Null);
            let mut node_parts = match raw_content {
                Value::Array(segments) => self.parse_segments_inner(segments, depth + 1),
                Value::String(text) => parse_string_content(text)
                    .map(|segments| self.parse_segments_inner(&segments, depth + 1))
                    .unwrap_or_else(|| ParsedQuoteParts::text(text.trim())),
                _ => ParsedQuoteParts::default(),
            };

            let node_quote = node_parts
                .clone()
                .into_quote()
                .with_sender_name(sender_name.clone());
            let node_quote = if let Some(sender_id) = sender_id.clone() {
                node_quote.with_sender_id(sender_id)
            } else {
                node_quote
            };

            if node_quote.has_content() {
                let mut forward_node = ForwardMessageNode::new(node_quote.clone())
                    .with_sender_name(sender_name.clone());
                if let Some(sender_id) = sender_id {
                    forward_node = forward_node.with_sender_id(sender_id);
                }
                parsed.nodes.push(forward_node);
            }

            if let Some(text) = node_parts.text.take() {
                parsed.push_text(format!("{sender_name}: {text}"));
            }
            parsed.merge(node_parts);
        }

        parsed
    }
}

impl Default for OneBotForwardParser {
    fn default() -> Self {
        Self {
            max_forward_node_depth: 8,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OneBotForwardParseResult {
    pub quote: QuotedMessage,
    pub nodes: Vec<ForwardMessageNode>,
}

impl OneBotForwardParseResult {
    pub fn has_content(&self) -> bool {
        self.quote.has_content() || !self.nodes.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ParsedQuoteParts {
    text: Option<String>,
    image_refs: Vec<QuotedImageReference>,
    forward_refs: Vec<ForwardMessageReference>,
    nodes: Vec<ForwardMessageNode>,
}

impl ParsedQuoteParts {
    fn text(text: impl Into<String>) -> Self {
        let mut parts = Self::default();
        parts.push_text(text);
        parts
    }

    fn push_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.text = Some(match self.text.take() {
            Some(existing) => format!("{existing}{text}"),
            None => text.to_string(),
        });
    }

    fn push_text_value(&mut self, value: Option<&Value>) {
        if let Some(text) = value.and_then(Value::as_str) {
            self.push_text(text);
        }
    }

    fn push_image_ref(&mut self, image_ref: QuotedImageReference) {
        if !image_ref.is_empty() && !self.image_refs.contains(&image_ref) {
            self.image_refs.push(image_ref);
        }
    }

    fn push_forward_ref(&mut self, forward_ref: ForwardMessageReference) {
        if !forward_ref.forward_id.trim().is_empty() && !self.forward_refs.contains(&forward_ref) {
            self.forward_refs.push(forward_ref);
        }
    }

    fn merge(&mut self, other: ParsedQuoteParts) {
        if let Some(text) = other.text {
            self.push_text(text);
        }
        for image_ref in other.image_refs {
            self.push_image_ref(image_ref);
        }
        for forward_ref in other.forward_refs {
            self.push_forward_ref(forward_ref);
        }
        for node in other.nodes {
            if !self.nodes.contains(&node) {
                self.nodes.push(node);
            }
        }
    }

    fn into_quote(self) -> QuotedMessage {
        let mut quote = QuotedMessage::new();
        if let Some(text) = self.text {
            quote = quote.with_text(text);
        }
        for image_ref in self.image_refs {
            quote.push_image_ref(image_ref);
        }
        for forward_ref in self.forward_refs {
            quote.push_forward_ref(forward_ref);
        }
        quote
    }

    fn into_result(self) -> OneBotForwardParseResult {
        OneBotForwardParseResult {
            quote: self.clone().into_quote(),
            nodes: self.nodes,
        }
    }
}

fn unwrap_onebot_data(payload: &Value) -> &Value {
    payload
        .get("data")
        .filter(|data| data.is_object())
        .unwrap_or(payload)
}

fn value_as_non_empty_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn image_ref_from_data(data: &Value) -> Option<QuotedImageReference> {
    data.get("url")
        .and_then(value_as_non_empty_string)
        .map(QuotedImageReference::url)
        .or_else(|| {
            data.get("file")
                .and_then(value_as_non_empty_string)
                .map(QuotedImageReference::file)
        })
}

fn file_image_ref_from_data(data: &Value) -> Option<QuotedImageReference> {
    let url = data.get("url").and_then(value_as_non_empty_string);
    if let Some(url) = url.filter(|url| looks_like_image_file_name(url)) {
        return Some(QuotedImageReference::url(url));
    }

    let file = data.get("file").and_then(value_as_non_empty_string)?;
    let name = data
        .get("name")
        .or_else(|| data.get("file_name"))
        .and_then(value_as_non_empty_string)
        .unwrap_or_else(|| file.clone());
    looks_like_image_file_name(&name).then(|| QuotedImageReference::file(file))
}

fn looks_like_image_file_name(value: &str) -> bool {
    let lower = value
        .split(['?', '#'])
        .next()
        .unwrap_or(value)
        .to_ascii_lowercase();
    matches!(
        lower.rsplit('.').next(),
        Some("jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp")
    )
}

fn parse_string_content(text: &str) -> Option<Vec<Value>> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str::<Vec<Value>>(trimmed).ok()
}

fn extract_multimsg_text(raw_json: &str) -> Option<String> {
    let parsed = serde_json::from_str::<Value>(raw_json).ok()?;
    (parsed.get("app").and_then(Value::as_str) == Some("com.tencent.multimsg")).then_some(())?;
    (parsed
        .get("config")
        .and_then(|config| config.get("forward"))
        .and_then(Value::as_i64)
        == Some(1))
    .then_some(())?;

    let news_items = parsed
        .get("meta")
        .and_then(|meta| meta.get("detail"))
        .and_then(|detail| detail.get("news"))
        .and_then(Value::as_array)?;

    let texts = news_items
        .iter()
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .map(|text| text.trim().replace("[图片]", "").trim().to_string())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();

    (!texts.is_empty()).then(|| texts.join("\n"))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::OneBotForwardParser;

    #[test]
    fn get_msg_payload_extracts_text_images_and_forward_refs() {
        let payload = json!({
            "data": {
                "message": [
                    {"type": "text", "data": {"text": "hello "}},
                    {"type": "image", "data": {"url": "https://example.test/a.png"}},
                    {"type": "forward", "data": {"id": "forward-1"}},
                    {"type": "json", "data": {"data": r#"{"app":"com.tencent.multimsg","config":{"forward":1},"meta":{"detail":{"news":[{"text":"preview [图片]"}]}}}"#}}
                ]
            }
        });

        let quote = OneBotForwardParser::new().parse_get_msg_payload(&payload);

        assert_eq!(quote.text.as_deref(), Some("hello[Image]preview"));
        assert_eq!(
            quote.image_ref_values(),
            vec!["https://example.test/a.png".to_string()]
        );
        assert_eq!(quote.forward_refs()[0].forward_id, "forward-1");
    }

    #[test]
    fn forward_payload_extracts_nested_node_text_and_images() {
        let payload = json!({
            "data": {
                "messages": [
                    {
                        "sender": {"nickname": "Alice", "user_id": 1001},
                        "message": [
                            {"type": "text", "data": {"text": "hi "}},
                            {"type": "image", "data": {"file": "nested.jpg"}}
                        ]
                    },
                    {
                        "sender": {"nickname": "Bob"},
                        "content": r#"[{"type":"text","data":{"text":"file "}},{"type":"file","data":{"name":"cover.png","url":"https://example.test/cover.png"}}]"#
                    }
                ]
            }
        });

        let result = OneBotForwardParser::new().parse_get_forward_payload(&payload);

        assert!(result.has_content());
        assert_eq!(result.nodes.len(), 2);
        assert_eq!(
            result.quote.text.as_deref(),
            Some("Alice: hi[Image]Bob: file[File:cover.png]")
        );
        assert_eq!(
            result.quote.image_ref_values(),
            vec![
                "nested.jpg".to_string(),
                "https://example.test/cover.png".to_string()
            ]
        );
    }

    #[test]
    fn get_msg_payload_uses_raw_message_when_segments_are_absent() {
        let payload = json!({"data": {"raw_message": "plain fallback"}});

        let quote = OneBotForwardParser::new().parse_get_msg_payload(&payload);

        assert_eq!(quote.text.as_deref(), Some("plain fallback"));
        assert!(quote.image_refs().is_empty());
    }
}
