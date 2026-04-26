# senda-acp-client

Minimal async Rust client for the **Agent Client Protocol** (ACP) over JSON-RPC stdio.

ACP is the open protocol agent CLIs like Claude Code and Gemini CLI use to expose
their agents to external clients. This crate spawns such an agent as a child
process, owns its stdio, and gives you a [`AcpSession`] you can prompt while
streaming updates back through a Tokio channel.

> Designed to be published to crates.io. Senda depends on it via path during
> development; downstream users can pin a published version.

## Usage

```rust,no_run
use senda_acp_client::spawn_agent;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let session = spawn_agent("claude-code-acp", &[]).await?;
    let session_id = session.new_session().await?;
    let mut updates = session.prompt(&session_id, "What is 2 + 2?").await?;
    while let Some(update) = updates.recv().await {
        println!("{update:?}");
    }
    Ok(())
}
```

## Status

POC. Implements: `initialize`, `session/new`, `session/prompt`,
`session/cancel`. Update notifications are forwarded as
[`SessionUpdate`] enum values through an `mpsc::Receiver`.

## License

MIT — Hector Santos &lt;netsirius@gmail.com&gt;
