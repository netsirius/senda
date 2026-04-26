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
    /// Optional prompt sent to the agent when this automation fires. When
    /// empty, the trigger's natural payload is used (webhook body for
    /// webhooks; a placeholder for cron / manual).
    ///
    /// Substitutions: `{event}` is replaced with the raw trigger payload;
    /// `{{KEY}}` is replaced with the matching `variables` entry; both
    /// substitutions also apply to subsequent agents in the `chain`.
    #[serde(default)]
    pub prompt_template: Option<String>,
    /// Per-automation variables substituted into the prompt as `{{KEY}}`.
    /// Useful for company-specific values like project keys or team ids
    /// that change between deployments without touching the agent body.
    #[serde(default)]
    pub variables: std::collections::BTreeMap<String, String>,
    /// When true, the previous successful run's output is prepended to
    /// every fire's prompt. Lets a cron see what it did last time without
    /// needing an external store.
    #[serde(default)]
    pub include_last_output: bool,
    /// Subsequent agent ids to fire after the primary agent succeeds.
    /// Each step receives the previous step's output as its trigger payload
    /// (so `{event}` resolves to the prior agent's stdout).
    #[serde(default)]
    pub chain: Vec<String>,
    pub last_run_at: Option<i64>,
    pub last_run_status: Option<String>,
    pub next_run_at: Option<i64>,
}
