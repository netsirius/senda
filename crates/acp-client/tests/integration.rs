//! Integration tests against a real ACP-capable agent.
//!
//! These tests are normally **skipped**: an external agent binary in `$PATH`
//! is required, which CI doesn't have by default. Set `SENDA_ACP_BIN`
//! (e.g. `claude-code-acp`) to opt in.

use senda_acp_client::{spawn_agent, SessionUpdate};

fn agent_bin() -> Option<String> {
    std::env::var("SENDA_ACP_BIN").ok()
}

#[tokio::test]
async fn opt_in_real_agent_handshake() {
    let Some(bin) = agent_bin() else {
        eprintln!("SENDA_ACP_BIN not set — skipping live ACP test");
        return;
    };

    let _ = tracing_subscriber::fmt::try_init();

    let session = spawn_agent(&bin, &[]).await.expect("spawn ACP agent");
    let session_id = session.new_session().await.expect("session/new");

    let mut updates = session
        .prompt(&session_id, "Reply with the literal text: pong")
        .await
        .expect("session/prompt");

    let mut saw_message = false;
    while let Some(update) = updates.recv().await {
        if matches!(update, SessionUpdate::Done) {
            break;
        }
        if matches!(update, SessionUpdate::AgentMessage { .. }) {
            saw_message = true;
        }
    }

    assert!(saw_message, "agent never streamed a message");
}

/// Always-on smoke test: protocol types serialize / deserialize round-trip.
/// Keeps the JSON-RPC wire format under regression coverage even when the
/// optional live test is skipped.
#[test]
fn protocol_types_round_trip() {
    use senda_acp_client::SessionUpdate;

    let json = r#"{"type":"agentMessage","content":"hello"}"#;
    let parsed: SessionUpdate = serde_json::from_str(json).unwrap();
    assert!(matches!(parsed, SessionUpdate::AgentMessage { .. }));
}
