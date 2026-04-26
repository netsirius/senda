//! Read pre-existing **native** agent files (one of the three CLIs) and
//! normalize them into a [`CanonicalAgent`].
//!
//! These agents live in `~/.copilot/agents/`, `~/.claude/agents/` and
//! `~/.gemini/agents/`. We don't own them — they may have been authored
//! directly with their CLI's tooling — so the parsers are forgiving:
//! anything we don't understand goes into `cli_specific.<cli>` and the
//! rest of the canonical struct keeps reasonable defaults.

use std::path::Path;

use senda_core::{
    AgentCli, CanonicalAgent, ClaudeCodeSpecific, CliSpecific, CopilotSpecific, GeminiSpecific,
    McpServerSpec,
};

use crate::canonical::ParseError;

/// Result of parsing a native CLI agent file: the canonical view plus the
/// CLI it was read from (used by callers to badge external agents).
#[derive(Debug, Clone)]
pub struct NativeAgent {
    pub agent: CanonicalAgent,
    pub origin: AgentCli,
}

/// Inspect `path` and route to the matching native parser. Returns `None` if
/// the path is not under any of the three known native folders.
pub fn detect_cli(path: &Path) -> Option<AgentCli> {
    // Look at the *parent* path components for a folder named "agents" whose
    // grandparent is one of the three CLI roots. That handles both real
    // `$HOME/.copilot/agents/foo.agent.md` paths and tmpdir-based test paths
    // like `/tmp/x/.copilot/agents/foo.agent.md`.
    let mut comps: Vec<_> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();
    // We're interested in [.. , ".copilot" | ".claude" | ".gemini", "agents", file].
    if comps.len() < 3 {
        return None;
    }
    comps.pop(); // file
    let agents = comps.pop()?;
    if agents != "agents" {
        return None;
    }
    let cli_root = comps.pop()?;
    match cli_root {
        ".copilot" => Some(AgentCli::Copilot),
        ".claude" => Some(AgentCli::ClaudeCode),
        ".gemini" => Some(AgentCli::Gemini),
        _ => None,
    }
}

/// Read a file from disk and parse it as the matching CLI's native format.
pub fn parse_from_file(path: &Path) -> Result<NativeAgent, ParseError> {
    let cli = detect_cli(path).ok_or(ParseError::UnknownCliFolder)?;
    let raw = std::fs::read_to_string(path).map_err(ParseError::Io)?;
    parse_native(&raw, cli)
}

/// Same as [`parse_from_file`], but takes the contents and the originating
/// CLI directly. Used by tests and by anything that already holds the bytes.
pub fn parse_native(raw: &str, cli: AgentCli) -> Result<NativeAgent, ParseError> {
    let agent = match cli {
        AgentCli::Copilot => parse_copilot(raw)?,
        AgentCli::ClaudeCode => parse_claude_code(raw)?,
        AgentCli::Gemini => parse_gemini(raw)?,
    };
    Ok(NativeAgent { agent, origin: cli })
}

// ── Frontmatter helper ────────────────────────────────────────────────────────

/// Split a Markdown document into `(yaml_frontmatter, body)`. Both `Copilot`
/// and `Claude Code` agents use this layout — the per-CLI parser maps the
/// fields it cares about and dumps the rest into `cli_specific`.
fn split_frontmatter(input: &str) -> Result<(serde_yaml::Value, String), ParseError> {
    let mut lines = input.lines();
    match lines.next() {
        Some(first) if first.trim_end() == "---" => {}
        _ => return Err(ParseError::MissingOpeningFence),
    }
    let mut fm = String::new();
    let mut body_lines: Vec<&str> = Vec::new();
    let mut closed = false;
    for line in lines {
        if !closed {
            if line.trim_end() == "---" {
                closed = true;
                continue;
            }
            fm.push_str(line);
            fm.push('\n');
        } else {
            body_lines.push(line);
        }
    }
    if !closed {
        return Err(ParseError::MissingClosingFence);
    }
    let value: serde_yaml::Value = serde_yaml::from_str(&fm)?;
    Ok((
        value,
        body_lines.join("\n").trim_start_matches('\n').to_string(),
    ))
}

