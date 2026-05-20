//! HTTP client wrappers for third-party LLM-agent platforms.
//!
//! Each client encapsulates the request shaping the platform needs:
//! - **Dify**: `POST /v1/chat-messages` with bearer token; `response_mode`
//!   chooses between SSE streaming and blocking JSON.
//! - **Coze**: `POST /v3/chat` with `Authorization: Bearer` and a workflow
//!   `bot_id`.
//! - **DashScope**: `POST /api/v1/apps/{app_id}/completion` (or
//!   `aigc/text-generation/generation` for raw LLM mode), Bearer token.
//!
//! The wrappers here build `reqwest::Request` and parse the response body
//! into a normalised [`ExternalAgentRawStreamEvent`] sequence. SSE handling
//! is intentionally simple: each `data:` line is decoded as JSON and folded
//! into the running event list. Real streaming consumers should drive the
//! HTTP layer themselves and pass each frame through [`Self::parse_event`].

use astrbot_core::{AstrbotError, Result};
use serde_json::{Value, json};

use crate::external::ExternalAgentRawStreamEvent;

/// Dify chat-messages client.
#[derive(Clone, Debug)]
pub struct DifyAgentClient {
    api_base: String,
    api_key: String,
    user: String,
    client: reqwest::Client,
}

impl DifyAgentClient {
    pub fn new(
        api_base: impl Into<String>,
        api_key: impl Into<String>,
        user: impl Into<String>,
    ) -> Self {
        Self {
            api_base: api_base.into().trim().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            user: user.into(),
            client: reqwest::Client::new(),
        }
    }

    pub fn endpoint(&self) -> String {
        format!("{}/v1/chat-messages", self.api_base)
    }

    /// Build the request body Dify expects for blocking mode.
    pub fn body_for(&self, prompt: &str, conversation_id: Option<&str>, streaming: bool) -> Value {
        let mut body = json!({
            "inputs": {},
            "query": prompt,
            "response_mode": if streaming { "streaming" } else { "blocking" },
            "user": self.user,
            "auto_generate_name": false,
        });
        if let Some(cid) = conversation_id {
            body["conversation_id"] = Value::String(cid.to_string());
        }
        body
    }

    /// Blocking call. Returns one final_text event + (optional)
    /// remote_thread_id.
    pub async fn send_blocking(
        &self,
        prompt: &str,
        conversation_id: Option<&str>,
    ) -> Result<Vec<ExternalAgentRawStreamEvent>> {
        let body = self.body_for(prompt, conversation_id, false);
        let response = self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|err| AstrbotError::Pipeline(format!("Dify HTTP failed: {err}")))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|err| AstrbotError::Pipeline(format!("Dify response read failed: {err}")))?;
        if !status.is_success() {
            return Err(AstrbotError::Pipeline(format!(
                "Dify returned {status}: {text}"
            )));
        }
        parse_dify_blocking_response(&text)
    }
}

/// Parse a single SSE-style `data:` payload from Dify into our event shape.
pub fn parse_dify_sse_event(payload: &str) -> Option<ExternalAgentRawStreamEvent> {
    let value: Value = serde_json::from_str(payload).ok()?;
    let event_type = value
        .get("event")
        .and_then(Value::as_str)
        .unwrap_or("message")
        .to_string();
    let conversation_id = value
        .get("conversation_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut event = match event_type.as_str() {
        "message" | "agent_message" => {
            let answer = value.get("answer").and_then(Value::as_str).unwrap_or("");
            ExternalAgentRawStreamEvent::delta(event_type, answer)
        }
        "message_end" => {
            let answer = value.get("answer").and_then(Value::as_str).unwrap_or("");
            ExternalAgentRawStreamEvent::final_text(event_type, answer)
        }
        "error" => {
            let message = value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("dify reported an error");
            ExternalAgentRawStreamEvent::error(event_type, message)
        }
        _ => return None,
    };
    if let Some(cid) = conversation_id {
        event = event.with_remote_thread_id(cid);
    }
    Some(event)
}

fn parse_dify_blocking_response(body: &str) -> Result<Vec<ExternalAgentRawStreamEvent>> {
    let value: Value = serde_json::from_str(body)
        .map_err(|err| AstrbotError::Pipeline(format!("Dify body not JSON: {err}")))?;
    let answer = value
        .get("answer")
        .and_then(Value::as_str)
        .ok_or_else(|| AstrbotError::Pipeline("Dify response missing answer".into()))?;
    let mut event = ExternalAgentRawStreamEvent::final_text("message_end", answer);
    if let Some(cid) = value.get("conversation_id").and_then(Value::as_str) {
        event = event.with_remote_thread_id(cid);
    }
    Ok(vec![event])
}

