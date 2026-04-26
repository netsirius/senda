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

/// Where to POST the result of a successful (or failed) run. Senda
/// auto-detects the payload shape from the URL host:
/// - teams.microsoft.com / *.webhook.office.com → Adaptive Card
/// - hooks.slack.com → `{ "text": ... }` mrkdwn
/// - discord.com/api/webhooks → `{ "content": ... }`
/// - everything else → raw JSON `{ automationId, runId, status, output, error }`
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OutputWebhook {
    pub url: String,
    /// Optional override; defaults to `auto` (URL host detection).
    #[serde(default = "default_format")]
    pub format: String,
    /// Send only on failure / only on success / always. Default: always.
    #[serde(default = "default_when")]
    pub when: String,
}

fn default_format() -> String {
    "auto".to_string()
}
fn default_when() -> String {
    "always".to_string()
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
    /// HTTP endpoints that get a POST with the run's outcome after each
    /// successful (or failed, depending on `when`) firing. Useful for
    /// pinging Teams, Slack, Discord, or your own service.
    #[serde(default)]
    pub output_webhooks: Vec<OutputWebhook>,
    pub last_run_at: Option<i64>,
    pub last_run_status: Option<String>,
    pub next_run_at: Option<i64>,
}
