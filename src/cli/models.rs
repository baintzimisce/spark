//! Dynamic model discovery and fallback presets for `agy`, `claude`, and `opencode`.

use crate::cli::detectors::CliStatus;
use crate::cli::AgentKind;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const MODEL_QUERY_TIMEOUT: Duration = Duration::from_secs(8);

/// Selectable model entry for an AI CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelOption {
    pub id: String,
    pub label: String,
}

impl ModelOption {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
        }
    }
}

/// Curated fallback presets for `agent` when dynamic discovery is unavailable.
pub fn default_models_for(agent: AgentKind) -> Vec<ModelOption> {
    match agent {
        AgentKind::Agy => vec![
            ModelOption::new("gemini-4-hydrogen-high", "Gemini 4 Hydrogen (High)"),
            ModelOption::new("gemini-4-argon-medium", "Gemini 4 Argon (Medium)"),
            ModelOption::new("gemini-3.8-flash-high", "Gemini 3.8 Flash (High)"),
            ModelOption::new("claude-sonnet-5-5-medium", "Claude Sonnet 5.5 (Medium)"),
            ModelOption::new("claude-opus-5-5-high", "Claude Opus 5.5 (High)"),
        ],
        AgentKind::Claude => vec![
            ModelOption::new("sonnet", "Claude Sonnet (alias)"),
            ModelOption::new("opus", "Claude Opus (alias)"),
            ModelOption::new("haiku", "Claude Haiku (alias)"),
            ModelOption::new("fable", "Claude Fable (alias)"),
        ],
        AgentKind::OpenCode => vec![
            ModelOption::new("opencode/big-pickle", "opencode/big-pickle"),
            ModelOption::new("opencode/beam-free", "opencode/beam-free"),
        ],
    }
}

/// Parse `agy models` stdout (`<id>\t<Display Name>` per line).
pub fn parse_agy_models(stdout: &str) -> Vec<ModelOption> {
    let mut out = Vec::new();
    for raw in stdout.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("Fetching") {
            continue;
        }
        if let Some((id, label)) = line.split_once('\t') {
            let id = id.trim();
            let label = label.trim();
            if !id.is_empty() {
                out.push(ModelOption::new(
                    id,
                    if label.is_empty() { id } else { label },
                ));
            }
        } else {
            out.push(ModelOption::new(line, line));
        }
    }
    out
}

/// Parse `opencode models` stdout (`provider/model` per line).
pub fn parse_opencode_models(stdout: &str) -> Vec<ModelOption> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("INFO") && line.contains('/'))
        .map(|id| ModelOption::new(id, id))
        .collect()
}

/// Discover available models for a single `agent`. Falls back to built-in presets on error.
pub async fn fetch_models_for(agent: AgentKind, binary_path: Option<&Path>) -> Vec<ModelOption> {
    let presets = default_models_for(agent);
    let Some(bin) = binary_path else {
        return presets;
    };

    if agent == AgentKind::Claude {
        // `claude` CLI accepts aliases and full names via `--model`, with no list subcommand.
        return presets;
    }

    let mut cmd = Command::new(bin);
    cmd.arg("models")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let Ok(Ok(output)) = tokio::time::timeout(MODEL_QUERY_TIMEOUT, cmd.output()).await else {
        return presets;
    };

    if !output.status.success() {
        return presets;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed = match agent {
        AgentKind::Agy => parse_agy_models(&stdout),
        AgentKind::OpenCode => parse_opencode_models(&stdout),
        AgentKind::Claude => presets.clone(),
    };

    if parsed.is_empty() {
        presets
    } else {
        parsed
    }
}

/// Fetch model catalogs for all agents based on their detected `CliStatus`.
pub async fn fetch_all_models(statuses: &[CliStatus]) -> HashMap<AgentKind, Vec<ModelOption>> {
    let mut map = HashMap::new();
    for agent in AgentKind::ALL {
        let bin = statuses
            .iter()
            .find(|s| s.agent == agent && s.available)
            .and_then(|s| s.binary_path.as_deref());
        map.insert(agent, fetch_models_for(agent, bin).await);
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_agy_models_handles_tsv_and_status_lines() {
        let sample = "\
Fetching available models...
gemini-4-hydrogen-high\tGemini 4 Hydrogen (High)
gemini-4-argon-medium\tGemini 4 Argon (Medium)
custom-model-id
";
        let parsed = parse_agy_models(sample);
        assert_eq!(parsed.len(), 3);
        assert_eq!(
            parsed[0],
            ModelOption::new("gemini-4-hydrogen-high", "Gemini 4 Hydrogen (High)")
        );
        assert_eq!(
            parsed[1],
            ModelOption::new("gemini-4-argon-medium", "Gemini 4 Argon (Medium)")
        );
        assert_eq!(
            parsed[2],
            ModelOption::new("custom-model-id", "custom-model-id")
        );
    }

    #[test]
    fn parse_opencode_models_filters_non_model_lines() {
        let sample = "\nINFO loading config\nlocal/gemma-4-12b\nopencode/big-pickle\n\n";
        let parsed = parse_opencode_models(sample);
        assert_eq!(
            parsed,
            vec![
                ModelOption::new("local/gemma-4-12b", "local/gemma-4-12b"),
                ModelOption::new("opencode/big-pickle", "opencode/big-pickle"),
            ]
        );
    }

    #[tokio::test]
    async fn fetch_models_without_binary_returns_presets() {
        for agent in AgentKind::ALL {
            let models = fetch_models_for(agent, None).await;
            assert!(!models.is_empty());
        }
    }
}
