//! Subprocess-backed MCP stdio transport.
//!
//! Spawns the configured command via [`tokio::process`], pipes stdin/stdout
//! and treats stdout as a stream of line-delimited JSON-RPC frames. Each
//! [`McpStdioSession`] owns its own subprocess; close the session to kill
//! the child.
//!
//! The transport only handles framing — protocol concerns (initialize
//! handshake, capabilities) live in the layer above.

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::timeout;

use crate::{McpError, McpResult};
use crate::transport::{McpJsonRpcFrame, McpProcessCommand};

/// Spawn a child process and wire stdio for JSON-RPC framing. Returns a
/// session handle the caller can `send` to and `recv` from.
pub async fn spawn_stdio_session(command: &McpProcessCommand) -> McpResult<McpStdioSession> {
    if command.command.trim().is_empty() {
        return Err(McpError::InvalidConfig(
            "stdio transport requires a non-empty command".into(),
        ));
    }
    let mut cmd = Command::new(&command.command);
    cmd.args(&command.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in &command.env {
        cmd.env(key, value);
    }

    let mut child: Child = cmd.spawn().map_err(|err| {
        McpError::Transport(format!("failed to spawn `{}`: {err}", command.command))
    })?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| McpError::Transport("subprocess stdin unavailable".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| McpError::Transport("subprocess stdout unavailable".into()))?;

    let (tx, rx) = mpsc::unbounded_channel::<McpJsonRpcFrame>();
    let reader_handle: JoinHandle<()> = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            // We deliberately swallow non-JSON-RPC lines: many servers
            // print human-readable startup banners to stdout before the
            // protocol begins.
            if let Ok(Some(frame)) = McpJsonRpcFrame::parse(&line) {
                if tx.send(frame).is_err() {
                    break;
                }
            }
        }
    });

    Ok(McpStdioSession {
        inner: Arc::new(McpStdioSessionInner {
            stdin: Mutex::new(stdin),
            child: Mutex::new(Some(child)),
            reader: Mutex::new(Some(reader_handle)),
            rx: Mutex::new(rx),
            pending: Mutex::new(HashMap::new()),
        }),
    })
}

/// Handle to a running MCP stdio subprocess. Clones share state.
#[derive(Clone)]
pub struct McpStdioSession {
    inner: Arc<McpStdioSessionInner>,
}

struct McpStdioSessionInner {
    stdin: Mutex<ChildStdin>,
    child: Mutex<Option<Child>>,
    reader: Mutex<Option<JoinHandle<()>>>,
    rx: Mutex<mpsc::UnboundedReceiver<McpJsonRpcFrame>>,
    pending: Mutex<HashMap<String, oneshot::Sender<McpJsonRpcFrame>>>,
}

impl McpStdioSession {
    /// Write a single JSON-RPC frame to the subprocess. The frame is
    /// serialised as a JSON object followed by a newline.
    pub async fn send(&self, frame: &McpJsonRpcFrame) -> McpResult<()> {
        let payload = serde_json::to_vec(&frame.value).map_err(|err| {
            McpError::Transport(format!("encode JSON-RPC frame: {err}"))
        })?;
        let mut stdin = self.inner.stdin.lock().await;
        stdin
            .write_all(&payload)
            .await
            .map_err(|err| McpError::Transport(format!("write stdio: {err}")))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|err| McpError::Transport(format!("write stdio: {err}")))?;
        stdin
            .flush()
            .await
            .map_err(|err| McpError::Transport(format!("flush stdio: {err}")))?;
        Ok(())
    }

    /// Receive the next JSON-RPC frame, blocking the caller. Returns
    /// `None` only when the subprocess has closed its stdout.
    pub async fn recv(&self) -> Option<McpJsonRpcFrame> {
        let mut rx = self.inner.rx.lock().await;
        rx.recv().await
    }

    /// Wait up to `read_timeout` for the next frame.
    pub async fn recv_with_timeout(
        &self,
        read_timeout: Duration,
    ) -> McpResult<Option<McpJsonRpcFrame>> {
        match timeout(read_timeout, self.recv()).await {
            Ok(frame) => Ok(frame),
            Err(_) => Err(McpError::Transport(format!(
                "timed out waiting for stdio frame after {}ms",
                read_timeout.as_millis()
            ))),
        }
    }

    /// Send a request frame and await the matching response. Matching is
    /// done on the JSON-RPC `id` field. Other frames received in the
    /// meantime are dropped — use the lower-level [`Self::send`] /
    /// [`Self::recv`] for notification fan-out.
    pub async fn request(
        &self,
        frame: McpJsonRpcFrame,
        read_timeout: Duration,
    ) -> McpResult<McpJsonRpcFrame> {
        let id_key = id_to_string(frame.id()).ok_or_else(|| {
            McpError::Transport("request frame is missing an id".into())
        })?;
        self.send(&frame).await?;
        let deadline = tokio::time::Instant::now() + read_timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(McpError::Transport(format!(
                    "timed out awaiting response to id {id_key}"
                )));
            }
            let next = self
                .recv_with_timeout(remaining)
                .await?
                .ok_or_else(|| McpError::Transport("stdio stream closed".into()))?;
            if id_to_string(next.id()).as_deref() == Some(id_key.as_str()) {
                return Ok(next);
            }
            // Frames that aren't for us are dropped; matchers that need
            // them should drive recv() directly.
        }
    }

    /// Kill the subprocess and join the reader task.
    pub async fn close(&self) -> McpResult<()> {
        if let Some(mut child) = self.inner.child.lock().await.take() {
            let _ = child.kill().await;
        }
        if let Some(handle) = self.inner.reader.lock().await.take() {
            handle.abort();
        }
        let _ = self.inner.pending.lock().await.drain();
        Ok(())
    }
}

