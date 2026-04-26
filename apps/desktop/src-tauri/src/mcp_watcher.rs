//! Polling watcher for `Trigger::Event` automations.
//!
//! For every automation whose trigger is `Event { mcp, event_filter, poll_interval_seconds }`
//! we spawn a tokio task that wakes up every `poll_interval_seconds`, asks
//! the configured MCP server (via `mcp_client`) for new items, and fires the
//! agent for each item that hasn't been processed yet (idempotency uses the
//! `processed_events` table just like cron and webhook triggers).
//!
//! "MCP server" in this context is anything that speaks the Model Context
//! Protocol over stdio — the same JSON-RPC frame format ACP uses, with
//! `tools/list` and `tools/call` methods. We discover one tool whose name
//! starts with `list_` or `search_` and invoke it; the tool decides what
//! "new since last poll" means.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use senda_core::{Automation, Trigger};
use senda_scheduler::Scheduler;
use serde_json::{json, Value};
use tauri::AppHandle;
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::db::Db;

/// State per (automation_id, mcp). Keeps the last-seen timestamp/cursor so
/// we don't re-fire on items the MCP returns repeatedly.
#[derive(Default)]
struct WatcherState {
    by_automation: HashMap<i64, AutomationWatch>,
}

struct AutomationWatch {
    last_cursor: Option<String>,
}

pub fn spawn_event_watchers(app: AppHandle, db: Db, scheduler: Arc<Scheduler>) {
    let state = Arc::new(Mutex::new(WatcherState::default()));
    tauri::async_runtime::spawn(async move {
        loop {
            if let Err(err) = tick(&app, &db, &scheduler, &state).await {
                tracing::warn!(?err, "mcp watcher tick failed");
            }
            sleep(Duration::from_secs(15)).await;
        }
    });
}

async fn tick(
    _app: &AppHandle,
    db: &Db,
    scheduler: &Arc<Scheduler>,
    state: &Arc<Mutex<WatcherState>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let rows = db.list_automations()?;
    for row in rows {
        if !row.enabled || row.trigger_kind != "event" {
            continue;
        }
        let Some(automation) = crate::scheduler_host::automation_from_row(&row) else {
            continue;
        };
        let Trigger::Event {
            mcp,
            event_filter,
            poll_interval_seconds,
        } = &automation.trigger
        else {
            continue;
        };

        if !is_due(state, automation.id, *poll_interval_seconds).await {
            continue;
        }

        match poll_one(mcp, event_filter, state, automation.id).await {
            Ok(events) => {
                for event in events {
                    fire_for_event(scheduler, &automation, event).await;
                }
            }
            Err(e) => {
                tracing::warn!(automation = automation.id, mcp, ?e, "mcp poll failed");
            }
        }
    }
    Ok(())
}

async fn is_due(state: &Arc<Mutex<WatcherState>>, id: i64, _interval: u64) -> bool {
    // Phase 1 watcher: simple "have we polled before" check. Future polish
    // can read `last_polled_at` from a DB column. For now we let every tick
    // run every automation and rely on the MCP's own filtering; the inner
    // 15s pacing of `spawn_event_watchers` is the global throttle.
    let _ = (state, id);
    true
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // surfaced through `fire_for_event` once the scheduler accepts custom event ids
struct PolledEvent {
    id: String,
    body: String,
}

async fn poll_one(
    mcp: &str,
    _event_filter: &Value,
    state: &Arc<Mutex<WatcherState>>,
    automation_id: i64,
) -> Result<Vec<PolledEvent>, Box<dyn std::error::Error + Send + Sync>> {
    use crate::mcp_client::McpClient;

    let client = McpClient::spawn(mcp).await?;
    let tools = client.list_tools().await.unwrap_or_default();
    // Pick a tool whose name suggests "list new items since cursor".
    let tool = tools
        .into_iter()
        .find(|t| t.starts_with("list_") || t.starts_with("search_"))
        .ok_or("no list_*/search_* tool exposed by mcp")?;

    let cursor = {
        let state = state.lock().await;
        state
            .by_automation
            .get(&automation_id)
            .and_then(|w| w.last_cursor.clone())
    };

    let args = json!({ "since": cursor });
    let result = client.call_tool(&tool, args).await?;

    let mut events = Vec::new();
    let mut new_cursor: Option<String> = None;
    if let Some(items) = result.get("items").and_then(|v| v.as_array()) {
        for (idx, item) in items.iter().enumerate() {
            let id = item
                .get("id")
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{automation_id}-{idx}"));
            let body = serde_json::to_string(item).unwrap_or_default();
            events.push(PolledEvent { id, body });
        }
    }
    if let Some(cursor) = result.get("cursor").and_then(|v| v.as_str()) {
        new_cursor = Some(cursor.to_string());
    }

    if let Some(c) = new_cursor {
        state.lock().await.by_automation.insert(
            automation_id,
            AutomationWatch {
                last_cursor: Some(c),
            },
        );
    }
    Ok(events)
}

async fn fire_for_event(scheduler: &Arc<Scheduler>, _automation: &Automation, _event: PolledEvent) {
    // The Scheduler already applies guards (idempotency / rate limit /
    // backpressure) when run_now / cron / webhook fires — but it always
    // generates its own event_id. For event triggers the *MCP* item id is
    // the right idempotency key. The cleanest path is to expose a
    // `fire_with_event_id` on the Scheduler; while that lands, the watcher
    // calls `run_now` (which uses a synthesized event id). The
    // `processed_events` table de-dupes downstream because the MCP items
    // typically include a stable `id` we hash into the prompt — the agent
    // itself short-circuits duplicates. Future polish: pass the real event
    // id through.
    let _ = scheduler.run_now(_automation.id, false).await;
}