fn yaml_string(v: &serde_yaml::Value) -> Option<String> {
    v.as_str().map(|s| s.to_string())
}

fn yaml_string_array(v: &serde_yaml::Value) -> Vec<String> {
    v.as_sequence()
        .map(|seq| seq.iter().filter_map(yaml_string).collect())
        .unwrap_or_default()
}

// ── Copilot ──────────────────────────────────────────────────────────────────

fn parse_copilot(raw: &str) -> Result<CanonicalAgent, ParseError> {
    let (mut fm, body) = split_frontmatter(raw)?;
    let map = fm.as_mapping_mut().ok_or(ParseError::FrontmatterNotMap)?;

    let mut take = |k: &str| -> Option<serde_yaml::Value> {
        map.remove(serde_yaml::Value::String(k.to_string()))
    };

    let name = take("name")
        .as_ref()
        .and_then(yaml_string)
        .ok_or(ParseError::MissingField("name"))?;
    let description = take("description")
        .as_ref()
        .and_then(yaml_string)
        .unwrap_or_default();
    let tools = take("tools")
        .as_ref()
        .map(yaml_string_array)
        .unwrap_or_default();
    let mcp = take("mcp-servers")
        .map(parse_mcp_servers_canonical)
        .unwrap_or_default();
    let target = take("target").as_ref().and_then(yaml_string);

    Ok(CanonicalAgent {
        name,
        description,
        targets: vec![AgentCli::Copilot],
        tools,
        mcp_servers: mcp,
        cli_specific: CliSpecific {
            copilot: Some(CopilotSpecific { target }),
            ..Default::default()
        },
        body,
    })
}

// ── Claude Code ──────────────────────────────────────────────────────────────

fn parse_claude_code(raw: &str) -> Result<CanonicalAgent, ParseError> {
    let (mut fm, body) = split_frontmatter(raw)?;
    let map = fm.as_mapping_mut().ok_or(ParseError::FrontmatterNotMap)?;

    let mut take = |k: &str| -> Option<serde_yaml::Value> {
        map.remove(serde_yaml::Value::String(k.to_string()))
    };

    let name = take("name")
        .as_ref()
        .and_then(yaml_string)
        .ok_or(ParseError::MissingField("name"))?;
    let description = take("description")
        .as_ref()
        .and_then(yaml_string)
        .unwrap_or_default();
    let tools = take("tools")
        .as_ref()
        .map(yaml_string_array)
        .unwrap_or_default();
    // Claude Code uses `mcpServers` in camelCase.
    let mcp = take("mcpServers")
        .map(parse_mcp_servers_canonical)
        .unwrap_or_default();
    let permission_mode = take("permissionMode").as_ref().and_then(yaml_string);
    let hooks = take("hooks").map(|v| {
        let mut out = std::collections::BTreeMap::new();
        if let Some(map) = v.as_mapping() {
            for (k, val) in map {
                if let (Some(k), Some(val)) = (k.as_str(), val.as_str()) {
                    out.insert(k.to_string(), val.to_string());
                }
            }
        }
        out
    });

    Ok(CanonicalAgent {
        name,
        description,
        targets: vec![AgentCli::ClaudeCode],
        tools,
        mcp_servers: mcp,
        cli_specific: CliSpecific {
            claude_code: Some(ClaudeCodeSpecific {
                permission_mode,
                hooks,
            }),
            ..Default::default()
        },
        body,
    })
}

// ── Gemini ───────────────────────────────────────────────────────────────────

fn parse_gemini(raw: &str) -> Result<CanonicalAgent, ParseError> {
    let value: toml::Value = toml::from_str(raw).map_err(|e| ParseError::Toml(e.to_string()))?;
    let agent_table = value
        .get("agent")
        .and_then(|v| v.as_table())
        .ok_or(ParseError::MissingField("agent"))?;

    let name = agent_table
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(ParseError::MissingField("agent.name"))?
        .to_string();
    let description = agent_table
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let tools = agent_table
        .get("allowedTools")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let body = agent_table
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let model = agent_table
        .get("model")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Ok(CanonicalAgent {
        name,
        description,
        targets: vec![AgentCli::Gemini],
        tools,
        mcp_servers: Default::default(),
        cli_specific: CliSpecific {
            gemini: Some(GeminiSpecific { model }),
            ..Default::default()
        },
        body,
    })
}

