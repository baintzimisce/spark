//! Locate `agy`, `claude`, and `opencode` binaries and query their versions.

use crate::cli::AgentKind;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// Result of probing a single agent CLI on the local machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliStatus {
    pub agent: AgentKind,
    pub available: bool,
    pub binary_path: Option<PathBuf>,
    pub version: Option<String>,
    pub error: Option<String>,
}

impl CliStatus {
    fn unavailable(agent: AgentKind, error: impl Into<String>) -> Self {
        Self {
            agent,
            available: false,
            binary_path: None,
            version: None,
            error: Some(error.into()),
        }
    }
}

/// Build Windows fallback paths from standard environment directories.
///
/// Separated from `std::env` lookups so path construction can be unit-tested
/// deterministically.
pub fn fallback_paths_from_env(
    agent: AgentKind,
    local_app_data: Option<&Path>,
    user_profile: Option<&Path>,
    app_data: Option<&Path>,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let bin = agent.binary_name();

    match agent {
        AgentKind::Agy => {
            if let Some(local) = local_app_data {
                paths.push(
                    local
                        .join("Microsoft")
                        .join("WinGet")
                        .join("Links")
                        .join("agy.exe"),
                );
                let packages_dir = local.join("Microsoft").join("WinGet").join("Packages");
                if let Ok(entries) = std::fs::read_dir(packages_dir) {
                    for entry in entries.flatten() {
                        let name = entry.file_name();
                        if name.to_string_lossy().starts_with("Google.AntigravityCLI") {
                            paths.push(entry.path().join("agy.exe"));
                        }
                    }
                }
            }
            if let Some(profile) = user_profile {
                paths.push(profile.join(".local").join("bin").join("agy.exe"));
            }
        }
        AgentKind::Claude => {
            if let Some(profile) = user_profile {
                paths.push(profile.join(".local").join("bin").join("claude.exe"));
            }
            if let Some(roaming) = app_data {
                paths.push(roaming.join("npm").join("claude.cmd"));
            }
            if let Some(local) = local_app_data {
                paths.push(
                    local
                        .join("Microsoft")
                        .join("WinGet")
                        .join("Links")
                        .join("claude.exe"),
                );
            }
        }
        AgentKind::OpenCode => {
            if let Some(roaming) = app_data {
                paths.push(roaming.join("npm").join("opencode.cmd"));
                paths.push(roaming.join("npm").join("opencode.exe"));
            }
            if let Some(profile) = user_profile {
                paths.push(profile.join(".local").join("bin").join("opencode.exe"));
            }
            if let Some(local) = local_app_data {
                paths.push(
                    local
                        .join("Microsoft")
                        .join("WinGet")
                        .join("Links")
                        .join("opencode.exe"),
                );
            }
        }
    }

    // Generic non-Windows / fallback executable name if needed.
    if let Some(profile) = user_profile {
        paths.push(profile.join(".local").join("bin").join(bin));
    }

    paths
}

/// Return candidate fallback paths for `agent` using the current process environment.
pub fn candidate_fallback_paths(agent: AgentKind) -> Vec<PathBuf> {
    let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let user_profile = std::env::var_os("USERPROFILE").map(PathBuf::from);
    let app_data = std::env::var_os("APPDATA").map(PathBuf::from);

    fallback_paths_from_env(
        agent,
        local_app_data.as_deref(),
        user_profile.as_deref(),
        app_data.as_deref(),
    )
}

/// Resolve the executable path for `agent` via `PATH` or known Windows fallbacks.
pub fn resolve_binary(agent: AgentKind) -> Option<PathBuf> {
    if let Ok(found) = which::which(agent.binary_name()) {
        return Some(found);
    }

    candidate_fallback_paths(agent)
        .into_iter()
        .find(|candidate| candidate.is_file())
}

