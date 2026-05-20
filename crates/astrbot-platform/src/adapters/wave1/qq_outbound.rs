//! QQ 官方机器人（QQ Open Bot / 频道）出站客户端。
//!
//! Tencent 把 QQ 官方机器人和 QQ 频道（Guild）合在同一套 OpenAPI 上：
//!
//! - 私信:           POST {api}/v2/users/{openid}/messages
//! - 群消息:         POST {api}/v2/groups/{group_openid}/messages
//! - 频道子频道消息: POST {api}/channels/{channel_id}/messages
//! - 频道私信:       POST {api}/dms/{guild_id}/messages
//!
//! 身份认证用 Bearer token（QQBot AppID + AppSecret 换的 access_token；
//! 与本 client 解耦——上层注入即可）。
//!
//! 内容字段：text、image_url、msg_type、event_id（机器人主动回复必带）。

use astrbot_core::{MessageChain, MessageComponent, MessageSession};
use serde_json::{Value, json};

use crate::{PlatformApiMethod, PlatformApiRequest};

/// 通道类型：私信 / 群 / 频道子频道 / 频道私信。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QqChannel {
    /// `/v2/users/{openid}/messages`
    UserDirect,
    /// `/v2/groups/{group_openid}/messages`
    Group,
    /// `/channels/{channel_id}/messages` — 频道子频道。
    GuildChannel,
    /// `/dms/{guild_id}/messages` — 频道私聊。
    GuildDirect,
}

#[derive(Clone, Debug)]
pub struct QqOfficialOutboundClient {
    access_token: String,
    api_base_url: String,
    platform_id: String,
    /// 是否是 Webhook（v2 OpenAPI）模式。`true` → 用 `/v2/users` 和
    /// `/v2/groups`；`false` → 频道 `/channels` 和 `/dms`。两种共用同一
    /// access_token 域。
    is_v2: bool,
}

impl QqOfficialOutboundClient {
    /// Webhook 模式（QQ 官方 v2 OpenAPI）。
    pub fn webhook(
        access_token: impl Into<String>,
        api_base_url: impl Into<String>,
        platform_id: impl Into<String>,
    ) -> Self {
        Self::new(access_token, api_base_url, platform_id, true)
    }

    /// 频道（QQ Guild）模式。
    pub fn guild(
        access_token: impl Into<String>,
        api_base_url: impl Into<String>,
        platform_id: impl Into<String>,
    ) -> Self {
        Self::new(access_token, api_base_url, platform_id, false)
    }

    fn new(
        access_token: impl Into<String>,
        api_base_url: impl Into<String>,
        platform_id: impl Into<String>,
        is_v2: bool,
    ) -> Self {
        Self {
            access_token: access_token.into(),
            api_base_url: api_base_url.into().trim().trim_end_matches('/').to_string(),
            platform_id: platform_id.into(),
            is_v2,
        }
    }

    pub fn endpoint_for(&self, channel: QqChannel, target_id: &str) -> String {
        match channel {
            QqChannel::UserDirect => {
                format!("{}/v2/users/{target_id}/messages", self.api_base_url)
            }
            QqChannel::Group => {
                format!("{}/v2/groups/{target_id}/messages", self.api_base_url)
            }
            QqChannel::GuildChannel => {
                format!("{}/channels/{target_id}/messages", self.api_base_url)
            }
            QqChannel::GuildDirect => {
                format!("{}/dms/{target_id}/messages", self.api_base_url)
            }
        }
    }

    /// 推断通道：v2 直连优先看 `is_group()`；频道模式遵循配置。
    pub fn detect_channel(&self, session: &MessageSession) -> QqChannel {
        if self.is_v2 {
            if session.is_group() {
                QqChannel::Group
            } else {
                QqChannel::UserDirect
            }
        } else if session.is_group() {
            QqChannel::GuildChannel
        } else {
            QqChannel::GuildDirect
        }
    }

    pub fn target_id_from_session(session: &MessageSession) -> &str {
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
        let channel = self.detect_channel(session);
        let target = Self::target_id_from_session(session).to_string();
        let endpoint = self.endpoint_for(channel, &target);

        let mut requests = Vec::new();
        let mut pending_text = String::new();
        let mut reply_to: Option<String> = None;

        for component in chain.components() {
            match component {
                MessageComponent::Plain { text } if !text.trim().is_empty() => {
                    if !pending_text.is_empty() {
                        pending_text.push('\n');
                    }
                    pending_text.push_str(text);
                }
                MessageComponent::Reply { message_id, .. } if !message_id.trim().is_empty() => {
                    reply_to = Some(message_id.clone());
                }
                MessageComponent::Image { url } if !url.trim().is_empty() => {
                    requests.push(self.post_image(&endpoint, url, channel));
                }
                _ => {}
            }
        }

        if !pending_text.is_empty() {
            requests.insert(
                0,
                self.post_text(&endpoint, &pending_text, reply_to, channel),
            );
        }
        requests
    }

    fn rate_limit_key(&self, action: &str) -> String {
        format!("qq:{}:{action}", self.platform_id)
    }

    fn auth_header(&self) -> (String, String) {
        (
            "authorization".to_string(),
            format!("QQBot {}", self.access_token),
        )
    }

