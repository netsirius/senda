//! Stdio transport: spawns the agent process, runs a reader task that emits
//! incoming frames into an mpsc, and writes outgoing frames serialized as
//! newline-delimited JSON.

use std::process::Stdio;
use std::sync::Arc;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::{mpsc, Mutex};

use crate::error::AcpError;
use crate::protocol::{Frame, Request, JSONRPC_VERSION};

/// One JSON object per line is the wire format. We allocate a fairly small
/// per-process buffer; agents rarely send anything close to a megabyte at once.
const READ_BUFFER_BYTES: usize = 64 * 1024;

pub struct StdioTransport {
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    pub(crate) inbound: mpsc::UnboundedReceiver<Result<Frame, AcpError>>,
}

impl StdioTransport {
    pub async fn spawn(command: &str, args: &[&str]) -> Result<Self, AcpError> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| AcpError::Spawn {
                command: command.to_string(),
                source: e,
            })?;

        let stdin = child.stdin.take().ok_or(AcpError::ProcessExited)?;
        let stdout = child.stdout.take().ok_or(AcpError::ProcessExited)?;
        let stderr = child.stderr.take();

        let (tx, inbound) = mpsc::unbounded_channel();

        // Reader task: parse one JSON object per line and forward it.
        let reader_tx = tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::with_capacity(READ_BUFFER_BYTES, stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<Frame>(trimmed) {
                            Ok(frame) => {
                                if reader_tx.send(Ok(frame)).is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                let _ = reader_tx.send(Err(AcpError::Decode(e)));
                            }
                        }
                    }
                    Err(e) => {
                        let _ = reader_tx.send(Err(AcpError::Io(e)));
                        break;
                    }
                }
            }
        });

        // Stderr drain — agents print diagnostics there. We keep it visible
        // through `tracing` so logs surface during development without
        // blocking the child's own stderr buffer.
        if let Some(stderr) = stderr {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) | Err(_) => break,
                        Ok(_) => tracing::debug!(target: "acp.stderr", "{}", line.trim_end()),
                    }
                }
            });
        }

        Ok(Self {
            child,
            stdin: Arc::new(Mutex::new(stdin)),
            inbound,
        })
    }

    /// Serialize and write a single request frame followed by a newline.
    pub async fn write_request(
        &self,
        id: &str,
        method: &str,
        params: Value,
    ) -> Result<(), AcpError> {
        let frame = Request {
            jsonrpc: JSONRPC_VERSION,
            id: id.to_string(),
            method,
            params,
        };
        let mut bytes = serde_json::to_vec(&frame).map_err(AcpError::Encode)?;
        bytes.push(b'\n');

        let mut stdin = self.stdin.lock().await;
        stdin.write_all(&bytes).await?;
        stdin.flush().await?;
        Ok(())
    }

    /// Best-effort kill of the child. `Drop` already requests this via
    /// `kill_on_drop`, but explicit shutdowns wait for the process to exit.
    pub async fn shutdown(&mut self) -> Result<(), AcpError> {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
        Ok(())
    }
}
