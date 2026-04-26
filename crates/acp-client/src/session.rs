//! High-level [`AcpSession`] API — multiplexes JSON-RPC responses by id and
//! forwards `session/update` notifications to per-prompt mpsc channels.
//!
//! A small router task owns the inbound stream; everything else is just a
//! convenience wrapper around `oneshot` for responses and `mpsc` for streams.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{json, Value};
use tokio::sync::{mpsc, oneshot, Mutex};
use uuid::Uuid;

use crate::error::AcpError;
use crate::protocol::{
    Frame, InitializeParams, InitializeResult, SessionNewResult, SessionPromptParams, SessionUpdate,
};
use crate::transport::StdioTransport;

type ResponseSlot = oneshot::Sender<Result<Value, AcpError>>;

#[derive(Default)]
struct Router {
    pending: HashMap<String, ResponseSlot>,
    /// Most ACP servers serialize one prompt at a time, so we forward every
    /// update to the **active** prompt channel. If the agent supports
    /// concurrent prompts in the future we'll switch to a per-id map.
    update_tx: Option<mpsc::UnboundedSender<SessionUpdate>>,
}

pub struct AcpSession {
    transport: Arc<Mutex<StdioTransport>>,
    router: Arc<Mutex<Router>>,
}

impl AcpSession {
    pub async fn new(mut transport: StdioTransport) -> Result<Self, AcpError> {
        let router = Arc::new(Mutex::new(Router::default()));

        // Take ownership of the inbound receiver and run the router task.
        let inbound = std::mem::replace(&mut transport.inbound, mpsc::unbounded_channel().1);
        let router_clone = Arc::clone(&router);
        tokio::spawn(route_inbound(inbound, router_clone));

        Ok(Self {
            transport: Arc::new(Mutex::new(transport)),
            router,
        })
    }

    pub async fn initialize(&self) -> Result<InitializeResult, AcpError> {
        let params = serde_json::to_value(InitializeParams {
            client_name: "senda".to_string(),
            client_version: env!("CARGO_PKG_VERSION").to_string(),
        })
        .map_err(AcpError::Encode)?;

        let value = self.request("initialize", params).await?;
        serde_json::from_value(value).map_err(AcpError::Decode)
    }

    pub async fn new_session(&self) -> Result<String, AcpError> {
        let value = self.request("session/new", json!({})).await?;
        let parsed: SessionNewResult = serde_json::from_value(value).map_err(AcpError::Decode)?;
        Ok(parsed.session_id)
    }

    /// Send a prompt and return a receiver that yields streaming updates until
    /// [`SessionUpdate::Done`] (or the channel closes).
    pub async fn prompt(
        &self,
        session_id: &str,
        prompt: &str,
    ) -> Result<mpsc::UnboundedReceiver<SessionUpdate>, AcpError> {
        let (tx, rx) = mpsc::unbounded_channel();
        {
            let mut router = self.router.lock().await;
            router.update_tx = Some(tx);
        }

        let params = serde_json::to_value(SessionPromptParams { session_id, prompt })
            .map_err(AcpError::Encode)?;

        // Fire and forget at the JSON-RPC level — completion is signalled by
        // the agent via `SessionUpdate::Done` on the update stream.
        self.request("session/prompt", params).await?;
        Ok(rx)
    }

    pub async fn cancel(&self, session_id: &str) -> Result<(), AcpError> {
        let params = json!({ "sessionId": session_id });
        self.request("session/cancel", params).await?;
        Ok(())
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, AcpError> {
        let id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        {
            let mut router = self.router.lock().await;
            router.pending.insert(id.clone(), tx);
        }

        {
            let transport = self.transport.lock().await;
            transport.write_request(&id, method, params).await?;
        }

        rx.await.map_err(|_| AcpError::Closed)?
    }
}

async fn route_inbound(
    mut inbound: mpsc::UnboundedReceiver<Result<Frame, AcpError>>,
    router: Arc<Mutex<Router>>,
) {
    while let Some(message) = inbound.recv().await {
        match message {
            Ok(frame) => dispatch(frame, &router).await,
            Err(err) => {
                tracing::warn!(?err, "acp inbound error");
            }
        }
    }
    // Stream closed — fail any outstanding requests so callers stop blocking.
    let mut router = router.lock().await;
    for (_, slot) in router.pending.drain() {
        let _ = slot.send(Err(AcpError::Closed));
    }
}

async fn dispatch(frame: Frame, router: &Arc<Mutex<Router>>) {
    if let Some(id) = frame.id {
        let mut router = router.lock().await;
        if let Some(slot) = router.pending.remove(&id) {
            let payload = if let Some(err) = frame.error {
                Err(AcpError::RpcError {
                    code: err.code,
                    message: err.message,
                })
            } else {
                Ok(frame.result.unwrap_or(Value::Null))
            };
            let _ = slot.send(payload);
        } else {
            tracing::warn!(%id, "response for unknown request id");
        }
        return;
    }

    if let (Some(method), Some(params)) = (frame.method, frame.params) {
        if method == "session/update" {
            let update: SessionUpdate = match serde_json::from_value(params) {
                Ok(u) => u,
                Err(e) => {
                    tracing::warn!(?e, "invalid session/update payload");
                    return;
                }
            };
            let router = router.lock().await;
            if let Some(tx) = &router.update_tx {
                let _ = tx.send(update);
            }
        }
    }
}
