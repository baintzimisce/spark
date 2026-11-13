//! CLI discovery and async subprocess streaming for Spark.

#![allow(dead_code)]

pub mod detectors;
pub mod models;
pub mod runner;
pub mod sessions;

use serde::{Deserialize, Serialize};
use std::fmt;

/// Supported local coding-agent CLIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentKind {
    Agy,
    Claude,
    OpenCode,
}

impl AgentKind {
    /// All supported agents in display order.
    pub const ALL: [AgentKind; 3] = [AgentKind::Agy, AgentKind::Claude, AgentKind::OpenCode];

    /// Executable name searched on `PATH`.
    pub const fn binary_name(self) -> &'static str {
        match self {
            AgentKind::Agy => "agy",
            AgentKind::Claude => "claude",
            AgentKind::OpenCode => "opencode",
        }
    }

    /// Human-readable agent name for UI headers.
    pub const fn display_name(self) -> &'static str {
        match self {
            AgentKind::Agy => "Antigravity",
            AgentKind::Claude => "Claude Code",
            AgentKind::OpenCode => "OpenCode",
        }
    }

    /// Distinguishing Impeccable glyph for each agent.
    pub const fn glyph(self) -> &'static str {
        match self {
            AgentKind::Agy => "◆",
            AgentKind::Claude => "●",
            AgentKind::OpenCode => "▲",
        }
    }
}

impl fmt::Display for AgentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.display_name(), self.binary_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_kind_metadata_is_consistent() {
        assert_eq!(AgentKind::ALL.len(), 3);
        assert_eq!(AgentKind::Agy.glyph(), "◆");
        assert_eq!(AgentKind::Claude.glyph(), "●");
        assert_eq!(AgentKind::OpenCode.glyph(), "▲");
        assert_eq!(AgentKind::Agy.to_string(), "Antigravity (agy)");
        assert_eq!(AgentKind::Claude.to_string(), "Claude Code (claude)");
        assert_eq!(AgentKind::OpenCode.to_string(), "OpenCode (opencode)");
    }
}
