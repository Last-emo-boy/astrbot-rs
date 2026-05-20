//! 企业微信应用消息出站。
//!
//! 端点：`https://qyapi.weixin.qq.com/cgi-bin/message/send?access_token={TOKEN}`
//!
//! 请求体形如：
//! ```json
//! {
//!   "touser": "u1|u2",     // 或 "toparty"/"totag"
//!   "msgtype": "text",
//!   "agentid": 1000002,
//!   "text": { "content": "..." }
//! }
//! ```
//!
//! WeCom 与公众号客服消息长得很像，区别：
//! - WeCom 多一个 `agentid`（应用 ID）。
//! - 收件人是 `touser` 但允许 `|` 分隔的多个用户。
//! - 媒体类型有 image/voice/video/file/textcard/news/markdown 等。
//!   我们 V1 仅实现 text/image/voice/file（与 Wave1 通用通道一致）。

use astrbot_core::{MessageChain, MessageComponent, MessageSession};
use serde_json::{Value, json};

use crate::{PlatformApiMethod, PlatformApiRequest};

#[derive(Clone, Debug)]
pub struct WeComOutboundClient {
    access_token: String,
    agent_id: i64,
    api_base_url: String,
    platform_id: String,
}

impl WeComOutboundClient {
    pub fn new(
        access_token: impl Into<String>,
        agent_id: i64,
        api_base_url: impl Into<String>,
        platform_id: impl Into<String>,
    ) -> Self {
        Self {
            access_token: access_token.into(),
            agent_id,
            api_base_url: api_base_url.into().trim().trim_end_matches('/').to_string(),
            platform_id: platform_id.into(),
        }
    }

    pub fn endpoint(&self) -> String {
        format!(
            "{}/cgi-bin/message/send?access_token={}",
            self.api_base_url, self.access_token
        )
    }

    /// AstrBot session.conversation_id → WeCom `touser`. We do not split
    /// multi-recipient sessions in V1.
    pub fn touser_from_session(session: &MessageSession) -> &str {
        let conv = session.conversation_id.as_str();
        conv.strip_prefix("private:")
            .or_else(|| conv.strip_prefix("group:"))
            .unwrap_or(conv)
    }

    pub fn requests_for_chain(
        &self,
        session: &MessageSession,
        chain: &MessageChain,
    ) -> Vec<PlatformApiRequest> {
        let touser = Self::touser_from_session(session).to_string();
        let mut requests = Vec::new();
        let mut pending_text = String::new();

        for component in chain.components() {
            match component {
                MessageComponent::Plain { text } if !text.trim().is_empty() => {
                    if !pending_text.is_empty() {
                        pending_text.push('\n');
                    }
                    pending_text.push_str(text);
                }
                MessageComponent::Image { url } if !url.trim().is_empty() => {
                    requests.push(self.send_media(&touser, "image", url));
                }
                MessageComponent::Record { url } if !url.trim().is_empty() => {
                    requests.push(self.send_media(&touser, "voice", url));
                }
                MessageComponent::File { name: _, url } if !url.trim().is_empty() => {
                    requests.push(self.send_media(&touser, "file", url));
                }
                MessageComponent::Video { url } if !url.trim().is_empty() => {
                    requests.push(self.send_media(&touser, "video", url));
                }
                _ => {}
            }
        }

        if !pending_text.is_empty() {
            requests.insert(0, self.send_text(&touser, &pending_text));
        }
        requests
    }

    fn rate_limit_key(&self, msgtype: &str) -> String {
        format!("wecom:{}:{msgtype}", self.platform_id)
    }

    fn post(&self, msgtype: &str, mut body: Value) -> PlatformApiRequest {
        // agentid 是 WeCom 强制字段。
        if let Some(obj) = body.as_object_mut() {
            obj.insert("agentid".into(), Value::from(self.agent_id));
        }
        PlatformApiRequest::new(
            "wecom".to_string(),
            PlatformApiMethod::Post,
            self.endpoint(),
        )
        .with_header("content-type", "application/json")
        .with_body(body.to_string().into_bytes())
        .with_rate_limit_key(self.rate_limit_key(msgtype))
    }

    fn send_text(&self, touser: &str, text: &str) -> PlatformApiRequest {
        self.post(
            "text",
            json!({
                "touser": touser,
                "msgtype": "text",
                "text": { "content": text },
            }),
        )
    }

    fn send_media(&self, touser: &str, msgtype: &str, media_id: &str) -> PlatformApiRequest {
        self.post(
            msgtype,
            json!({
                "touser": touser,
                "msgtype": msgtype,
                msgtype: { "media_id": media_id },
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> WeComOutboundClient {
        WeComOutboundClient::new("tok", 1000002, "https://qyapi.weixin.qq.com/", "wc1")
    }

    fn body_as_value(req: &PlatformApiRequest) -> Value {
        serde_json::from_slice(&req.body).unwrap()
    }

    #[test]
    fn endpoint_includes_token_query() {
        assert_eq!(
            client().endpoint(),
            "https://qyapi.weixin.qq.com/cgi-bin/message/send?access_token=tok"
        );
    }

    #[test]
    fn text_message_carries_agent_id() {
        let session = MessageSession::new("wc1", "private:alice");
        let chain = MessageChain::new(vec![MessageComponent::plain("hi")]);
        let req = &client().requests_for_chain(&session, &chain)[0];
        let body = body_as_value(req);
        assert_eq!(body["agentid"], 1000002);
        assert_eq!(body["touser"], "alice");
        assert_eq!(body["text"]["content"], "hi");
    }

    #[test]
    fn media_message_uses_media_id_object() {
        let session = MessageSession::new("wc1", "private:alice");
        let chain = MessageChain::new(vec![MessageComponent::image("img-id")]);
        let body = body_as_value(&client().requests_for_chain(&session, &chain)[0]);
        assert_eq!(body["msgtype"], "image");
        assert_eq!(body["image"]["media_id"], "img-id");
    }

    #[test]
    fn mixed_chain_emits_text_then_media() {
        let session = MessageSession::new("wc1", "private:alice");
        let chain = MessageChain::new(vec![
            MessageComponent::plain("part1"),
            MessageComponent::image("img"),
            MessageComponent::file("doc.pdf", "file-id"),
            MessageComponent::plain("part2"),
        ]);
        let requests = client().requests_for_chain(&session, &chain);
        assert_eq!(requests.len(), 3);
        assert_eq!(
            body_as_value(&requests[0])["text"]["content"],
            "part1\npart2"
        );
        assert_eq!(body_as_value(&requests[1])["msgtype"], "image");
        assert_eq!(body_as_value(&requests[2])["msgtype"], "file");
    }

    #[test]
    fn rate_limit_key_keyed_by_platform_msgtype() {
        let session = MessageSession::new("wc1", "private:alice");
        let chain = MessageChain::new(vec![MessageComponent::plain("x")]);
        assert_eq!(
            client().requests_for_chain(&session, &chain)[0]
                .rate_limit_key
                .as_deref(),
            Some("wecom:wc1:text")
        );
    }
}