/// Coze v3 chat client. Coze's response shape is asynchronous: POST returns
/// a `chat_id` + `conversation_id`, then the caller polls `/v3/chat/retrieve`
/// until status is "completed". We expose the request builder + a final-event
/// parser; polling orchestration is the caller's job.
#[derive(Clone, Debug)]
pub struct CozeAgentClient {
    api_base: String,
    api_key: String,
    bot_id: String,
    client: reqwest::Client,
}

impl CozeAgentClient {
    pub fn new(
        api_base: impl Into<String>,
        api_key: impl Into<String>,
        bot_id: impl Into<String>,
    ) -> Self {
        Self {
            api_base: api_base.into().trim().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            bot_id: bot_id.into(),
            client: reqwest::Client::new(),
        }
    }

    pub fn endpoint(&self) -> String {
        format!("{}/v3/chat", self.api_base)
    }

    pub fn body_for(&self, user_id: &str, prompt: &str, conversation_id: Option<&str>) -> Value {
        let mut body = json!({
            "bot_id": self.bot_id,
            "user_id": user_id,
            "auto_save_history": true,
            "additional_messages": [
                { "role": "user", "content_type": "text", "content": prompt }
            ],
        });
        if let Some(cid) = conversation_id {
            body["conversation_id"] = Value::String(cid.to_string());
        }
        body
    }

    pub async fn start_chat(
        &self,
        user_id: &str,
        prompt: &str,
        conversation_id: Option<&str>,
    ) -> Result<CozeChatStarted> {
        let body = self.body_for(user_id, prompt, conversation_id);
        let response = self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|err| AstrbotError::Pipeline(format!("Coze HTTP failed: {err}")))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|err| AstrbotError::Pipeline(format!("Coze response read failed: {err}")))?;
        if !status.is_success() {
            return Err(AstrbotError::Pipeline(format!(
                "Coze returned {status}: {text}"
            )));
        }
        parse_coze_start_response(&text)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CozeChatStarted {
    pub chat_id: String,
    pub conversation_id: String,
}

fn parse_coze_start_response(body: &str) -> Result<CozeChatStarted> {
    let value: Value = serde_json::from_str(body)
        .map_err(|err| AstrbotError::Pipeline(format!("Coze body not JSON: {err}")))?;
    let data = value
        .get("data")
        .ok_or_else(|| AstrbotError::Pipeline("Coze response missing data".into()))?;
    let chat_id = data
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| AstrbotError::Pipeline("Coze response missing data.id".into()))?
        .to_string();
    let conversation_id = data
        .get("conversation_id")
        .and_then(Value::as_str)
        .ok_or_else(|| AstrbotError::Pipeline("Coze response missing conversation_id".into()))?
        .to_string();
    Ok(CozeChatStarted {
        chat_id,
        conversation_id,
    })
}

/// DashScope application completion client. The standard mode is
/// `POST /api/v1/apps/{app_id}/completion`.
#[derive(Clone, Debug)]
pub struct DashScopeAgentClient {
    api_base: String,
    api_key: String,
    app_id: String,
    client: reqwest::Client,
}

impl DashScopeAgentClient {
    pub fn new(
        api_base: impl Into<String>,
        api_key: impl Into<String>,
        app_id: impl Into<String>,
    ) -> Self {
        Self {
            api_base: api_base.into().trim().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            app_id: app_id.into(),
            client: reqwest::Client::new(),
        }
    }

    pub fn endpoint(&self) -> String {
        format!("{}/api/v1/apps/{}/completion", self.api_base, self.app_id)
    }

    pub fn body_for(&self, prompt: &str, session_id: Option<&str>) -> Value {
        let mut body = json!({
            "input": { "prompt": prompt },
            "parameters": {},
            "debug": {},
        });
        if let Some(sid) = session_id {
            body["input"]["session_id"] = Value::String(sid.to_string());
        }
        body
    }

    pub async fn send_blocking(
        &self,
        prompt: &str,
        session_id: Option<&str>,
    ) -> Result<Vec<ExternalAgentRawStreamEvent>> {
        let body = self.body_for(prompt, session_id);
        let response = self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .header("X-DashScope-SSE", "disable")
            .json(&body)
            .send()
            .await
            .map_err(|err| AstrbotError::Pipeline(format!("DashScope HTTP failed: {err}")))?;
        let status = response.status();
        let text = response.text().await.map_err(|err| {
            AstrbotError::Pipeline(format!("DashScope response read failed: {err}"))
        })?;
        if !status.is_success() {
            return Err(AstrbotError::Pipeline(format!(
                "DashScope returned {status}: {text}"
            )));
        }
        parse_dashscope_response(&text)
    }
}

