use serde::{Deserialize, Serialize};
use specta::Type;

/// AI CLI flavor that owns custom agent files. Senda is agnostic to the CLI
/// thanks to a canonical format that gets transpiled per [`AgentCli`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum AgentCli {
    Copilot,
    ClaudeCode,
    Gemini,
}

impl AgentCli {
    pub const ALL: [AgentCli; 3] = [AgentCli::Copilot, AgentCli::ClaudeCode, AgentCli::Gemini];

    /// Folder where this CLI stores its native agent files (relative to `$HOME`).
    pub fn agents_dir(self) -> &'static str {
        match self {
            AgentCli::Copilot => ".copilot/agents",
            AgentCli::ClaudeCode => ".claude/agents",
            AgentCli::Gemini => ".gemini/agents",
        }
    }

    /// File extension used by this CLI's native agent format.
    pub fn agent_extension(self) -> &'static str {
        match self {
            AgentCli::Copilot => "agent.md",
            AgentCli::ClaudeCode => "md",
            AgentCli::Gemini => "toml",
        }
    }

    /// Stable string id (kebab-case) used in storage and IPC.
    pub fn as_id(self) -> &'static str {
        match self {
            AgentCli::Copilot => "copilot",
            AgentCli::ClaudeCode => "claude-code",
            AgentCli::Gemini => "gemini",
        }
    }
}
