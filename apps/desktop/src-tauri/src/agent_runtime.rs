//! Helpers shared between the user-facing PTY runner (`commands::execution`)
//! and the background automation runner (`scheduler_host`).
//!
//! The PTY path streams ANSI to xterm.js. The automation path captures
//! stdout/stderr to a string so the run can be persisted in
//! `automation_runs.output_text`. The argv layout is identical between the
//! two — kept here as the single source of truth.

use std::path::Path;
use std::time::Duration;

use senda_acp_client::{spawn_agent, SessionUpdate};
use senda_agent_parser::{parse_canonical, parse_native};
use senda_core::{AgentCli, AgentSource, CatalogEntry};
use tokio::process::Command;
use tokio::time::timeout;

use crate::db::Db;

/// Probe for an ACP-capable wrapper for the given CLI. Returns the binary
/// name to spawn, or `None` if not available — caller falls back to plain
/// subprocess. The names follow community conventions; a future Settings
/// override can replace these.
fn acp_binary_for(cli: AgentCli) -> Option<&'static str> {
    let candidate = match cli {
        AgentCli::ClaudeCode => "claude-code-acp",
        AgentCli::Gemini => "gemini-acp",
        AgentCli::Copilot => return None, // Copilot doesn't ship an ACP server.
    };
    if which::which(candidate).is_ok() {
        Some(candidate)
    } else {
        None
    }
}

/// Build the (program, args) pair that spawns a given agent on a given CLI.
/// Mirrors the layout used by [`crate::commands::execution::build_command`].
pub fn argv(cli: AgentCli, agent_name: &str, prompt: &str) -> (&'static str, Vec<String>) {
    match cli {
        AgentCli::Copilot => (
            "copilot",
            vec![
                format!("--agent={agent_name}"),
                format!("--prompt={prompt}"),
            ],
        ),
        AgentCli::ClaudeCode => (
            "claude",
            vec!["--agent".into(), agent_name.to_string(), prompt.to_string()],
        ),
        AgentCli::Gemini => (
            "gemini",
            vec![
                "--agent".into(),
                agent_name.to_string(),
                "--prompt".into(),
                prompt.to_string(),
            ],
        ),
    }
}

/// Outcome of a background spawn. Captured stdout+stderr go into `output`;
/// any spawn-time error (binary not in `$PATH`, etc.) goes into `error`.
#[derive(Debug, Clone)]
pub struct AutomationRun {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub output: String,
    pub error: Option<String>,
}

/// Spawn an agent without a PTY, capture its combined output, and wait up to
/// `wall_clock` for it to exit. Used by the scheduler — the user-facing
/// runner uses PTY streaming instead.
///
/// For Claude Code / Gemini, this prefers `claude-code-acp` /
/// `gemini-acp` when present in `$PATH` and talks the Agent Client
/// Protocol — output is the concatenation of every `agentMessage`
/// update. Falls back to plain subprocess if the ACP binary is missing.
pub async fn spawn_for_automation(
    cli: AgentCli,
    agent_name: &str,
    prompt: &str,
    dry_run: bool,
    wall_clock: Duration,
) -> AutomationRun {
    if let Some(acp_bin) = acp_binary_for(cli) {
        return spawn_via_acp(acp_bin, agent_name, prompt, dry_run, wall_clock).await;
    }

    let (program, args) = argv(cli, agent_name, prompt);
    let mut cmd = Command::new(program);
    cmd.args(&args);
    cmd.env("TERM", "xterm-256color");
    if dry_run {
        cmd.env("SENDA_DRY_RUN", "1");
    }

    match timeout(wall_clock, cmd.output()).await {
        Ok(Ok(out)) => {
            let mut combined = String::new();
            combined.push_str(&String::from_utf8_lossy(&out.stdout));
            if !out.stderr.is_empty() {
                combined.push_str("\n--- stderr ---\n");
                combined.push_str(&String::from_utf8_lossy(&out.stderr));
            }
            AutomationRun {
                success: out.status.success(),
                exit_code: out.status.code(),
                output: combined,
                error: None,
            }
        }
        Ok(Err(e)) => AutomationRun {
            success: false,
            exit_code: None,
            output: String::new(),
            error: Some(format!("spawn `{program}`: {e}")),
        },
        Err(_) => AutomationRun {
            success: false,
            exit_code: None,
            output: String::new(),
            error: Some(format!("timeout after {}s", wall_clock.as_secs())),
        },
    }
}