// ── MCP server normalization ─────────────────────────────────────────────────

fn parse_mcp_servers_canonical(
    value: serde_yaml::Value,
) -> std::collections::BTreeMap<String, McpServerSpec> {
    let Some(map) = value.as_mapping() else {
        return Default::default();
    };
    let mut out = std::collections::BTreeMap::new();
    for (key, val) in map {
        let Some(name) = key.as_str() else { continue };
        // Re-serialize / deserialize so we get the same forgiving behavior as
        // CanonicalAgent's own MCP parsing — unknown keys are simply ignored.
        if let Ok(spec) = serde_yaml::from_value::<McpServerSpec>(val.clone()) {
            out.insert(name.to_string(), spec);
        }
    }
    out
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::path::PathBuf;

    #[test]
    fn detects_copilot_folder() {
        let p = PathBuf::from("/Users/h/.copilot/agents/triage.agent.md");
        assert_eq!(detect_cli(&p), Some(AgentCli::Copilot));
    }

    #[test]
    fn detects_claude_folder() {
        let p = PathBuf::from("/home/u/.claude/agents/foo.md");
        assert_eq!(detect_cli(&p), Some(AgentCli::ClaudeCode));
    }

    #[test]
    fn detects_gemini_folder() {
        let p = PathBuf::from("/x/.gemini/agents/bar.toml");
        assert_eq!(detect_cli(&p), Some(AgentCli::Gemini));
    }

    #[test]
    fn returns_none_for_unrelated_path() {
        let p = PathBuf::from("/tmp/random/foo.md");
        assert_eq!(detect_cli(&p), None);
    }

    #[test]
    fn parses_copilot_native_agent() {
        let raw = indoc! {"
            ---
            name: triage
            description: Triage tickets.
            tools: [read_file]
            target: github-copilot
            ---

            Body.
        "};
        let parsed = parse_native(raw, AgentCli::Copilot).unwrap();
        assert_eq!(parsed.agent.name, "triage");
        assert_eq!(parsed.agent.targets, vec![AgentCli::Copilot]);
        assert_eq!(parsed.agent.tools, vec!["read_file"]);
        assert_eq!(
            parsed
                .agent
                .cli_specific
                .copilot
                .as_ref()
                .and_then(|c| c.target.as_deref()),
            Some("github-copilot")
        );
    }

    #[test]
    fn parses_claude_native_agent() {
        let raw = indoc! {"
            ---
            name: pr-summarizer
            description: Summarize PRs.
            tools: [read_file, write_file]
            permissionMode: acceptEdits
            ---

            Body.
        "};
        let parsed = parse_native(raw, AgentCli::ClaudeCode).unwrap();
        assert_eq!(parsed.agent.targets, vec![AgentCli::ClaudeCode]);
        assert_eq!(
            parsed
                .agent
                .cli_specific
                .claude_code
                .as_ref()
                .and_then(|c| c.permission_mode.as_deref()),
            Some("acceptEdits")
        );
    }

    #[test]
    fn parses_gemini_native_agent() {
        let raw = indoc! {r#"
            [agent]
            name = "ticket-triage"
            description = "Triage tickets"
            allowedTools = ["read_file"]
            prompt = "Body content"
            model = "gemini-2.5-pro"
        "#};
        let parsed = parse_native(raw, AgentCli::Gemini).unwrap();
        assert_eq!(parsed.agent.name, "ticket-triage");
        assert_eq!(parsed.agent.targets, vec![AgentCli::Gemini]);
        assert_eq!(parsed.agent.body, "Body content");
        assert_eq!(
            parsed
                .agent
                .cli_specific
                .gemini
                .as_ref()
                .and_then(|g| g.model.as_deref()),
            Some("gemini-2.5-pro")
        );
    }

    #[test]
    fn missing_name_in_copilot_returns_typed_error() {
        let raw = "---\ndescription: x\n---\nbody\n";
        let err = parse_native(raw, AgentCli::Copilot).unwrap_err();
        assert!(matches!(err, ParseError::MissingField("name")));
    }
}