/// Parse the first non-empty trimmed line from `--version` output.
pub fn parse_version_output(stdout: &str, stderr: &str) -> Option<String> {
    stdout
        .lines()
        .chain(stderr.lines())
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

/// Probe a single CLI binary and query its `--version`.
pub async fn detect_cli(agent: AgentKind) -> CliStatus {
    let Some(binary_path) = resolve_binary(agent) else {
        return CliStatus::unavailable(
            agent,
            format!(
                "'{}' not found on PATH or standard install locations",
                agent.binary_name()
            ),
        );
    };

    let mut cmd = Command::new(&binary_path);
    cmd.arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = match tokio::time::timeout(VERSION_TIMEOUT, cmd.output()).await {
        Ok(Ok(out)) => out,
        Ok(Err(err)) => {
            return CliStatus {
                agent,
                available: false,
                binary_path: Some(binary_path),
                version: None,
                error: Some(format!("Failed to execute --version: {err}")),
            };
        }
        Err(_) => {
            return CliStatus {
                agent,
                available: false,
                binary_path: Some(binary_path),
                version: None,
                error: Some("Timed out querying --version".to_string()),
            };
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let version = parse_version_output(&stdout, &stderr);

    if output.status.success() {
        CliStatus {
            agent,
            available: true,
            binary_path: Some(binary_path),
            version,
            error: None,
        }
    } else {
        CliStatus {
            agent,
            available: false,
            binary_path: Some(binary_path),
            version,
            error: Some(format!(
                "--version exited with status {}",
                output
                    .status
                    .code()
                    .map_or_else(|| "unknown".to_string(), |c| c.to_string())
            )),
        }
    }
}

/// What to run in place of an npm `.cmd` shim: the program the shim would start,
/// plus any arguments that must come before the caller's own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShimTarget {
    pub program: PathBuf,
    pub leading_args: Vec<String>,
}

/// `true` when `path` is a `.cmd` or `.bat` file.
pub fn is_batch_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"))
}

/// Find what an npm `.cmd` shim starts, as a path relative to the shim's folder.
///
/// npm writes these files, in a few generations of the same shape: the line that
/// forwards the arguments (`%*`) names the target as `"%dp0%\..."` (older npm:
/// `"%~dp0\..."`). A package that ships a native binary points at an `.exe`; a
/// JavaScript package points at a script and puts `node` in front of it.
pub fn parse_cmd_shim(contents: &str) -> Option<String> {
    for line in contents.lines().filter(|l| l.contains("%*")) {
        let target = line
            .split('"')
            .skip(1)
            .step_by(2)
            .filter_map(|quoted| {
                quoted
                    .strip_prefix("%dp0%\\")
                    .or_else(|| quoted.strip_prefix("%~dp0\\"))
            })
            .last()
            .filter(|rel| !rel.is_empty() && !rel.eq_ignore_ascii_case("node.exe"));
        if let Some(rel) = target {
            return Some(rel.to_string());
        }
    }
    None
}

/// Resolve the program behind the npm shim at `path`, if `path` is one and its
/// target exists. `None` means "run `path` itself".
pub fn resolve_cmd_shim(path: &Path) -> Option<ShimTarget> {
    if !is_batch_file(path) {
        return None;
    }
    let dir = path.parent()?;
    let contents = std::fs::read_to_string(path).ok()?;
    let target = dir.join(parse_cmd_shim(&contents)?);
    if !target.is_file() {
        return None;
    }

    let is_exe = target
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
    if is_exe {
        return Some(ShimTarget {
            program: target,
            leading_args: Vec::new(),
        });
    }

    // A script: the shim runs it with the `node.exe` beside it, else `node` on PATH.
    let bundled_node = dir.join("node.exe");
    let node = if bundled_node.is_file() {
        bundled_node
    } else {
        which::which("node").ok()?
    };
    Some(ShimTarget {
        program: node,
        leading_args: vec![target.to_string_lossy().into_owned()],
    })
}

/// Concurrently detect all three supported CLIs.
pub async fn detect_all() -> Vec<CliStatus> {
    let (agy, claude, opencode) = tokio::join!(
        detect_cli(AgentKind::Agy),
        detect_cli(AgentKind::Claude),
        detect_cli(AgentKind::OpenCode),
    );
    vec![agy, claude, opencode]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cmd_shim_finds_a_native_binary_target() {
        // What npm writes for @anthropic-ai/claude-code 2.x.
        let shim = "@ECHO off\r\nGOTO start\r\n:find_dp0\r\nSET dp0=%~dp0\r\nEXIT /b\r\n:start\r\nSETLOCAL\r\nCALL :find_dp0\r\n\"%dp0%\\node_modules\\@anthropic-ai\\claude-code\\bin\\claude.exe\"   %*\r\n";
        assert_eq!(
            parse_cmd_shim(shim).as_deref(),
            Some("node_modules\\@anthropic-ai\\claude-code\\bin\\claude.exe")
        );
    }

    #[test]
    fn parse_cmd_shim_finds_a_node_script_target() {
        let shim = "@ECHO off\r\nSETLOCAL\r\nCALL :find_dp0\r\n\r\nIF EXIST \"%dp0%\\node.exe\" (\r\n  SET \"_prog=%dp0%\\node.exe\"\r\n) ELSE (\r\n  SET \"_prog=node\"\r\n  SET PATHEXT=%PATHEXT:;.JS;=;%\r\n)\r\n\r\nendLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & \"%_prog%\"  \"%dp0%\\node_modules\\opencode-ai\\bin\\opencode\" %*\r\n";
        assert_eq!(
            parse_cmd_shim(shim).as_deref(),
            Some("node_modules\\opencode-ai\\bin\\opencode")
        );
    }

    #[test]
    fn parse_cmd_shim_reads_the_older_npm_layout_and_ignores_other_batch_files() {
        let old = "@IF EXIST \"%~dp0\\node.exe\" (\r\n  \"%~dp0\\node.exe\"  \"%~dp0\\node_modules\\tool\\cli.js\" %*\r\n) ELSE (\r\n  node  \"%~dp0\\node_modules\\tool\\cli.js\" %*\r\n)\r\n";
        assert_eq!(
            parse_cmd_shim(old).as_deref(),
            Some("node_modules\\tool\\cli.js")
        );
        assert_eq!(parse_cmd_shim("@echo off\r\necho hello %*\r\n"), None);
        assert_eq!(parse_cmd_shim(""), None);
    }

    #[test]
    fn resolve_cmd_shim_leaves_real_executables_alone() {
        assert!(!is_batch_file(Path::new(r"C:\tools\claude.exe")));
        assert!(is_batch_file(Path::new(r"C:\tools\claude.CMD")));
        assert_eq!(resolve_cmd_shim(Path::new(r"C:\tools\claude.exe")), None);
        assert_eq!(resolve_cmd_shim(Path::new(r"C:\no\such\shim.cmd")), None);
    }

    #[test]
    fn fallback_paths_include_expected_windows_locations() {
        let local = Path::new(r"C:\Users\test\AppData\Local");
        let profile = Path::new(r"C:\Users\test");
        let roaming = Path::new(r"C:\Users\test\AppData\Roaming");

        let agy_paths =
            fallback_paths_from_env(AgentKind::Agy, Some(local), Some(profile), Some(roaming));
        assert!(agy_paths
            .iter()
            .any(|p| p.ends_with(Path::new(r"Microsoft\WinGet\Links\agy.exe"))));
        assert!(agy_paths
            .iter()
            .any(|p| p.ends_with(Path::new(r".local\bin\agy.exe"))));

        let claude_paths =
            fallback_paths_from_env(AgentKind::Claude, Some(local), Some(profile), Some(roaming));
        assert!(claude_paths
            .iter()
            .any(|p| p.ends_with(Path::new(r".local\bin\claude.exe"))));
        assert!(claude_paths
            .iter()
            .any(|p| p.ends_with(Path::new(r"npm\claude.cmd"))));

        let opencode_paths = fallback_paths_from_env(
            AgentKind::OpenCode,
            Some(local),
            Some(profile),
            Some(roaming),
        );
        assert!(opencode_paths
            .iter()
            .any(|p| p.ends_with(Path::new(r"npm\opencode.cmd"))));
    }

    #[test]
    fn parse_version_output_trims_and_falls_back_to_stderr() {
        assert_eq!(
            parse_version_output("\n  1.5.5  \n", ""),
            Some("1.5.5".to_string())
        );
        assert_eq!(
            parse_version_output("", "  opencode v2.1.4\n"),
            Some("opencode v2.1.4".to_string())
        );
        assert_eq!(parse_version_output("   \n", "\t\n"), None);
    }

    #[tokio::test]
    async fn detect_all_returns_three_statuses_in_order() {
        let statuses = detect_all().await;
        assert_eq!(statuses.len(), 3);
        assert_eq!(statuses[0].agent, AgentKind::Agy);
        assert_eq!(statuses[1].agent, AgentKind::Claude);
        assert_eq!(statuses[2].agent, AgentKind::OpenCode);
    }
}
