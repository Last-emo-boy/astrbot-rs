//! OS-subprocess executor for the Computer Sandbox.
//!
//! Spawns a configured command via `tokio::process::Command` with bounded
//! wall-clock time, optional working directory, and stdout/stderr capture.
//! This is the "Local" execution mode — no container, but the executor
//! still enforces a timeout (the bare minimum to keep a runaway script from
//! pinning the host).
//!
//! For real OS-level isolation (cgroups v2, setrlimit, Windows Job Objects)
//! the sandbox layer above will wrap this executor in platform-specific
//! plumbing; the API surface here stays stable across both paths.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use astrbot_core::{AstrbotError, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;

/// Configuration knobs per subprocess invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SubprocessSpec {
    /// Program to invoke (e.g. `"python"`, `"sh"`).
    pub program: String,
    /// Positional arguments.
    pub args: Vec<String>,
    /// Working directory (defaults to the host process's CWD).
    pub working_dir: Option<PathBuf>,
    /// Extra environment variables. The child still inherits the parent's
    /// environment by default; entries here override or augment.
    pub env: BTreeMap<String, String>,
    /// If `true`, the child sees a freshly cleared environment; only the
    /// `env` map is exposed. Defaults to `false`.
    pub clear_env: bool,
    /// Hard wall-clock budget. The child is killed when it expires.
    pub timeout: Duration,
    /// Optional UTF-8 string written to the child's stdin.
    pub stdin: Option<String>,
}

impl SubprocessSpec {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            working_dir: None,
            env: BTreeMap::new(),
            clear_env: false,
            timeout: Duration::from_secs(30),
            stdin: None,
        }
    }

    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for arg in args {
            self.args.push(arg.into());
        }
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn cleared_env(mut self) -> Self {
        self.clear_env = true;
        self
    }

    pub fn with_stdin(mut self, stdin: impl Into<String>) -> Self {
        self.stdin = Some(stdin.into());
        self
    }
}

/// Outcome of a single subprocess invocation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubprocessOutcome {
    /// `Some(code)` if the child exited normally; `None` on signal /
    /// timeout.
    pub exit_code: Option<i32>,
    /// Capture of the child's stdout (UTF-8 lossy).
    pub stdout: String,
    /// Capture of the child's stderr (UTF-8 lossy).
    pub stderr: String,
    /// True when the subprocess was killed because the configured timeout
    /// elapsed.
    pub timed_out: bool,
}

impl SubprocessOutcome {
    pub fn is_success(&self) -> bool {
        !self.timed_out && self.exit_code == Some(0)
    }
}

/// Execute a single subprocess synchronously and return the outcome.
pub async fn run_subprocess(spec: &SubprocessSpec) -> Result<SubprocessOutcome> {
    if spec.program.trim().is_empty() {
        return Err(AstrbotError::Pipeline(
            "subprocess program is empty".to_string(),
        ));
    }
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args);
    if spec.clear_env {
        cmd.env_clear();
    }
    for (key, value) in &spec.env {
        cmd.env(key, value);
    }
    if let Some(dir) = &spec.working_dir {
        cmd.current_dir(dir);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    if spec.stdin.is_some() {
        cmd.stdin(Stdio::piped());
    } else {
        cmd.stdin(Stdio::null());
    }
    let mut child = cmd
        .spawn()
        .map_err(|err| AstrbotError::Pipeline(format!("spawn `{}` failed: {err}", spec.program)))?;

    if let (Some(input), Some(mut child_stdin)) = (&spec.stdin, child.stdin.take()) {
        use tokio::io::AsyncWriteExt;
        child_stdin
            .write_all(input.as_bytes())
            .await
            .map_err(|err| AstrbotError::Pipeline(format!("subprocess stdin write: {err}")))?;
        drop(child_stdin);
    }

    let wait_future = child.wait_with_output();
    match timeout(spec.timeout, wait_future).await {
        Ok(Ok(output)) => Ok(SubprocessOutcome {
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            timed_out: false,
        }),
        Ok(Err(err)) => Err(AstrbotError::Pipeline(format!(
            "subprocess wait failed: {err}"
        ))),
        Err(_) => Ok(SubprocessOutcome {
            exit_code: None,
            stdout: String::new(),
            stderr: format!("timed out after {}ms", spec.timeout.as_millis()),
            timed_out: true,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn echo_spec() -> SubprocessSpec {
        #[cfg(target_os = "windows")]
        {
            SubprocessSpec::new("powershell")
                .with_arg("-NoProfile")
                .with_arg("-Command")
                .with_arg("Write-Output hello")
                .with_timeout(Duration::from_secs(5))
        }
        #[cfg(not(target_os = "windows"))]
        {
            SubprocessSpec::new("sh")
                .with_arg("-c")
                .with_arg("echo hello")
                .with_timeout(Duration::from_secs(5))
        }
    }

    #[tokio::test]
    async fn echoes_stdout() {
        let outcome = run_subprocess(&echo_spec()).await.unwrap();
        assert!(outcome.is_success());
        assert!(outcome.stdout.trim().contains("hello"));
    }

    #[tokio::test]
    async fn empty_program_rejected() {
        let spec = SubprocessSpec::new("   ");
        let err = run_subprocess(&spec).await.unwrap_err();
        match err {
            AstrbotError::Pipeline(msg) => assert!(msg.contains("empty")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn nonexistent_program_returns_pipeline_error() {
        let spec = SubprocessSpec::new("definitely-not-a-real-program-9bfa7e");
        let err = run_subprocess(&spec).await.unwrap_err();
        match err {
            AstrbotError::Pipeline(msg) => assert!(msg.contains("spawn")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn timeout_kills_long_running_child() {
        // Sleep 5 seconds with a 100ms budget.
        #[cfg(target_os = "windows")]
        let spec = SubprocessSpec::new("powershell")
            .with_arg("-NoProfile")
            .with_arg("-Command")
            .with_arg("Start-Sleep -Seconds 5")
            .with_timeout(Duration::from_millis(200));
        #[cfg(not(target_os = "windows"))]
        let spec = SubprocessSpec::new("sh")
            .with_arg("-c")
            .with_arg("sleep 5")
            .with_timeout(Duration::from_millis(200));
        let outcome = run_subprocess(&spec).await.unwrap();
        assert!(outcome.timed_out);
        assert!(!outcome.is_success());
    }

    #[tokio::test]
    async fn stdin_passes_through() {
        #[cfg(target_os = "windows")]
        let spec = SubprocessSpec::new("powershell")
            .with_arg("-NoProfile")
            .with_arg("-Command")
            .with_arg("$input")
            .with_stdin("hello-from-stdin")
            .with_timeout(Duration::from_secs(5));
        #[cfg(not(target_os = "windows"))]
        let spec = SubprocessSpec::new("cat")
            .with_stdin("hello-from-stdin")
            .with_timeout(Duration::from_secs(5));
        let outcome = run_subprocess(&spec).await.unwrap();
        assert!(outcome.stdout.contains("hello-from-stdin"));
    }
}
