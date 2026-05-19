//! 微信公众号客服消息出站。
//!
//! 公众号 ≠ 个人微信。这里只对接 **公众号客服消息接口**：
//! `https://api.weixin.qq.com/cgi-bin/message/custom/send?access_token={TOKEN}`
//!
//! 与 Telegram 不同之处：
//! - access_token 走 query string，而不是 path。
//! - body 是 WeChat 自家的 `{touser, msgtype, text|image|voice|...}` 嵌套对象。
//! - access_token 由 `/cgi-bin/token?grant_type=client_credential&appid=&secret=`
//!   自取。本 module 假定 token 已经被 ProviderOrchestrator 在配置层刷新好；
//!   只负责把单条 token 拼到 URL 里。

use astrbot_core::{MessageChain, MessageComponent, MessageSession};
use serde_json::{Value, json};

use crate::{PlatformApiMethod, PlatformApiRequest};

/// 直连客服消息构造器。
#[derive(Clone, Debug)]
pub struct WechatOfficialOutboundClient {
    access_token: String,
    api_base_url: String,
    platform_id: String,
}

impl WechatOfficialOutboundClient {
    pub fn new(
        access_token: impl Into<String>,
        api_base_url: impl Into<String>,
        platform_id: impl Into<String>,
    ) -> Self {
        Self {
            access_token: access_token.into(),
            api_base_url: api_base_url
                .into()
                .trim()
                .trim_end_matches('/')
                .to_string(),
            platform_id: platform_id.into(),
        }
    }

    /// `https://api.weixin.qq.com/cgi-bin/message/custom/send?access_token=...`
    pub fn endpoint(&self) -> String {
        format!(
            "{}/cgi-bin/message/custom/send?access_token={}",
            self.api_base_url, self.access_token
        )
    }

    /// AstrBot 会话 → 微信 OpenID。会话 conversation_id 形如
    /// `private:<openid>` 或 `group:<openid>`（公众号没有真群组，但保留兼容）。
    pub fn openid_from_session(session: &MessageSession) -> &str {
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
        let openid = Self::openid_from_session(session).to_string();
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
                    // 客服消息走 media_id；我们把 url 直接当作 media_id 透传，
                    // 上层若需要走 /cgi-bin/media/upload 提前换 id，再回填。
                    requests.push(self.send_media(&openid, "image", "media_id", url));
                }
                MessageComponent::Record { url } if !url.trim().is_empty() => {
                    requests.push(self.send_media(&openid, "voice", "media_id", url));
                }
                MessageComponent::File { name: _, url } if !url.trim().is_empty() => {
                    // 公众号客服消息没有独立 file 类型；用 image 兜底，
                    // fallback_from 字段保留语义供上层换格式。
                    requests.push(self.send_media(&openid, "image", "media_id", url));
                }
                _ => {}
            }
        }

        if !pending_text.is_empty() {
            requests.insert(0, self.send_text(&openid, &pending_text));
        }
        requests
    }

    fn rate_limit_key(&self, msgtype: &str) -> String {
        format!("weixin_official_account:{}:{msgtype}", self.platform_id)
    }

    fn post(&self, msgtype: &str, body: Value) -> PlatformApiRequest {
        PlatformApiRequest::new(
            "weixin_official_account".to_string(),
            PlatformApiMethod::Post,
            self.endpoint(),
        )
        .with_header("content-type", "application/json")
        .with_body(body.to_string().into_bytes())
        .with_rate_limit_key(self.rate_limit_key(msgtype))
    }

    fn send_text(&self, openid: &str, text: &str) -> PlatformApiRequest {
        self.post(
            "text",
            json!({
                "touser": openid,
                "msgtype": "text",
                "text": { "content": text },
            }),
        )
    }

    fn send_media(
        &self,
        openid: &str,
        msgtype: &str,
        media_field: &str,
        media_id_or_url: &str,
    ) -> PlatformApiRequest {
        let mut media = serde_json::Map::new();
        media.insert(
            media_field.to_string(),
            Value::String(media_id_or_url.to_string()),
        );
        self.post(
            msgtype,
            json!({
                "touser": openid,
                "msgtype": msgtype,
                msgtype: media,
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> WechatOfficialOutboundClient {
        WechatOfficialOutboundClient::new("tok", "https://api.weixin.qq.com/", "wx1")
    }

    fn body_as_value(req: &PlatformApiRequest) -> Value {
        serde_json::from_slice(&req.body).unwrap()
    }

    #[test]
    fn endpoint_includes_token_query() {
        assert_eq!(
            client().endpoint(),
            "https://api.weixin.qq.com/cgi-bin/message/custom/send?access_token=tok"
        );
    }

    #[test]
    fn openid_strips_private_prefix() {
        let session = MessageSession::new("wx1", "private:oXyZ");
        assert_eq!(
            WechatOfficialOutboundClient::openid_from_session(&session),
            "oXyZ"
        );
    }

    #[test]
    fn plain_text_chain_emits_text_message() {
        let session = MessageSession::new("wx1", "private:oXyZ");
        let chain = MessageChain::new(vec![MessageComponent::plain("hello")]);
        let requests = client().requests_for_chain(&session, &chain);
        assert_eq!(requests.len(), 1);
        let body = body_as_value(&requests[0]);
        assert_eq!(body["touser"], "oXyZ");
        assert_eq!(body["msgtype"], "text");
        assert_eq!(body["text"]["content"], "hello");
    }

    #[test]
    fn image_chain_emits_image_with_media_id() {
        let session = MessageSession::new("wx1", "private:oXyZ");
        let chain =
            MessageChain::new(vec![MessageComponent::image("media-abc-123")]);
        let requests = client().requests_for_chain(&session, &chain);
        assert_eq!(requests.len(), 1);
        let body = body_as_value(&requests[0]);
        assert_eq!(body["msgtype"], "image");
        assert_eq!(body["image"]["media_id"], "media-abc-123");
    }

    #[test]
    fn mixed_chain_collapses_text_then_media() {
        let session = MessageSession::new("wx1", "private:oXyZ");
        let chain = MessageChain::new(vec![
            MessageComponent::plain("part one"),
            MessageComponent::image("img-id"),
            MessageComponent::plain("part two"),
            MessageComponent::record("voice-id"),
        ]);
        let requests = client().requests_for_chain(&session, &chain);
        assert_eq!(requests.len(), 3);
        let text_body = body_as_value(&requests[0]);
        assert_eq!(text_body["text"]["content"], "part one\npart two");
        assert_eq!(body_as_value(&requests[1])["msgtype"], "image");
        assert_eq!(body_as_value(&requests[2])["msgtype"], "voice");
    }

    #[test]
    fn rate_limit_key_includes_platform_and_msgtype() {
        let session = MessageSession::new("wx1", "private:oXyZ");
        let chain = MessageChain::new(vec![MessageComponent::plain("x")]);
        let requests = client().requests_for_chain(&session, &chain);
        assert_eq!(
            requests[0].rate_limit_key.as_deref(),
            Some("weixin_official_account:wx1:text")
        );
    }
}
