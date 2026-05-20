//! Telegram-native outbound request builder.
//!
//! The generic [`Wave1Sink`] in [`super::common`] dispatches an AstrBot
//! envelope to a relay. That works for development against the local
//! gateway, but for direct cloud deployment a plugin must call
//! `https://api.telegram.org/bot{TOKEN}/sendMessage` (et al.) with the
//! payload shape Telegram's Bot API expects.
//!
//! [`TelegramOutboundClient`] is the thin builder that converts a
//! [`MessageSession`] + [`MessageChain`] pair into a list of
//! [`PlatformApiRequest`]s targeting Telegram directly.
//!
//! Reference: <https://core.telegram.org/bots/api>.

use astrbot_core::{MessageChain, MessageComponent, MessageSession};
use serde_json::{Value, json};

use crate::{PlatformApiMethod, PlatformApiRequest};

/// Concrete builder for Telegram Bot API requests.
#[derive(Clone, Debug)]
pub struct TelegramOutboundClient {
    bot_token: String,
    api_base_url: String,
    platform_id: String,
}

impl TelegramOutboundClient {
    /// Construct a new client.
    ///
    /// * `bot_token` — value returned by BotFather, e.g. `"123:ABC"`.
    /// * `api_base_url` — base URL **without** the `/bot{token}` suffix.
    ///   The Telegram default is `https://api.telegram.org`. The trailing
    ///   slash is normalised away.
    /// * `platform_id` — id of the AstrBot platform record this client
    ///   serves. Used to derive a rate-limit key per platform.
    pub fn new(
        bot_token: impl Into<String>,
        api_base_url: impl Into<String>,
        platform_id: impl Into<String>,
    ) -> Self {
        let api_base_url = api_base_url.into();
        let normalised = api_base_url.trim().trim_end_matches('/').to_string();
        Self {
            bot_token: bot_token.into(),
            api_base_url: normalised,
            platform_id: platform_id.into(),
        }
    }

    /// `https://api.telegram.org/bot{TOKEN}/{action}`.
    pub fn endpoint_for(&self, action: &str) -> String {
        format!("{}/bot{}/{}", self.api_base_url, self.bot_token, action)
    }

    /// Map [`MessageSession::conversation_id`] to a Telegram `chat_id`.
    /// AstrBot encodes session conversation ids as `private:<id>` or
    /// `group:<id>`; we strip the prefix.
    pub fn chat_id_from_session(session: &MessageSession) -> &str {
        let conv = session.conversation_id.as_str();
        conv.strip_prefix("private:")
            .or_else(|| conv.strip_prefix("group:"))
            .unwrap_or(conv)
    }

    /// Translate a [`MessageChain`] into zero-or-more Telegram requests.
    pub fn requests_for_chain(
        &self,
        session: &MessageSession,
        chain: &MessageChain,
    ) -> Vec<PlatformApiRequest> {
        let chat_id = Self::chat_id_from_session(session).to_string();
        let mut out = Vec::new();
        let mut pending_text = String::new();
        let mut pending_reply: Option<String> = None;

        for component in chain.components() {
            match component {
                MessageComponent::Plain { text } if !text.trim().is_empty() => {
                    if !pending_text.is_empty() {
                        pending_text.push('\n');
                    }
                    pending_text.push_str(text);
                }
                MessageComponent::Reply { message_id, .. } if !message_id.trim().is_empty() => {
                    pending_reply = Some(message_id.clone());
                }
                MessageComponent::Image { url } if !url.trim().is_empty() => {
                    out.push(self.send_photo(&chat_id, url));
                }
                MessageComponent::Record { url } if !url.trim().is_empty() => {
                    out.push(self.send_voice(&chat_id, url));
                }
                MessageComponent::File { name, url } if !url.trim().is_empty() => {
                    out.push(self.send_document(&chat_id, name, url));
                }
                MessageComponent::Video { url } if !url.trim().is_empty() => {
                    out.push(self.send_video(&chat_id, url));
                }
                _ => {}
            }
        }

        if !pending_text.is_empty() {
            out.insert(0, self.send_message(&chat_id, &pending_text, pending_reply));
        }
        out
    }

    fn rate_limit_key(&self, action: &str) -> String {
        format!("telegram:{}:{action}", self.platform_id)
    }

    fn post(&self, action: &str, body: Value) -> PlatformApiRequest {
        PlatformApiRequest::new(
            "telegram".to_string(),
            PlatformApiMethod::Post,
            self.endpoint_for(action),
        )
        .with_header("content-type", "application/json")
        .with_body(body.to_string().into_bytes())
        .with_rate_limit_key(self.rate_limit_key(action))
    }

    fn send_message(
        &self,
        chat_id: &str,
        text: &str,
        reply_to: Option<String>,
    ) -> PlatformApiRequest {
        let mut body = json!({ "chat_id": chat_id, "text": text });
        if let Some(reply_to) = reply_to {
            // `reply_to_message_id` is the spelling Telegram uses.
            body["reply_to_message_id"] = Value::String(reply_to);
        }
        self.post("sendMessage", body)
    }

