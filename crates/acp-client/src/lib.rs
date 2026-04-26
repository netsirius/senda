//! senda-acp-client — async Rust client for the Agent Client Protocol.
//!
//! ACP is a JSON-RPC 2.0 protocol over the agent process's stdio. This crate
//! does only three things:
//!
//! 1. Spawn the agent process and own its `stdin`/`stdout`.
//! 2. Multiplex outgoing requests with incoming responses by `id`.
//! 3. Forward streaming `session/update` notifications through an mpsc channel.
//!
//! All transport and framing details are kept inside [`transport`]; users of
//! the crate only deal with [`AcpSession`].

pub mod error;
pub mod protocol;
pub mod session;
pub mod transport;

pub use error::AcpError;
pub use protocol::*;
pub use session::AcpSession;

/// Spawn an ACP-capable command (e.g. `claude-code-acp`) and complete the
/// `initialize` handshake. Returns a ready-to-use [`AcpSession`].
pub async fn spawn_agent(command: &str, args: &[&str]) -> Result<AcpSession, AcpError> {
    let transport = transport::StdioTransport::spawn(command, args).await?;
    let session = AcpSession::new(transport).await?;
    session.initialize().await?;
    Ok(session)
}
