//! HTTP / Server-Sent-Events MCP transport.
//!
//! MCP over HTTP defines two endpoint shapes:
//!
//! - **`Sse`** — server-initiated stream of newline-delimited JSON-RPC
//!   frames carried over `text/event-stream`. Client → server frames are
//!   delivered through a separate POST endpoint that the server advertises
//!   via the initial `endpoint` event.
//! - **`StreamableHttp`** — single bidirectional HTTP/2 connection. Client
//!   posts frames, server responds with one-or-more frames per request.
//!
//! This module implements both shapes against the host's `reqwest` client.
//! The transport handles framing only; protocol-layer concerns (initialise,
//! tool listings) live above.

use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::transport::McpJsonRpcFrame;
use crate::{McpError, McpResult};

/// HTTP client for the `StreamableHttp` shape: every request posts a single
/// JSON-RPC frame and reads a response body that may contain multiple
/// newline-delimited frames.
pub struct McpStreamableHttpSession {
    inner: Arc<McpStreamableHttpInner>,
}

struct McpStreamableHttpInner {
    client: Client,
    url: String,
    pending_responses: Mutex<Vec<McpJsonRpcFrame>>,
}

impl McpStreamableHttpSession {
    /// Build a session against the given URL. Uses a fresh `reqwest::Client`
    /// configured for direct (no-proxy) calls.
    pub fn new(url: impl Into<String>) -> McpResult<Self> {
        let url = url.into();
        if url.trim().is_empty() {
            return Err(McpError::InvalidConfig(
                "streamable HTTP MCP transport requires a non-empty URL".into(),
            ));
        }
        let client = Client::builder()
            .build()
            .map_err(|err| McpError::Transport(format!("build reqwest client: {err}")))?;
        Ok(Self {
            inner: Arc::new(McpStreamableHttpInner {
                client,
                url,
                pending_responses: Mutex::new(Vec::new()),
            }),
        })
    }

    /// Test seam: construct a session using a caller-supplied client. Lets
    /// integration tests swap in a mock HTTP layer.
    pub fn with_client(client: Client, url: impl Into<String>) -> McpResult<Self> {
        let url = url.into();
        if url.trim().is_empty() {
            return Err(McpError::InvalidConfig(
                "streamable HTTP MCP transport requires a non-empty URL".into(),
            ));
        }
        Ok(Self {
            inner: Arc::new(McpStreamableHttpInner {
                client,
                url,
                pending_responses: Mutex::new(Vec::new()),
            }),
        })
    }

    pub fn url(&self) -> &str {
        &self.inner.url
    }

