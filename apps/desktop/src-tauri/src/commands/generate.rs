//! Generate a canonical agent document from a natural-language prompt by
//! routing through the user's primary CLI. This sits behind a single Tauri
//! command so the frontend doesn't need to know how each CLI is invoked.

use std::time::Duration;

use senda_core::AgentCli;
use serde::{Deserialize, Serialize};

use crate::agent_runtime::spawn_for_automation;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GenerateArgs {
    pub primary_cli: AgentCli,
    pub user_intent: String,
    /// Targets the generated agent should declare. Defaults to `[primary_cli]`
    /// when empty.
    #[serde(default)]
    pub targets: Vec<AgentCli>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GenerateResult {
    /// Full canonical document the user can paste into the editor.
    pub canonical_source: String,
    /// Raw CLI output (for debugging when extraction fails).
    pub raw_output: String,
}

#[tauri::command]
pub async fn generate_agent(args: GenerateArgs) -> Result<GenerateResult, String> {
    let targets = if args.targets.is_empty() {
        vec![args.primary_cli]
    } else {
        args.targets
    };
    let target_list = targets
        .iter()
        .map(|c| c.as_id())
        .collect::<Vec<_>>()
        .join(", ");

    let prompt = build_meta_prompt(&args.user_intent, &target_list);

    // We use `spawn_for_automation` because it gives us captured stdout, no
    // PTY noise, and a wall-clock timeout. The "agent" we invoke here is the
    // CLI itself talking without a custom agent — `--prompt` carries our
    // meta-instruction in full.
    let result = spawn_for_automation(
        args.primary_cli,
        // Pass an empty agent name so the CLI runs against its default
        // assistant (not a specialized agent) — we want a generic completion.
        "",
        &prompt,
        false,
        Duration::from_secs(120),
    )
    .await;

    if let Some(err) = result.error {
        return Err(err);
    }
    if !result.success {
        return Err(format!(
            "{} exited non-zero. Output:\n{}",
            args.primary_cli.as_id(),
            result.output
        ));
    }

    let canonical_source =
        extract_canonical(&result.output).unwrap_or_else(|| result.output.clone());
    Ok(GenerateResult {
        canonical_source,
        raw_output: result.output,
    })
}

fn build_meta_prompt(intent: &str, targets: &str) -> String {
    format!(
        r#"Create a Senda canonical agent document for the following intent:

INTENT:
{intent}

REQUIREMENTS:
- Output a single fenced code block tagged ```yaml-md``` containing the full canonical document.
- The frontmatter MUST include: name (kebab-case), description, targets: [{targets}], tools (array).
- The body after the closing --- fence is the agent's system prompt.
- Do NOT include explanation outside the fenced block.

EXAMPLE OUTPUT FORMAT:
```yaml-md
---
name: example-agent
description: A short description.
targets: [{targets}]
tools: []
---

You are an example agent. Body here.
```

Now produce the canonical document for the intent above."#
    )
}

fn extract_canonical(output: &str) -> Option<String> {
    // Look for ```yaml-md ... ``` first; fall back to any fenced block.
    for fence in ["```yaml-md", "```markdown", "```md", "```yaml", "```"] {
        if let Some(start) = output.find(fence) {
            let after = &output[start + fence.len()..];
            let after = after.trim_start_matches('\n');
            if let Some(end) = after.find("```") {
                let block = &after[..end];
                if block.trim_start().starts_with("---") {
                    return Some(block.trim_end().to_string());
                }
            }
        }
    }
    // No fenced block — accept the raw output if it already starts with ---.
    if output.trim_start().starts_with("---") {
        return Some(output.trim_end().to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_yaml_md_fenced_block() {
        let raw = "Sure!\n```yaml-md\n---\nname: x\ndescription: y\ntargets: [copilot]\n---\nbody\n```\nDone.";
        let extracted = extract_canonical(raw).unwrap();
        assert!(extracted.starts_with("---"));
        assert!(extracted.contains("name: x"));
    }

    #[test]
    fn falls_back_to_raw_when_no_fence() {
        let raw = "---\nname: x\ndescription: y\ntargets: [copilot]\n---\nbody\n";
        let extracted = extract_canonical(raw).unwrap();
        assert!(extracted.starts_with("---"));
    }

    #[test]
    fn returns_none_on_unrecognized_output() {
        assert_eq!(extract_canonical("totally unrelated"), None);
    }
}
