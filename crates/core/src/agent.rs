use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::BTreeMap;

use crate::cli::AgentCli;

/// Where an agent comes from. Used by the UI to badge cards and decide
/// whether the agent is editable.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AgentSource {
    /// Agent in canonical format under `~/.senda/agents/`.
    Personal,
    /// Native, pre-existing agent in the CLI's own folder. Read-only.
    External { original_cli: AgentCli },
    /// Agent backed by a connected git repository.
    Repo { repo_id: i64, path: String },
}

/// MCP server declared in canonical kebab-case form.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub struct McpServerSpec {
    /// `local` or `remote`.
    pub r#type: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub url: Option<String>,
    /// Environment variables. Values prefixed with `${secret:...}` are resolved
    /// from the OS keychain at run time.
    pub env: Option<BTreeMap<String, String>>,
}

/// CLI-specific overrides. Only the field for the matching CLI is read by the
/// transpiler; the rest are left alone in the canonical document.
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "kebab-case")]
pub struct CliSpecific {
    pub copilot: Option<CopilotSpecific>,
    #[serde(rename = "claude-code")]
    pub claude_code: Option<ClaudeCodeSpecific>,
    pub gemini: Option<GeminiSpecific>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "kebab-case")]
pub struct CopilotSpecific {
    pub target: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct ClaudeCodeSpecific {
    pub permission_mode: Option<String>,
    pub hooks: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "kebab-case")]
pub struct GeminiSpecific {
    pub model: Option<String>,
}

/// Canonical agent document — superset of the three CLIs, source of truth
/// for everything Senda manages.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub struct CanonicalAgent {
    pub name: String,
    pub description: String,
    /// Required, non-empty. Senda blocks save when empty.
    pub targets: Vec<AgentCli>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub mcp_servers: BTreeMap<String, McpServerSpec>,
    #[serde(default, flatten)]
    pub cli_specific: CliSpecific,
    /// Markdown body — the actual prompt. Lives outside the frontmatter on disk,
    /// so we never serialize it via the YAML codec.
    #[serde(default, skip_serializing)]
    pub body: String,
}

/// View model the frontend consumes. `CanonicalAgent` plus runtime metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    pub agent: CanonicalAgent,
    pub source: AgentSource,
    /// Path of the canonical document on disk, when applicable.
    pub canonical_path: Option<String>,
    pub warnings_count: u32,
}
