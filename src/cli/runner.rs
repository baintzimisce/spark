//! Async subprocess runner with `Stdio::null()` stdin, line-by-line streaming, and cancellation.

use crate::cli::AgentKind;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Parameters for invoking an agent CLI in non-interactive mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRequest {
    pub agent: AgentKind,
    pub prompt: String,
    pub working_dir: Option<PathBuf>,
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub effort: Option<String>,
}

impl RunRequest {
    /// Create a minimal non-interactive request for `agent` and `prompt`.
    pub fn new(agent: AgentKind, prompt: impl Into<String>) -> Self {
        Self {
            agent,
            prompt: prompt.into(),
            working_dir: None,
            session_id: None,
            model: None,
            mode: None,
            effort: None,
        }
    }
}

/// Incremental output and lifecycle events emitted by a running agent CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamEvent {
    StdoutLine {
        agent: AgentKind,
        line: String,
    },
    StderrLine {
        agent: AgentKind,
        line: String,
    },
    Finished {
        agent: AgentKind,
        exit_code: Option<i32>,
        success: bool,
    },
    Error {
        agent: AgentKind,
        message: String,
    },
}

/// Handle to a running agent subprocess that allows clean termination.
pub struct RunningProcess {
    kill_tx: Option<oneshot::Sender<()>>,
}

impl RunningProcess {
    /// Request immediate termination (`Child::kill()`) of the running subprocess.
    ///
    /// Returns `true` if a cancellation signal was sent, or `false` if the
    /// process was already cancelled or dropped.
    pub fn cancel(&mut self) -> bool {
        if let Some(tx) = self.kill_tx.take() {
            tx.send(()).is_ok()
        } else {
            false
        }
    }
}

/// Construct CLI arguments for non-interactive execution of `req`.
pub fn build_args(req: &RunRequest) -> Vec<String> {
    let mut args = Vec::new();

    match req.agent {
        AgentKind::Agy => {
            args.push("-p".to_string());
            args.push(req.prompt.clone());
            args.push("--output-format".to_string());
            args.push("text".to_string());

            if let Some(session_id) = req.session_id.as_deref().filter(|s| !s.is_empty()) {
                args.push("--conversation".to_string());
                args.push(session_id.to_string());
            }
            if let Some(model) = req.model.as_deref().filter(|s| !s.is_empty()) {
                args.push("--model".to_string());
                args.push(model.to_string());
            }
            if let Some(mode) = req.mode.as_deref().filter(|s| !s.is_empty()) {
                args.push("--mode".to_string());
                args.push(mode.to_string());
            }
            if let Some(effort) = req.effort.as_deref().filter(|s| !s.is_empty()) {
                args.push("--effort".to_string());
                args.push(effort.to_string());
            }
        }
        AgentKind::Claude => {
            args.push("-p".to_string());
            args.push(req.prompt.clone());
            args.push("--output-format".to_string());
            args.push("text".to_string());

            if let Some(session_id) = req.session_id.as_deref().filter(|s| !s.is_empty()) {
                args.push("--resume".to_string());
                args.push(session_id.to_string());
            }
            if let Some(model) = req.model.as_deref().filter(|s| !s.is_empty()) {
                args.push("--model".to_string());
                args.push(model.to_string());
            }
            if let Some(effort) = req.effort.as_deref().filter(|s| !s.is_empty()) {
                args.push("--effort".to_string());
                args.push(effort.to_string());
            }
        }
        AgentKind::OpenCode => {
            args.push("run".to_string());
            args.push("--auto".to_string());

            if let Some(session_id) = req.session_id.as_deref().filter(|s| !s.is_empty()) {
                args.push("--session".to_string());
                args.push(session_id.to_string());
            }
            if let Some(model) = req.model.as_deref().filter(|s| !s.is_empty()) {
                args.push("--model".to_string());
                args.push(model.to_string());
            }

            args.push(req.prompt.clone());
        }
    }

    args
}

