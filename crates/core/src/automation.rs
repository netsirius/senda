use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Trigger {
    Schedule {
        cron: String,
        timezone: String,
    },
    Event {
        mcp: String,
        event_filter: serde_json::Value,
        poll_interval_seconds: u64,
    },
    Webhook {
        path: String,
        secret: Option<String>,
    },
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum BackpressurePolicy {
    Skip,
    Queue,
    Concurrent,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Guards {
    pub idempotency: bool,
    pub rate_limit_per_hour: u32,
    pub approval_gate: bool,
    pub backpressure: BackpressurePolicy,
}

impl Default for Guards {
    fn default() -> Self {
        Self {
            idempotency: true,
            rate_limit_per_hour: 100,
            approval_gate: false,
            backpressure: BackpressurePolicy::Skip,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Automation {
    pub id: i64,
    pub name: String,
    pub agent_id: String,
    pub trigger: Trigger,
    pub guards: Guards,
    pub enabled: bool,
    pub last_run_at: Option<i64>,
    pub last_run_status: Option<String>,
    pub next_run_at: Option<i64>,
}