async fn spawn_via_acp(
    binary: &str,
    _agent_name: &str,
    prompt: &str,
    dry_run: bool,
    wall_clock: Duration,
) -> AutomationRun {
    // ACP doesn't carry a `dry_run` concept — agents that opt-in honour the
    // SENDA_DRY_RUN env, but the protocol itself can't tell the agent. We
    // export the env var via the spawned child's environment for parity
    // with the subprocess path, even though ACP doesn't surface it.
    if dry_run {
        std::env::set_var("SENDA_DRY_RUN", "1");
    }

    let session = match spawn_agent(binary, &[]).await {
        Ok(s) => s,
        Err(e) => {
            return AutomationRun {
                success: false,
                exit_code: None,
                output: String::new(),
                error: Some(format!("ACP spawn `{binary}`: {e}")),
            };
        }
    };

    let result: Result<AutomationRun, AutomationRun> = timeout(wall_clock, async {
        let session_id = session
            .new_session()
            .await
            .map_err(|e| acp_failure(format!("session/new: {e}")))?;
        let mut updates = session
            .prompt(&session_id, prompt)
            .await
            .map_err(|e| acp_failure(format!("session/prompt: {e}")))?;
        let mut buf = String::new();
        while let Some(update) = updates.recv().await {
            match update {
                SessionUpdate::AgentMessage { content } => buf.push_str(&content),
                SessionUpdate::Done => break,
                _ => {}
            }
        }
        Ok(AutomationRun {
            success: true,
            exit_code: Some(0),
            output: buf,
            error: None,
        })
    })
    .await
    .unwrap_or_else(|_| {
        Err(acp_failure(format!(
            "ACP timeout after {}s",
            wall_clock.as_secs()
        )))
    });

    match result {
        Ok(r) | Err(r) => r,
    }
}

fn acp_failure(message: String) -> AutomationRun {
    AutomationRun {
        success: false,
        exit_code: None,
        output: String::new(),
        error: Some(message),
    }
}

/// Resolve an `agent_id` (as returned by `read_catalog`) to the CLI it should
/// run under and the bare agent name. Picks the first target listed by the
/// agent. Returns `None` when the file no longer exists or can't be parsed.
pub fn resolve_agent(db: &Db, agent_id: &str) -> Option<(AgentCli, String)> {
    for entry in scan_catalog(db) {
        if let CatalogEntry::Agent(agent) = entry {
            if agent.id == agent_id {
                let cli = agent.agent.targets.first().copied()?;
                return Some((cli, agent.agent.name));
            }
        }
    }
    None
}

/// Plain (non-Tauri) version of `read_catalog` so background tasks can resolve
/// agents without going through the IPC layer. Mirrors the logic in
/// [`crate::commands::agents::read_catalog`].
fn scan_catalog(db: &Db) -> Vec<CatalogEntry> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    let mut entries = Vec::new();

    let senda_agents = home.join(".senda").join("agents");
    if senda_agents.is_dir() {
        scan_canonical(&senda_agents, &mut entries);
    }

    for cli in AgentCli::ALL {
        let dir = home.join(cli.agents_dir());
        if dir.is_dir() {
            scan_native(&dir, cli, &mut entries);
        }
    }

    if let Ok(repos) = db.list_repos() {
        for repo in repos {
            let agents_dir = Path::new(&repo.local_path).join("agents");
            if agents_dir.is_dir() {
                scan_repo(&agents_dir, repo.id, &repo.repo, &mut entries);
            }
        }
    }

    entries
}

fn scan_canonical(dir: &Path, out: &mut Vec<CatalogEntry>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if !path.is_file() || !ends_with(&path, ".agent.md") {
            continue;
        }
        let id = stem(&path);
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(canonical) = parse_canonical(&raw) {
                out.push(CatalogEntry::Agent(Box::new(senda_core::Agent {
                    id,
                    agent: canonical,
                    source: AgentSource::Personal,
                    canonical_path: Some(path.to_string_lossy().to_string()),
                    warnings_count: 0,
                })));
            }
        }
    }
}

fn scan_native(dir: &Path, cli: AgentCli, out: &mut Vec<CatalogEntry>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let matches = match cli {
            AgentCli::Copilot => ends_with(&path, ".agent.md"),
            AgentCli::ClaudeCode => ends_with(&path, ".md") && !ends_with(&path, ".agent.md"),
            AgentCli::Gemini => ends_with(&path, ".toml"),
        };
        if !matches {
            continue;
        }
        let id = format!("{}/{}", cli.as_id(), stem(&path));
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(parsed) = parse_native(&raw, cli) {
                out.push(CatalogEntry::Agent(Box::new(senda_core::Agent {
                    id,
                    agent: parsed.agent,
                    source: AgentSource::External { original_cli: cli },
                    canonical_path: Some(path.to_string_lossy().to_string()),
                    warnings_count: 0,
                })));
            }
        }
    }
}

fn scan_repo(dir: &Path, repo_id: i64, repo_name: &str, out: &mut Vec<CatalogEntry>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if !path.is_file() || !ends_with(&path, ".agent.md") {
            continue;
        }
        let id = format!("{repo_name}/{}", stem(&path));
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Ok(canonical) = parse_canonical(&raw) {
                out.push(CatalogEntry::Agent(Box::new(senda_core::Agent {
                    id,
                    agent: canonical,
                    source: AgentSource::Repo {
                        repo_id,
                        path: path.to_string_lossy().to_string(),
                    },
                    canonical_path: Some(path.to_string_lossy().to_string()),
                    warnings_count: 0,
                })));
            }
        }
    }
}

fn ends_with(path: &Path, suffix: &str) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(suffix))
}

fn stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .trim_end_matches(".agent")
        .to_string()
}