/// Validate a `RunRequest` before spawning a subprocess.
///
/// Rejects empty prompts, control characters (`\0`, `\r`, `\n`) in CLI flag
/// values, and non-existent working directories.
pub fn validate_request(req: &RunRequest) -> Result<(), String> {
    if req.prompt.trim().is_empty() {
        return Err("Prompt must not be empty".to_string());
    }

    for (field, value) in [
        ("session_id", req.session_id.as_deref()),
        ("model", req.model.as_deref()),
        ("mode", req.mode.as_deref()),
        ("effort", req.effort.as_deref()),
    ] {
        if let Some(val) = value {
            if val.chars().any(|c| c == '\0' || c == '\r' || c == '\n') {
                return Err(format!(
                    "Invalid control character in '{field}' flag value"
                ));
            }
        }
    }

    if let Some(dir) = req.working_dir.as_deref() {
        if !dir.is_dir() {
            return Err(format!(
                "Working directory does not exist: {}",
                dir.display()
            ));
        }
    }

    Ok(())
}

/// Spawn `binary_path` with `req`, streaming stdout/stderr lines over `tx`.
///
/// Always configures `Stdio::null()` for stdin per Windows CLI safety rules.
pub fn spawn_agent(
    binary_path: &Path,
    req: &RunRequest,
    tx: mpsc::UnboundedSender<StreamEvent>,
) -> Result<RunningProcess, String> {
    validate_request(req)?;
    let args = build_args(req);
    spawn_command(binary_path, &args, req.working_dir.as_deref(), req.agent, tx)
}

