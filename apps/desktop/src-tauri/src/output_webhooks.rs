//! Fire-and-forget HTTP POSTs to user-configured endpoints after a run
//! completes. Senda auto-detects the body shape from the URL host:
//!
//! - Microsoft Teams (Office 365 / Workflow incoming webhooks):
//!   Adaptive Card payload posted to `*.webhook.office.com` /
//!   `*.teams.microsoft.com`.
//! - Slack incoming webhooks (`hooks.slack.com/services/...`):
//!   `{ "text": "<mrkdwn>" }`.
//! - Discord webhooks (`discord.com/api/webhooks/...`):
//!   `{ "content": "..." }`.
//! - Anything else: raw JSON `{ automationId, runId, status, output, error }`.
//!
//! Failures are logged but never bubble — a webhook that's down shouldn't
//! prevent the run from being recorded as successful.

use senda_core::OutputWebhook;
use serde_json::json;

const TRUNCATE: usize = 4_000;

#[derive(Debug, Clone)]
pub struct RunSummary {
    pub automation_id: i64,
    pub automation_name: String,
    pub agent_id: String,
    pub run_id: i64,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
}

pub async fn dispatch(webhooks: &[OutputWebhook], summary: &RunSummary) {
    if webhooks.is_empty() {
        return;
    }
    let client = reqwest::Client::new();
    for hook in webhooks {
        if !should_fire(hook, summary.success) {
            continue;
        }
        let body = format_body(hook, summary);
        match client.post(&hook.url).json(&body).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    tracing::warn!(
                        url = %hook.url,
                        status = resp.status().as_u16(),
                        "output webhook returned non-2xx"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(url = %hook.url, ?e, "output webhook delivery failed");
            }
        }
    }
}

fn should_fire(hook: &OutputWebhook, success: bool) -> bool {
    match hook.when.as_str() {
        "success" => success,
        "failure" => !success,
        _ => true,
    }
}

fn format_body(hook: &OutputWebhook, summary: &RunSummary) -> serde_json::Value {
    let format = if hook.format == "auto" {
        detect_format(&hook.url)
    } else {
        hook.format.as_str()
    };
    let truncated = truncate(&summary.output, TRUNCATE);
    let title = format!(
        "{} {}",
        if summary.success { "✅" } else { "❌" },
        summary.automation_name,
    );

    match format {
        "teams" => teams_card(&title, summary, &truncated),
        "slack" => json!({ "text": slack_text(&title, summary, &truncated) }),
        "discord" => json!({ "content": slack_text(&title, summary, &truncated) }),
        _ => json!({
            "automationId": summary.automation_id,
            "automationName": summary.automation_name,
            "agentId": summary.agent_id,
            "runId": summary.run_id,
            "status": if summary.success { "success" } else { "failed" },
            "output": truncated,
            "error": summary.error,
        }),
    }
}

fn detect_format(url: &str) -> &'static str {
    if url.contains("webhook.office.com") || url.contains("teams.microsoft.com") {
        "teams"
    } else if url.contains("hooks.slack.com") {
        "slack"
    } else if url.contains("discord.com/api/webhooks")
        || url.contains("discordapp.com/api/webhooks")
    {
        "discord"
    } else {
        "raw"
    }
}

fn slack_text(title: &str, summary: &RunSummary, output: &str) -> String {
    let mut buf = format!("*{title}*\n");
    buf.push_str(&format!("Agent: `{}`\n", summary.agent_id));
    if let Some(err) = &summary.error {
        buf.push_str(&format!("Error: ```{err}```\n"));
    }
    if !output.trim().is_empty() {
        buf.push_str(&format!("```{output}```"));
    }
    buf
}

/// Microsoft Teams Adaptive Card 1.4. Office 365 connector and Workflow
/// incoming webhooks both accept this shape.
fn teams_card(title: &str, summary: &RunSummary, output: &str) -> serde_json::Value {
    let mut body = vec![
        json!({
            "type": "TextBlock",
            "text": title,
            "weight": "Bolder",
            "size": "Medium",
        }),
        json!({
            "type": "FactSet",
            "facts": [
                { "title": "Agent", "value": summary.agent_id },
                {
                    "title": "Status",
                    "value": if summary.success { "success" } else { "failed" },
                },
            ],
        }),
    ];
    if let Some(err) = &summary.error {
        body.push(json!({
            "type": "TextBlock",
            "text": format!("**Error:** {err}"),
            "wrap": true,
            "color": "Attention",
        }));
    }
    if !output.trim().is_empty() {
        body.push(json!({
            "type": "TextBlock",
            "text": output,
            "wrap": true,
            "isSubtle": true,
            "fontType": "Monospace",
        }));
    }
    json!({
        "type": "message",
        "attachments": [{
            "contentType": "application/vnd.microsoft.card.adaptive",
            "content": {
                "$schema": "http://adaptivecards.io/schemas/adaptive-card.json",
                "type": "AdaptiveCard",
                "version": "1.4",
                "body": body,
            }
        }]
    })
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let head: String = s.chars().take(n).collect();
    format!("{head}\n…(truncated, {} chars total)", s.chars().count())
}
