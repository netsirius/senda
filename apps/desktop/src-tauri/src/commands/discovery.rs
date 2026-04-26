//! Discovery commands — what MCP servers, tools, and skills are present on
//! the user's machine. The editor uses these to feed autocomplete and the
//! Skills page to render its list.
//!
//! `add_mcp` / `delete_mcp` write back into the CLI's own config file. Senda
//! only owns the entry it adds — it never rewrites unrelated keys, and the
//! file is read → mutated → written so other entries survive.

use std::path::{Path, PathBuf};

use senda_core::AgentCli;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InstalledMcp {
    /// CLI whose config declares this MCP.
    pub cli: AgentCli,
    pub name: String,
    /// `local` or `remote`. Empty when the CLI's config doesn't expose the
    /// distinction.
    pub r#type: String,
    pub command: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CliTools {
    pub cli: AgentCli,
    pub tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkillEntry {
    pub cli: AgentCli,
    pub name: String,
    pub path: String,
    pub description: Option<String>,
}

#[tauri::command]
pub async fn list_installed_mcps() -> Result<Vec<InstalledMcp>, String> {
    let home = dirs::home_dir().ok_or_else(|| "no home".to_string())?;
    let mut out = Vec::new();
    out.extend(read_copilot_mcps(&home));
    out.extend(read_claude_mcps(&home));
    out.extend(read_gemini_mcps(&home));
    Ok(out)
}

#[tauri::command]
pub async fn list_builtin_tools() -> Result<Vec<CliTools>, String> {
    Ok(vec![
        CliTools {
            cli: AgentCli::Copilot,
            tools: COPILOT_TOOLS.iter().map(|s| s.to_string()).collect(),
        },
        CliTools {
            cli: AgentCli::ClaudeCode,
            tools: CLAUDE_TOOLS.iter().map(|s| s.to_string()).collect(),
        },
        CliTools {
            cli: AgentCli::Gemini,
            tools: GEMINI_TOOLS.iter().map(|s| s.to_string()).collect(),
        },
    ])
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AddMcpArgs {
    pub cli: AgentCli,
    pub name: String,
    /// `local` (default) or `remote`. Remote MCPs use `url`; local ones use
    /// `command` + optional `args` + optional `env`.
    #[serde(default = "default_type")]
    pub r#type: String,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    pub url: Option<String>,
    #[serde(default)]
    pub env: std::collections::BTreeMap<String, String>,
}

fn default_type() -> String {
    "local".to_string()
}

#[tauri::command]
pub async fn add_mcp(args: AddMcpArgs) -> Result<(), String> {
    let home = dirs::home_dir().ok_or_else(|| "no home".to_string())?;
    let path = mcp_config_path(args.cli, &home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }

    let mut root = read_or_default(&path);
    // Build the entry. We always write `command` for local MCPs and `url`
    // for remote ones — older Copilot / Gemini configs accept extra keys.
    let mut entry = Map::new();
    entry.insert("type".into(), Value::String(args.r#type.clone()));
    if let Some(command) = args.command {
        entry.insert("command".into(), Value::String(command));
    }
    if !args.args.is_empty() {
        entry.insert(
            "args".into(),
            Value::Array(args.args.into_iter().map(Value::String).collect()),
        );
    }
    if let Some(url) = args.url {
        entry.insert("url".into(), Value::String(url));
    }
    if !args.env.is_empty() {
        let mut env_map = Map::new();
        for (k, v) in args.env {
            env_map.insert(k, Value::String(v));
        }
        entry.insert("env".into(), Value::Object(env_map));
    }

    upsert_mcp_entry(&mut root, &args.name, Value::Object(entry));
    write_pretty(&path, &root)
}

#[tauri::command]
pub async fn delete_mcp(cli: AgentCli, name: String) -> Result<(), String> {
    let home = dirs::home_dir().ok_or_else(|| "no home".to_string())?;
    let path = mcp_config_path(cli, &home);
    if !path.exists() {
        return Ok(());
    }
    let mut root = read_or_default(&path);
    if let Some(servers) = root
        .as_object_mut()
        .and_then(|m| m.get_mut("mcpServers"))
        .and_then(|v| v.as_object_mut())
    {
        servers.remove(&name);
    }
    write_pretty(&path, &root)
}

#[tauri::command]
pub async fn delete_skill(cli: AgentCli, name: String) -> Result<(), String> {
    if !matches!(cli, AgentCli::ClaudeCode) {
        return Err("only Claude Code skills are managed by Senda today".into());
    }
    let home = dirs::home_dir().ok_or_else(|| "no home".to_string())?;
    let dir = home.join(".claude").join("skills").join(&name);
    if !dir.exists() {
        return Ok(());
    }
    if !dir.is_dir() {
        return Err(format!("not a directory: {}", dir.display()));
    }
    std::fs::remove_dir_all(&dir).map_err(|e| format!("delete: {e}"))
}

#[tauri::command]
pub async fn list_skills() -> Result<Vec<SkillEntry>, String> {
    let home = dirs::home_dir().ok_or_else(|| "no home".to_string())?;
    let mut out = Vec::new();

    // Claude Code stores skills as folders under `~/.claude/skills/<name>/SKILL.md`.
    let claude_skills = home.join(".claude").join("skills");
    if claude_skills.is_dir() {
        if let Ok(read) = std::fs::read_dir(&claude_skills) {
            for entry in read.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                let skill_md = path.join("SKILL.md");
                let description = std::fs::read_to_string(&skill_md)
                    .ok()
                    .and_then(extract_description);
                out.push(SkillEntry {
                    cli: AgentCli::ClaudeCode,
                    name,
                    path: path.to_string_lossy().to_string(),
                    description,
                });
            }
        }
    }

    Ok(out)
}

// ── shared helpers for add/delete ───────────────────────────────────────────

fn mcp_config_path(cli: AgentCli, home: &Path) -> PathBuf {
    match cli {
        AgentCli::Copilot => home.join(".copilot").join("mcp-servers.json"),
        AgentCli::ClaudeCode => {
            // Prefer ~/.claude/settings.json since it's the documented user-scope
            // file; fall back to ~/.claude.json if that one already exists.
            let settings = home.join(".claude").join("settings.json");
            let claude_json = home.join(".claude.json");
            if claude_json.exists() && !settings.exists() {
                claude_json
            } else {
                settings
            }
        }
        AgentCli::Gemini => home.join(".gemini").join("settings.json"),
    }
}

fn read_or_default(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({}))
}

fn upsert_mcp_entry(root: &mut Value, name: &str, entry: Value) {
    let obj = match root.as_object_mut() {
        Some(o) => o,
        None => {
            *root = json!({});
            root.as_object_mut().expect("just initialized")
        }
    };
    let servers = obj
        .entry("mcpServers".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(map) = servers.as_object_mut() {
        map.insert(name.to_string(), entry);
    }
}

fn write_pretty(path: &Path, value: &Value) -> Result<(), String> {
    let serialized = serde_json::to_string_pretty(value).map_err(|e| format!("encode: {e}"))?;
    std::fs::write(path, serialized).map_err(|e| format!("write {}: {e}", path.display()))
}

// ── per-CLI MCP readers ─────────────────────────────────────────────────────

fn read_copilot_mcps(home: &Path) -> Vec<InstalledMcp> {
    // Copilot CLI stores MCP servers under ~/.copilot/mcp-servers.json
    let path = home.join(".copilot").join("mcp-servers.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(servers) = value.get("mcpServers").and_then(|v| v.as_object()) {
        for (name, spec) in servers {
            out.push(InstalledMcp {
                cli: AgentCli::Copilot,
                name: name.clone(),
                r#type: spec
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("local")
                    .to_string(),
                command: spec
                    .get("command")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                url: spec.get("url").and_then(|v| v.as_str()).map(str::to_string),
            });
        }
    }
    out
}

fn read_claude_mcps(home: &Path) -> Vec<InstalledMcp> {
    // Claude Code stores user-scope MCPs under ~/.claude.json or
    // ~/.claude/mcp_servers.json depending on version.
    let candidates = [
        home.join(".claude.json"),
        home.join(".claude").join("mcp_servers.json"),
        home.join(".claude").join("settings.json"),
    ];
    for path in candidates {
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
            continue;
        };
        let servers = value
            .get("mcpServers")
            .or_else(|| value.get("mcp_servers"))
            .and_then(|v| v.as_object());
        if let Some(servers) = servers {
            return servers
                .iter()
                .map(|(name, spec)| InstalledMcp {
                    cli: AgentCli::ClaudeCode,
                    name: name.clone(),
                    r#type: spec
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("local")
                        .to_string(),
                    command: spec
                        .get("command")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    url: spec.get("url").and_then(|v| v.as_str()).map(str::to_string),
                })
                .collect();
        }
    }
    Vec::new()
}

fn read_gemini_mcps(home: &Path) -> Vec<InstalledMcp> {
    // Gemini CLI uses ~/.gemini/settings.json with an `mcpServers` block.
    let path = home.join(".gemini").join("settings.json");
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Some(servers) = value.get("mcpServers").and_then(|v| v.as_object()) {
        for (name, spec) in servers {
            out.push(InstalledMcp {
                cli: AgentCli::Gemini,
                name: name.clone(),
                r#type: spec
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("local")
                    .to_string(),
                command: spec
                    .get("command")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                url: spec.get("url").and_then(|v| v.as_str()).map(str::to_string),
            });
        }
    }
    out
}

fn extract_description(skill_md: String) -> Option<String> {
    // SKILL.md frontmatter looks like agents'; reuse the same parser.
    let parsed = senda_agent_parser::parse_canonical(&skill_md).ok()?;
    Some(parsed.description)
}

// ── CLI built-in tools ──────────────────────────────────────────────────────

const COPILOT_TOOLS: &[&str] = &[
    "read_file",
    "write_file",
    "edit_file",
    "list_files",
    "search_files",
    "run_shell",
    "fetch_url",
];

const CLAUDE_TOOLS: &[&str] = &[
    "Bash",
    "Edit",
    "Glob",
    "Grep",
    "Read",
    "TodoWrite",
    "WebFetch",
    "WebSearch",
    "Write",
];

const GEMINI_TOOLS: &[&str] = &[
    "read_file",
    "write_file",
    "list_directory",
    "search_files",
    "run_shell_command",
    "google_search",
];