/// Low-level helper that spawns `program` with explicit `args`, used by `spawn_agent`
/// and unit tests.
fn spawn_command(
    program: &Path,
    args: &[String],
    working_dir: Option<&Path>,
    agent: AgentKind,
    tx: mpsc::UnboundedSender<StreamEvent>,
) -> Result<RunningProcess, String> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(dir) = working_dir {
        cmd.current_dir(dir);
    }

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd
        .spawn()
        .map_err(|err| format!("Failed to spawn {}: {err}", program.display()))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (kill_tx, mut kill_rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        let stdout_task = tokio::spawn({
            let tx = tx.clone();
            async move {
                if let Some(out) = stdout {
                    let mut lines = BufReader::new(out).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        if tx.send(StreamEvent::StdoutLine { agent, line }).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        let stderr_task = tokio::spawn({
            let tx = tx.clone();
            async move {
                if let Some(err) = stderr {
                    let mut lines = BufReader::new(err).lines();
                    while let Ok(Some(line)) = lines.next_line().await {
                        if tx.send(StreamEvent::StderrLine { agent, line }).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        let exit_status = tokio::select! {
            status = child.wait() => status,
            _ = &mut kill_rx => {
                let _ = child.kill().await;
                child.wait().await
            }
        };

        let _ = stdout_task.await;
        let _ = stderr_task.await;

        match exit_status {
            Ok(status) => {
                let _ = tx.send(StreamEvent::Finished {
                    agent,
                    exit_code: status.code(),
                    success: status.success(),
                });
            }
            Err(err) => {
                let _ = tx.send(StreamEvent::Error {
                    agent,
                    message: format!("Process wait failed: {err}"),
                });
            }
        }
    });

    Ok(RunningProcess {
        kill_tx: Some(kill_tx),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_args_for_agy_includes_prompt_and_optional_flags() {
        let mut req = RunRequest::new(AgentKind::Agy, "hello world");
        assert_eq!(
            build_args(&req),
            vec!["-p", "hello world", "--output-format", "text"]
        );

        req.session_id = Some("conv-123".to_string());
        req.model = Some("gemini-2.5-pro".to_string());
        req.mode = Some("auto".to_string());
        req.effort = Some("high".to_string());

        assert_eq!(
            build_args(&req),
            vec![
                "-p",
                "hello world",
                "--output-format",
                "text",
                "--conversation",
                "conv-123",
                "--model",
                "gemini-2.5-pro",
                "--mode",
                "auto",
                "--effort",
                "high",
            ]
        );
    }

    #[test]
    fn build_args_for_claude_includes_prompt_and_optional_flags() {
        let mut req = RunRequest::new(AgentKind::Claude, "explain main.rs");
        assert_eq!(
            build_args(&req),
            vec!["-p", "explain main.rs", "--output-format", "text"]
        );

        req.session_id = Some("sess-456".to_string());
        req.model = Some("sonnet".to_string());
        req.effort = Some("medium".to_string());

        assert_eq!(
            build_args(&req),
            vec![
                "-p",
                "explain main.rs",
                "--output-format",
                "text",
                "--resume",
                "sess-456",
                "--model",
                "sonnet",
                "--effort",
                "medium",
            ]
        );
    }

    #[test]
    fn build_args_for_opencode_includes_run_auto_and_optional_flags() {
        let mut req = RunRequest::new(AgentKind::OpenCode, "fix tests");
        assert_eq!(build_args(&req), vec!["run", "--auto", "fix tests"]);

        req.session_id = Some("oc-789".to_string());
        req.model = Some("anthropic/claude-sonnet-4".to_string());

        assert_eq!(
            build_args(&req),
            vec![
                "run",
                "--auto",
                "--session",
                "oc-789",
                "--model",
                "anthropic/claude-sonnet-4",
                "fix tests",
            ]
        );
    }

    #[tokio::test]
    async fn spawn_command_streams_stdout_and_completes() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        #[cfg(windows)]
        let (program, args) = (
            Path::new("cmd"),
            vec![
                "/C".to_string(),
                "echo line-one & echo line-two".to_string(),
            ],
        );
        #[cfg(not(windows))]
        let (program, args) = (
            Path::new("sh"),
            vec![
                "-c".to_string(),
                "printf 'line-one\\nline-two\\n'".to_string(),
            ],
        );

        let _proc = spawn_command(program, &args, None, AgentKind::Agy, tx)
            .expect("test command should spawn");

        let mut lines = Vec::new();
        let mut finished = None;

        while let Some(ev) = rx.recv().await {
            match ev {
                StreamEvent::StdoutLine { line, .. } => lines.push(line.trim().to_string()),
                StreamEvent::Finished { success, .. } => {
                    finished = Some(success);
                    break;
                }
                StreamEvent::Error { message, .. } => panic!("unexpected error: {message}"),
                StreamEvent::StderrLine { .. } => {}
            }
        }

        assert_eq!(lines, vec!["line-one", "line-two"]);
        assert_eq!(finished, Some(true));
    }

    #[tokio::test]
    async fn spawn_command_can_be_cancelled() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        #[cfg(windows)]
        let (program, args) = (
            Path::new("powershell"),
            vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "Start-Sleep -Seconds 30".to_string(),
            ],
        );
        #[cfg(not(windows))]
        let (program, args) = (Path::new("sleep"), vec!["30".to_string()]);

        let mut proc = spawn_command(program, &args, None, AgentKind::Claude, tx)
            .expect("sleep command should spawn");

        assert!(proc.cancel());
        assert!(!proc.cancel());

        let mut saw_finished = false;
        while let Some(ev) = rx.recv().await {
            if let StreamEvent::Finished { success, .. } = ev {
                assert!(!success);
                saw_finished = true;
                break;
            }
        }

        assert!(saw_finished);
    }

    #[tokio::test]
    async fn spawn_command_nullifies_stdin_so_child_gets_immediate_eof() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        #[cfg(windows)]
        let (program, args) = (
            Path::new("powershell"),
            vec![
                "-NoProfile".to_string(),
                "-Command".to_string(),
                "[Console]::In.ReadToEnd() | Out-Null; Write-Output 'stdin-eof'".to_string(),
            ],
        );
        #[cfg(not(windows))]
        let (program, args) = (
            Path::new("sh"),
            vec![
                "-c".to_string(),
                "cat >/dev/null; echo stdin-eof".to_string(),
            ],
        );

        let _proc = spawn_command(program, &args, None, AgentKind::Agy, tx)
            .expect("stdin test command should spawn");

        let mut lines = Vec::new();
        let mut finished = None;
        let deadline = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while let Some(ev) = rx.recv().await {
                match ev {
                    StreamEvent::StdoutLine { line, .. } => lines.push(line.trim().to_string()),
                    StreamEvent::Finished { success, .. } => {
                        finished = Some(success);
                        break;
                    }
                    StreamEvent::Error { message, .. } => panic!("unexpected error: {message}"),
                    StreamEvent::StderrLine { .. } => {}
                }
            }
        })
        .await;

        assert!(deadline.is_ok(), "child hung waiting for stdin");
        assert_eq!(lines, vec!["stdin-eof"]);
        assert_eq!(finished, Some(true));
    }
}