    /// POST a single JSON-RPC frame and queue every frame found in the
    /// response body. Reads can pull them out via [`Self::recv`].
    pub async fn send(&self, frame: &McpJsonRpcFrame) -> McpResult<()> {
        let body = serde_json::to_vec(&frame.value)
            .map_err(|err| McpError::Transport(format!("encode JSON-RPC frame: {err}")))?;
        let response = self
            .inner
            .client
            .post(&self.inner.url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .body(body)
            .send()
            .await
            .map_err(|err| McpError::Transport(format!("HTTP POST failed: {err}")))?;
        let status = response.status();
        if !status.is_success() {
            return Err(McpError::Transport(format!(
                "MCP HTTP server returned status {status}"
            )));
        }
        let text = response
            .text()
            .await
            .map_err(|err| McpError::Transport(format!("read HTTP body: {err}")))?;
        let frames = parse_frames_body(&text);
        if !frames.is_empty() {
            self.inner.pending_responses.lock().await.extend(frames);
        }
        Ok(())
    }

    /// Pop the next pending response frame, if any.
    pub async fn recv(&self) -> Option<McpJsonRpcFrame> {
        let mut pending = self.inner.pending_responses.lock().await;
        if pending.is_empty() {
            None
        } else {
            Some(pending.remove(0))
        }
    }

    /// Send a request and wait for the matching response (by `id`). The
    /// caller's `read_timeout` bounds how long we'll wait for the POST to
    /// finish; for `StreamableHttp` the response is normally synchronous.
    pub async fn request(
        &self,
        frame: McpJsonRpcFrame,
        read_timeout: Duration,
    ) -> McpResult<McpJsonRpcFrame> {
        let send_future = self.send(&frame);
        tokio::time::timeout(read_timeout, send_future)
            .await
            .map_err(|_| {
                McpError::Transport(format!(
                    "MCP HTTP request timed out after {}ms",
                    read_timeout.as_millis()
                ))
            })??;
        // Look for the response matching the frame's id; for simple
        // StreamableHttp servers it'll be the next frame.
        let expected = frame.id().cloned();
        let mut buffered: Vec<McpJsonRpcFrame> = Vec::new();
        loop {
            let next = self.recv().await;
            match next {
                Some(frame) if frame.id() == expected.as_ref() => {
                    // Restore any frames we pulled past while searching.
                    if !buffered.is_empty() {
                        let mut pending = self.inner.pending_responses.lock().await;
                        for buffered_frame in buffered.into_iter().rev() {
                            pending.insert(0, buffered_frame);
                        }
                    }
                    return Ok(frame);
                }
                Some(frame) => buffered.push(frame),
                None => {
                    return Err(McpError::Transport(format!(
                        "no MCP response received for id {expected:?}"
                    )));
                }
            }
        }
    }

    /// Drain queued responses. No subprocess to kill on HTTP transports.
    pub async fn close(&self) {
        self.inner.pending_responses.lock().await.clear();
    }
}

/// Parse a chunk of HTTP body text into JSON-RPC frames. Handles three
/// shapes the wild encounters:
///
/// - Single JSON object on its own line.
/// - Newline-delimited JSON.
/// - `text/event-stream` formatted blocks (`data: { ... }` lines).
pub fn parse_frames_body(body: &str) -> Vec<McpJsonRpcFrame> {
    let mut out = Vec::new();
    // Try parsing each non-empty line. SSE `data:` lines are stripped of
    // their prefix before parsing.
    for raw_line in body.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let candidate = match line.strip_prefix("data:") {
            Some(rest) => rest.trim(),
            None => line,
        };
        // Skip non-JSON SSE control lines like `event: endpoint`.
        if !candidate.starts_with('{') {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(candidate) {
            if value
                .as_object()
                .and_then(|obj| obj.get("jsonrpc").and_then(Value::as_str))
                == Some("2.0")
            {
                out.push(McpJsonRpcFrame { value });
            }
        }
    }
    // Also try the body as a single JSON object as a fallback.
    if out.is_empty() {
        if let Ok(value) = serde_json::from_str::<Value>(body.trim()) {
            if value
                .as_object()
                .and_then(|obj| obj.get("jsonrpc").and_then(Value::as_str))
                == Some("2.0")
            {
                out.push(McpJsonRpcFrame { value });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_single_json_object_body() {
        let body = r#"{"jsonrpc":"2.0","id":1,"result":"ok"}"#;
        let frames = parse_frames_body(body);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].id(), Some(&json!(1)));
    }

    #[test]
    fn parse_newline_delimited_frames() {
        let body = "{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"ok\"}\n{\"jsonrpc\":\"2.0\",\"method\":\"notify\"}";
        let frames = parse_frames_body(body);
        assert_eq!(frames.len(), 2);
    }

    #[test]
    fn parse_sse_data_frames() {
        let body = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":7,\"result\":42}\n\n";
        let frames = parse_frames_body(body);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].id(), Some(&json!(7)));
        assert_eq!(frames[0].value["result"], 42);
    }

    #[test]
    fn parse_ignores_non_jsonrpc_objects() {
        let body = "{\"hello\":\"world\"}\n{\"jsonrpc\":\"2.0\",\"id\":1}";
        let frames = parse_frames_body(body);
        assert_eq!(frames.len(), 1);
    }

    #[test]
    fn empty_url_rejected() {
        assert!(McpStreamableHttpSession::new("").is_err());
        assert!(McpStreamableHttpSession::new("   ").is_err());
    }

    #[tokio::test]
    async fn recv_returns_none_when_no_pending() {
        let session = McpStreamableHttpSession::new("http://localhost:0/mcp").unwrap();
        assert!(session.recv().await.is_none());
    }
}
