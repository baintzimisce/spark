//! Local session continuity and turn history persistence (`~/.spark/history.json`).

use crate::cli::AgentKind;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum number of turn records retained in `history.json`.
pub const MAX_HISTORY_TURNS: usize = 500;

/// Per-agent persisted session and option state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSessionState {
    pub session_id: Option<String>,
    pub model: String,
    pub mode: String,
    pub effort: String,
}

/// Single completed or cancelled agent turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnRecord {
    pub id: u64,
    pub timestamp_epoch_secs: u64,
    pub agent: AgentKind,
    pub prompt: String,
    pub output: String,
    pub stderr: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub session_id: Option<String>,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub effort: Option<String>,
}

/// Persistent store of agent session preferences and turn history.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryStore {
    pub next_id: u64,
    pub sessions: HashMap<AgentKind, AgentSessionState>,
    pub turns: Vec<TurnRecord>,
}

impl HistoryStore {
    /// Return the default history file path (`~/.spark/history.json`).
    pub fn default_path() -> Option<PathBuf> {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)?;
        Some(home.join(".spark").join("history.json"))
    }

    /// Load history from `path`. Returns `Ok(Self::default())` if the file does not exist.
    pub fn load_from(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(path)
            .map_err(|err| format!("Failed to read history file {}: {err}", path.display()))?;
        serde_json::from_str(&raw)
            .map_err(|err| format!("Failed to parse history file {}: {err}", path.display()))
    }

    /// Persist history to `path`, creating parent directories if needed.
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                format!("Failed to create history directory {}: {err}", parent.display())
            })?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|err| format!("Failed to serialize history: {err}"))?;
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, json).map_err(|err| {
            format!("Failed to write temp history file {}: {err}", tmp_path.display())
        })?;
        std::fs::rename(&tmp_path, path).map_err(|err| {
            format!("Failed to commit history file {}: {err}", path.display())
        })
    }

    /// Record a new turn and enforce the `MAX_HISTORY_TURNS` cap.
    #[allow(clippy::too_many_arguments)]
    pub fn record_turn(
        &mut self,
        agent: AgentKind,
        prompt: String,
        output: String,
        stderr: String,
        success: bool,
        exit_code: Option<i32>,
        duration_ms: u64,
        session_id: Option<String>,
        model: Option<String>,
        mode: Option<String>,
        effort: Option<String>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);

        let timestamp_epoch_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.turns.push(TurnRecord {
            id,
            timestamp_epoch_secs,
            agent,
            prompt,
            output,
            stderr,
            success,
            exit_code,
            duration_ms,
            session_id,
            model,
            mode,
            effort,
        });

        if self.turns.len() > MAX_HISTORY_TURNS {
            let overflow = self.turns.len() - MAX_HISTORY_TURNS;
            self.turns.drain(0..overflow);
        }

        id
    }

    /// Return turns for `agent` in reverse chronological order (newest first).
    pub fn turns_for(&self, agent: AgentKind) -> Vec<&TurnRecord> {
        self.turns
            .iter()
            .rev()
            .filter(|turn| turn.agent == agent)
            .collect()
    }

    /// Get or initialize mutable session state for `agent`.
    pub fn session_mut(&mut self, agent: AgentKind) -> &mut AgentSessionState {
        self.sessions.entry(agent).or_default()
    }

    /// Get session state for `agent`, if present.
    pub fn session(&self, agent: AgentKind) -> Option<&AgentSessionState> {
        self.sessions.get(&agent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_history_path(name: &str) -> PathBuf {
        let pid = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir()
            .join(format!("spark_test_{name}_{pid}_{nanos}"))
            .join("history.json")
    }

    #[test]
    fn history_round_trip_preserves_sessions_and_turns() {
        let path = temp_history_path("round_trip");
        let mut store = HistoryStore::default();
        store.session_mut(AgentKind::Agy).session_id = Some("conv-42".to_string());
        store.session_mut(AgentKind::Claude).model = "sonnet".to_string();
        store.session_mut(AgentKind::OpenCode).session_id = Some("oc-9".to_string());

        let id = store.record_turn(
            AgentKind::Agy,
            "ping".to_string(),
            "pong".to_string(),
            String::new(),
            true,
            Some(0),
            120,
            Some("conv-42".to_string()),
            None,
            Some("auto".to_string()),
            None,
        );
        assert_eq!(id, 0);

        store.save_to(&path).expect("save should succeed");
        let loaded = HistoryStore::load_from(&path).expect("load should succeed");
        assert_eq!(loaded, store);
        assert_eq!(loaded.turns_for(AgentKind::Agy).len(), 1);
        assert_eq!(loaded.turns_for(AgentKind::Claude).len(), 0);

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn load_missing_returns_default_and_corrupt_returns_error() {
        let missing = temp_history_path("missing");
        let loaded = HistoryStore::load_from(&missing).expect("missing file should return default");
        assert_eq!(loaded, HistoryStore::default());

        let corrupt = temp_history_path("corrupt");
        std::fs::create_dir_all(corrupt.parent().unwrap()).unwrap();
        std::fs::write(&corrupt, "{ not valid json").unwrap();
        let err = HistoryStore::load_from(&corrupt).expect_err("corrupt JSON must fail");
        assert!(err.contains("Failed to parse history file"));

        let _ = std::fs::remove_dir_all(corrupt.parent().unwrap());
    }

    #[test]
    fn record_turn_enforces_max_history_cap() {
        let mut store = HistoryStore::default();
        for i in 0..(MAX_HISTORY_TURNS + 25) {
            store.record_turn(
                AgentKind::Claude,
                format!("prompt-{i}"),
                "ok".to_string(),
                String::new(),
                true,
                Some(0),
                10,
                None,
                None,
                None,
                None,
            );
        }
        assert_eq!(store.turns.len(), MAX_HISTORY_TURNS);
        assert_eq!(store.turns.first().unwrap().prompt, "prompt-25");
        assert_eq!(
            store.turns.last().unwrap().prompt,
            format!("prompt-{}", MAX_HISTORY_TURNS + 24)
        );
    }
}