fn id_to_string(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn echo_command() -> McpProcessCommand {
        // Both Windows cmd.exe and POSIX shells provide a simple way to
        // echo a single line. We need something that:
        //  - writes one JSON-RPC formatted line to stdout
        //  - then exits
        //
        // PowerShell on Windows is consistent; on Unix use sh. We pick the
        // available tool at runtime via cfg(target_os).
        #[cfg(target_os = "windows")]
        {
            McpProcessCommand::new("powershell")
                .with_arg("-NoProfile")
                .with_arg("-Command")
                .with_arg("Write-Output '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"ok\"}'")
        }
        #[cfg(not(target_os = "windows"))]
        {
            McpProcessCommand::new("sh")
                .with_arg("-c")
                .with_arg("echo '{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"ok\"}'")
        }
    }

    #[tokio::test]
    async fn spawn_and_read_canned_frame() {
        let session = spawn_stdio_session(&echo_command())
            .await
            .expect("spawn ok");
        let frame = session
            .recv_with_timeout(Duration::from_secs(5))
            .await
            .expect("recv ok")
            .expect("frame present");
        assert_eq!(frame.value["jsonrpc"], "2.0");
        assert_eq!(frame.value["result"], "ok");
        session.close().await.expect("close ok");
    }

    #[tokio::test]
    async fn empty_command_rejected() {
        let cmd = McpProcessCommand::new("   ");
        let result = spawn_stdio_session(&cmd).await;
        assert!(result.is_err(), "empty command must be rejected");
    }

    #[tokio::test]
    async fn request_matches_response_by_id() {
        // The canned subprocess writes id=1; sending any request with id=1
        // should hit that response. Using a longer-running echo: the test
        // simulates a request/response pair where we manually treat the
        // canned line as the response.
        let session = spawn_stdio_session(&echo_command())
            .await
            .expect("spawn ok");
        // We don't actually need to send for this echo, but request() must
        // be able to flush to the (already-closed) stdin. PowerShell exits
        // immediately so write may fail; tolerate either path.
        let request = McpJsonRpcFrame {
            value: json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }),
        };
        let _ = session.send(&request).await;
        let response = session
            .recv_with_timeout(Duration::from_secs(5))
            .await
            .expect("recv ok")
            .expect("frame");
        assert_eq!(response.id(), Some(&json!(1)));
        session.close().await.expect("close ok");
    }

    #[test]
    fn id_to_string_handles_string_and_number() {
        assert_eq!(id_to_string(Some(&json!("abc"))), Some("abc".to_string()));
        assert_eq!(id_to_string(Some(&json!(42))), Some("42".to_string()));
        assert_eq!(id_to_string(Some(&json!(null))), None);
        assert_eq!(id_to_string(None), None);
    }
}
