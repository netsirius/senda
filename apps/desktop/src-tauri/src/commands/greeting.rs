use serde::Serialize;

/// Phase 0 IPC smoke test — proves the JS↔Rust bridge is alive. Phase 1
/// replaces this with `read_catalog` and friends.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Greeting {
    pub agent_name: String,
    pub agent_version: String,
}

#[tauri::command]
pub fn hello_world(name: String) -> Greeting {
    tracing::info!(target: "senda.ipc", name = %name, "hello_world invoked");
    Greeting {
        agent_name: format!("senda → {name}"),
        agent_version: env!("CARGO_PKG_VERSION").to_string(),
    }
}
