//! JSON-RPC 2.0 frames and ACP-specific request / notification payloads.
//!
//! The wire format is one JSON object per line (LSP-style framing is **not**
//! used in ACP — agents emit and read newline-delimited JSON). Notifications
//! arrive interleaved with responses; the transport layer routes them by
//! `method` vs. presence of `id`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const JSONRPC_VERSION: &str = "2.0";

/// Outgoing JSON-RPC request frame.
#[derive(Debug, Serialize)]
pub struct Request<'a> {
    pub jsonrpc: &'a str,
    pub id: String,
    pub method: &'a str,
    pub params: Value,
}

/// Incoming JSON-RPC frame — either response (has `id`) or notification (no `id`).
#[derive(Debug, Deserialize)]
pub struct Frame {
    #[serde(default)]
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub params: Option<Value>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<Value>,
}

// ── ACP-specific payloads ────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub client_name: String,
    pub client_version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub agent_name: String,
    pub agent_version: String,
    #[serde(default)]
    pub capabilities: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionNewResult {
    #[serde(rename = "sessionId")]
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptParams<'a> {
    pub session_id: &'a str,
    pub prompt: &'a str,
}

/// Streaming update notification emitted by the agent during a prompt.
///
/// Variants match the most common shapes ACP servers send. Anything we don't
/// know about lands in [`SessionUpdate::Other`] so the client never crashes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SessionUpdate {
    AgentMessage {
        content: String,
    },
    ToolCall {
        name: String,
        input: Value,
    },
    ToolResult {
        name: String,
        output: Value,
    },
    Done,
    #[serde(other)]
    Other,
}