fn parse_dashscope_response(body: &str) -> Result<Vec<ExternalAgentRawStreamEvent>> {
    let value: Value = serde_json::from_str(body)
        .map_err(|err| AstrbotError::Pipeline(format!("DashScope body not JSON: {err}")))?;
    let output = value
        .get("output")
        .ok_or_else(|| AstrbotError::Pipeline("DashScope missing output".into()))?;
    let text = output
        .get("text")
        .and_then(Value::as_str)
        .or_else(|| {
            output
                .get("choices")
                .and_then(Value::as_array)
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("message"))
                .and_then(|msg| msg.get("content"))
                .and_then(Value::as_str)
        })
        .ok_or_else(|| AstrbotError::Pipeline("DashScope missing output.text".into()))?;
    let mut event = ExternalAgentRawStreamEvent::final_text("completion", text);
    if let Some(sid) = output.get("session_id").and_then(Value::as_str) {
        event = event.with_remote_thread_id(sid);
    }
    Ok(vec![event])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dify_endpoint_uses_v1_chat_messages() {
        let client = DifyAgentClient::new("https://api.dify.ai/", "k", "user-1");
        assert_eq!(client.endpoint(), "https://api.dify.ai/v1/chat-messages");
    }

    #[test]
    fn dify_body_carries_query_user_and_conversation() {
        let client = DifyAgentClient::new("https://x", "k", "user-1");
        let body = client.body_for("hello", Some("conv-1"), false);
        assert_eq!(body["query"], "hello");
        assert_eq!(body["user"], "user-1");
        assert_eq!(body["conversation_id"], "conv-1");
        assert_eq!(body["response_mode"], "blocking");
    }

    #[test]
    fn dify_sse_message_decodes_as_delta() {
        let event =
            parse_dify_sse_event(r#"{"event":"message","answer":"hi","conversation_id":"c1"}"#)
                .unwrap();
        assert_eq!(event.event_type, "message");
        assert_eq!(event.text_delta.as_deref(), Some("hi"));
        assert_eq!(event.remote_thread_id.as_deref(), Some("c1"));
    }

    #[test]
    fn dify_sse_message_end_decodes_as_final() {
        let event = parse_dify_sse_event(
            r#"{"event":"message_end","answer":"final","conversation_id":"c1"}"#,
        )
        .unwrap();
        assert_eq!(event.final_text.as_deref(), Some("final"));
    }

    #[test]
    fn dify_blocking_response_emits_final_event() {
        let events =
            parse_dify_blocking_response(r#"{"answer":"full text","conversation_id":"c1"}"#)
                .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].final_text.as_deref(), Some("full text"));
    }

    #[test]
    fn coze_endpoint_is_v3_chat() {
        let client = CozeAgentClient::new("https://api.coze.com/", "k", "bot-1");
        assert_eq!(client.endpoint(), "https://api.coze.com/v3/chat");
    }

    #[test]
    fn coze_body_includes_bot_user_and_additional_message() {
        let client = CozeAgentClient::new("https://x", "k", "bot-1");
        let body = client.body_for("user-1", "hi", None);
        assert_eq!(body["bot_id"], "bot-1");
        assert_eq!(body["user_id"], "user-1");
        assert_eq!(body["additional_messages"][0]["content"], "hi");
    }

    #[test]
    fn coze_start_response_parses_ids() {
        let started = parse_coze_start_response(
            r#"{"data":{"id":"chat-1","conversation_id":"conv-1","status":"in_progress"}}"#,
        )
        .unwrap();
        assert_eq!(started.chat_id, "chat-1");
        assert_eq!(started.conversation_id, "conv-1");
    }

    #[test]
    fn dashscope_endpoint_uses_apps_path() {
        let client = DashScopeAgentClient::new("https://dashscope.aliyuncs.com/", "k", "app-9");
        assert_eq!(
            client.endpoint(),
            "https://dashscope.aliyuncs.com/api/v1/apps/app-9/completion"
        );
    }

    #[test]
    fn dashscope_text_response_emits_final_event() {
        let events =
            parse_dashscope_response(r#"{"output":{"text":"done","session_id":"s1"}}"#).unwrap();
        assert_eq!(events[0].final_text.as_deref(), Some("done"));
        assert_eq!(events[0].remote_thread_id.as_deref(), Some("s1"));
    }
}