    fn post(&self, endpoint: &str, body: Value, action: &str) -> PlatformApiRequest {
        let (auth_name, auth_value) = self.auth_header();
        PlatformApiRequest::new(
            "qq_official".to_string(),
            PlatformApiMethod::Post,
            endpoint.to_string(),
        )
        .with_header("content-type", "application/json")
        .with_header(auth_name, auth_value)
        .with_body(body.to_string().into_bytes())
        .with_rate_limit_key(self.rate_limit_key(action))
    }

    fn post_text(
        &self,
        endpoint: &str,
        text: &str,
        reply_to: Option<String>,
        channel: QqChannel,
    ) -> PlatformApiRequest {
        let mut body = json!({ "content": text });
        // v2 用 msg_type=0 表示纯文本；频道接口用同样字段但字段名一致。
        body["msg_type"] = Value::from(0);
        if let Some(reply_to) = reply_to {
            body["msg_id"] = Value::String(reply_to);
        }
        let action = if matches!(channel, QqChannel::Group | QqChannel::GuildChannel) {
            "text_group"
        } else {
            "text_direct"
        };
        self.post(endpoint, body, action)
    }

    fn post_image(&self, endpoint: &str, url: &str, channel: QqChannel) -> PlatformApiRequest {
        let mut body = json!({ "image": url });
        body["msg_type"] = Value::from(7);
        let action = if matches!(channel, QqChannel::Group | QqChannel::GuildChannel) {
            "image_group"
        } else {
            "image_direct"
        };
        self.post(endpoint, body, action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body_as_value(req: &PlatformApiRequest) -> Value {
        serde_json::from_slice(&req.body).unwrap()
    }

    #[test]
    fn webhook_user_direct_endpoint() {
        let client = QqOfficialOutboundClient::webhook("tok", "https://api.sgroup.qq.com/", "qq1");
        assert_eq!(
            client.endpoint_for(QqChannel::UserDirect, "u-1"),
            "https://api.sgroup.qq.com/v2/users/u-1/messages"
        );
    }

    #[test]
    fn webhook_group_endpoint() {
        let client = QqOfficialOutboundClient::webhook("tok", "https://api.sgroup.qq.com/", "qq1");
        assert_eq!(
            client.endpoint_for(QqChannel::Group, "g-1"),
            "https://api.sgroup.qq.com/v2/groups/g-1/messages"
        );
    }

    #[test]
    fn guild_channel_endpoint() {
        let client = QqOfficialOutboundClient::guild("tok", "https://api.sgroup.qq.com/", "qq2");
        assert_eq!(
            client.endpoint_for(QqChannel::GuildChannel, "c-9001"),
            "https://api.sgroup.qq.com/channels/c-9001/messages"
        );
    }

    #[test]
    fn webhook_text_carries_msg_type_and_authorization() {
        let client = QqOfficialOutboundClient::webhook("tok", "https://api.sgroup.qq.com/", "qq1");
        let session = MessageSession::new("qq1", "private:u-1");
        let chain = MessageChain::new(vec![MessageComponent::plain("hi")]);
        let req = &client.requests_for_chain(&session, &chain)[0];
        assert!(req.endpoint.ends_with("/v2/users/u-1/messages"));
        let body = body_as_value(req);
        assert_eq!(body["content"], "hi");
        assert_eq!(body["msg_type"], 0);
        let auth = req
            .headers
            .iter()
            .find(|(name, _)| name == "authorization")
            .map(|(_, v)| v.as_str());
        assert_eq!(auth, Some("QQBot tok"));
    }

    #[test]
    fn reply_attaches_msg_id() {
        let client = QqOfficialOutboundClient::webhook("tok", "https://api.sgroup.qq.com/", "qq1");
        let session = MessageSession::new("qq1", "private:u-1");
        let chain = MessageChain::new(vec![
            MessageComponent::reply("99", ""),
            MessageComponent::plain("answer"),
        ]);
        let req = &client.requests_for_chain(&session, &chain)[0];
        let body = body_as_value(req);
        assert_eq!(body["msg_id"], "99");
    }

    #[test]
    fn group_session_routes_to_group_endpoint() {
        let client = QqOfficialOutboundClient::webhook("tok", "https://api.sgroup.qq.com/", "qq1");
        let session = MessageSession::group("qq1", "group:g-77");
        let chain = MessageChain::new(vec![MessageComponent::plain("hi")]);
        let req = &client.requests_for_chain(&session, &chain)[0];
        assert!(req.endpoint.ends_with("/v2/groups/g-77/messages"));
    }

    #[test]
    fn guild_group_session_routes_to_channels() {
        let client = QqOfficialOutboundClient::guild("tok", "https://api.sgroup.qq.com/", "qq2");
        let session = MessageSession::group("qq2", "group:c-9001");
        let chain = MessageChain::new(vec![MessageComponent::plain("hi")]);
        let req = &client.requests_for_chain(&session, &chain)[0];
        assert!(req.endpoint.ends_with("/channels/c-9001/messages"));
    }

    #[test]
    fn image_chain_uses_msg_type_7() {
        let client = QqOfficialOutboundClient::webhook("tok", "https://api.sgroup.qq.com/", "qq1");
        let session = MessageSession::new("qq1", "private:u-1");
        let chain = MessageChain::new(vec![MessageComponent::image("https://i/p.png")]);
        let req = &client.requests_for_chain(&session, &chain)[0];
        let body = body_as_value(req);
        assert_eq!(body["image"], "https://i/p.png");
        assert_eq!(body["msg_type"], 7);
    }
}