    fn send_photo(&self, chat_id: &str, url: &str) -> PlatformApiRequest {
        self.post("sendPhoto", json!({ "chat_id": chat_id, "photo": url }))
    }

    fn send_voice(&self, chat_id: &str, url: &str) -> PlatformApiRequest {
        self.post("sendVoice", json!({ "chat_id": chat_id, "voice": url }))
    }

    fn send_document(&self, chat_id: &str, name: &str, url: &str) -> PlatformApiRequest {
        let mut body = json!({ "chat_id": chat_id, "document": url });
        if !name.trim().is_empty() {
            body["caption"] = Value::String(name.to_string());
        }
        self.post("sendDocument", body)
    }

    fn send_video(&self, chat_id: &str, url: &str) -> PlatformApiRequest {
        self.post("sendVideo", json!({ "chat_id": chat_id, "video": url }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body_as_value(req: &PlatformApiRequest) -> Value {
        serde_json::from_slice(&req.body).expect("body must be JSON")
    }

    fn client() -> TelegramOutboundClient {
        TelegramOutboundClient::new("123:ABC", "https://api.telegram.org/", "tg1")
    }

    #[test]
    fn endpoint_includes_bot_token() {
        assert_eq!(
            client().endpoint_for("sendMessage"),
            "https://api.telegram.org/bot123:ABC/sendMessage"
        );
    }

    #[test]
    fn chat_id_strips_session_prefix() {
        let session = MessageSession::new("tg1", "private:42");
        assert_eq!(TelegramOutboundClient::chat_id_from_session(&session), "42");
        let group = MessageSession::group("tg1", "group:9001");
        assert_eq!(TelegramOutboundClient::chat_id_from_session(&group), "9001");
    }

    #[test]
    fn plain_text_chain_emits_send_message() {
        let session = MessageSession::new("tg1", "private:42");
        let chain = MessageChain::new(vec![MessageComponent::plain("hello")]);
        let requests = client().requests_for_chain(&session, &chain);
        assert_eq!(requests.len(), 1);
        let req = &requests[0];
        assert_eq!(req.method, PlatformApiMethod::Post);
        assert!(req.endpoint.ends_with("/sendMessage"));
        let body = body_as_value(req);
        assert_eq!(body["chat_id"], "42");
        assert_eq!(body["text"], "hello");
        assert!(body.get("reply_to_message_id").is_none());
    }

    #[test]
    fn reply_attaches_reply_to_message_id() {
        let session = MessageSession::new("tg1", "private:42");
        let chain = MessageChain::new(vec![
            MessageComponent::reply("99", ""),
            MessageComponent::plain("answer"),
        ]);
        let requests = client().requests_for_chain(&session, &chain);
        assert_eq!(requests.len(), 1);
        let body = body_as_value(&requests[0]);
        assert_eq!(body["reply_to_message_id"], "99");
        assert_eq!(body["text"], "answer");
    }

    #[test]
    fn image_chain_emits_send_photo() {
        let session = MessageSession::new("tg1", "private:42");
        let chain = MessageChain::new(vec![MessageComponent::image("https://example.com/p.png")]);
        let requests = client().requests_for_chain(&session, &chain);
        assert_eq!(requests.len(), 1);
        assert!(requests[0].endpoint.ends_with("/sendPhoto"));
        let body = body_as_value(&requests[0]);
        assert_eq!(body["photo"], "https://example.com/p.png");
    }

    #[test]
    fn mixed_chain_collapses_text_and_emits_attachments() {
        let session = MessageSession::new("tg1", "private:42");
        let chain = MessageChain::new(vec![
            MessageComponent::plain("part one"),
            MessageComponent::image("https://example.com/p.png"),
            MessageComponent::plain("part two"),
            MessageComponent::file("doc.pdf", "https://example.com/d.pdf"),
        ]);
        let requests = client().requests_for_chain(&session, &chain);
        // Expected: 1 sendMessage (text=part one\npart two) + 1 sendPhoto + 1 sendDocument
        assert_eq!(requests.len(), 3);
        let send_message = &requests[0];
        assert!(send_message.endpoint.ends_with("/sendMessage"));
        let send_message_body = body_as_value(send_message);
        assert_eq!(send_message_body["text"], "part one\npart two");
        assert!(requests[1].endpoint.ends_with("/sendPhoto"));
        assert!(requests[2].endpoint.ends_with("/sendDocument"));
        let doc_body = body_as_value(&requests[2]);
        assert_eq!(doc_body["caption"], "doc.pdf");
    }

    #[test]
    fn empty_chain_emits_nothing() {
        let session = MessageSession::new("tg1", "private:42");
        let chain = MessageChain::new(vec![]);
        let requests = client().requests_for_chain(&session, &chain);
        assert!(requests.is_empty());
    }

    #[test]
    fn rate_limit_key_per_platform_and_action() {
        let req = client().send_message("42", "hi", None);
        assert_eq!(
            req.rate_limit_key,
            Some("telegram:tg1:sendMessage".to_string())
        );
    }
}
