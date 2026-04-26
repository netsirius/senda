use serde::{Deserialize, Serialize};
use specta::Type;

use crate::cli::AgentCli;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum RunStatus {
    Running,
    Success,
    Failed,
    Cancelled,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    pub id: i64,
    pub agent_id: String,
    pub agent_source: String,
    pub cli: AgentCli,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub exit_code: Option<i32>,
    /// SHA-256 of the prompt — Senda never persists raw prompts.
    pub prompt_hash: String,
    pub cwd: Option<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    pub status: RunStatus,
    pub started_at: i64,
    pub ended_at: i64,
    pub output_text: String,
    pub error_text: Option<String>,
    pub dry_run: bool,
}
