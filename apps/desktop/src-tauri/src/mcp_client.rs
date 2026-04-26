//! Minimal MCP client over stdio JSON-RPC.
//!
//! Only `tools/list` and `tools/call` are implemented — that's enough for the
//! event watcher to discover a "list new items" tool and poll it. The MCP
//! framing is the same newline-delimited JSON ACP uses, so this client is a
//! near-clone of the ACP transport without the streaming notification path.

use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};
use tokio::time::timeout;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("could not spawn `{0}`: {1}")]
    Spawn(String, std::io::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("mcp returned error {code}: {message}")]
    Rpc { code: i64, message: String },
    #[error("request timed out")]
    Timeout,
    #[error("mcp process exited before responding")]
    Closed,
}

pub struct McpClient {
    _child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Arc<Mutex<std::collections::HashMap<u64, oneshot::Sender<Value>>>>,
    next_id: Arc<Mutex<u64>>,
}

impl McpClient {
    /// Spawn an MCP server. `command` may be a bare program (`"gmail-mcp"`) or
    /// a full path; tokio searches `$PATH`.
    pub async fn spawn(command: &str) -> Result<Self, McpError> {
        let mut child = Command::new(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| McpError::Spawn(command.to_string(), e))?;

        let stdin = child.stdin.take().ok_or(McpError::Closed)?;
        let stdout = child.stdout.take().ok_or(McpError::Closed)?;
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            tracing::debug!(target: "mcp.stderr", "{}", line.trim_end());
                        }
                    }
                }
            });
        }

        let pending: Arc<Mutex<std::collections::HashMap<u64, oneshot::Sender<Value>>>> =
            Arc::new(Mutex::new(Default::default()));
        let pending_for_reader = Arc::clone(&pending);
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        let Ok(parsed): Result<Frame, _> = serde_json::from_str(trimmed) else {
                            continue;
                        };
                        if let Some(id) = parsed.id {
                            let mut p = pending_for_reader.lock().await;
                            if let Some(slot) = p.remove(&id) {
                                if let Some(err) = parsed.error {
                                    let _ = slot.send(json!({
                                        "_error": { "code": err.code, "message": err.message }
                                    }));
                                } else {
                                    let _ = slot.send(parsed.result.unwrap_or(Value::Null));
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            _child: child,
            stdin: Arc::new(Mutex::new(stdin)),
            pending,
            next_id: Arc::new(Mutex::new(1)),
        })
    }

    pub async fn list_tools(&self) -> Result<Vec<String>, McpError> {
        let result = self.request("tools/list", json!({})).await?;
        let mut out = Vec::new();
        if let Some(arr) = result.get("tools").and_then(|v| v.as_array()) {
            for tool in arr {
                if let Some(name) = tool.get("name").and_then(|v| v.as_str()) {
                    out.push(name.to_string());
                }
            }
        }
        Ok(out)
    }

    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<Value, McpError> {
        let params = json!({ "name": name, "arguments": arguments });
        let result = self.request("tools/call", params).await?;
        Ok(result)
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, McpError> {
        let id = {
            let mut n = self.next_id.lock().await;
            let cur = *n;
            *n += 1;
            cur
        };
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let frame = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let mut bytes = serde_json::to_vec(&frame)?;
        bytes.push(b'\n');
        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(&bytes).await?;
            stdin.flush().await?;
        }

        match timeout(REQUEST_TIMEOUT, rx).await {
            Ok(Ok(value)) => {
                if let Some(err) = value.get("_error") {
                    return Err(McpError::Rpc {
                        code: err.get("code").and_then(|c| c.as_i64()).unwrap_or(0),
                        message: err
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("unknown")
                            .to_string(),
                    });
                }
                Ok(value)
            }
            Ok(Err(_)) => Err(McpError::Closed),
            Err(_) => Err(McpError::Timeout),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Frame {
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<RpcErr>,
}

#[derive(Debug, Deserialize)]
struct RpcErr {
    code: i64,
    message: String,
}
