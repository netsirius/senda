//! Filesystem watcher over the three native agent folders.
//!
//! Phase 4-F: when an external tool (a Copilot `agent-creator` agent, the
//! user's `claude` CLI, etc.) drops a new `.agent.md` / `.md` / `.toml` file
//! into one of those folders **while the wizard is in chat mode**, we want
//! to convert it to canonical and import it as a draft so the editor opens
//! pre-filled.
//!
//! This module provides the long-lived watcher; the wizard subscribes to
//! `agents:detected` events and decides whether to act.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use notify::{EventKind, RecursiveMode, Watcher};
use senda_agent_parser::{parse_native, serialize_canonical};
use senda_core::AgentCli;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DetectedAgent {
    pub original_path: String,
    pub canonical_path: String,
    pub cli: AgentCli,
    pub name: String,
}

/// Spawns a single watcher task that lives for the duration of the app.
/// Newly created agent files are converted to canonical and dropped into
/// `~/.senda/drafts/` (with a `_imported` suffix to avoid stomping any
/// existing draft); a `agents:detected` event fires for the frontend.
pub fn spawn_agent_watcher(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = run(app).await {
            tracing::warn!(?e, "agent watcher exited");
        }
    });
}

async fn run(app: AppHandle) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let home = dirs::home_dir().ok_or("no home")?;
    let watched: Vec<(PathBuf, AgentCli)> = AgentCli::ALL
        .iter()
        .map(|cli| (home.join(cli.agents_dir()), *cli))
        .filter(|(p, _)| p.is_dir())
        .collect();

    let known: Arc<Mutex<std::collections::HashSet<PathBuf>>> = Arc::new(Mutex::new(
        watched
            .iter()
            .flat_map(|(dir, _)| std::fs::read_dir(dir).ok().into_iter().flatten().flatten())
            .map(|entry| entry.path())
            .collect(),
    ));

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(PathBuf, AgentCli)>();

    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else {
            return;
        };
        if !matches!(
            event.kind,
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any
        ) {
            return;
        }
        for path in event.paths {
            // Determine which CLI's folder this came from.
            let cli = path
                .ancestors()
                .nth(1)
                .and_then(|parent| parent.file_name())
                .and_then(|n| n.to_str())
                .and_then(|n| match n {
                    "agents" => path.ancestors().nth(2),
                    _ => None,
                })
                .and_then(|root| root.file_name())
                .and_then(|n| n.to_str())
                .and_then(|n| match n {
                    ".copilot" => Some(AgentCli::Copilot),
                    ".claude" => Some(AgentCli::ClaudeCode),
                    ".gemini" => Some(AgentCli::Gemini),
                    _ => None,
                });
            if let Some(cli) = cli {
                let _ = tx.send((path, cli));
            }
        }
    })?;

    for (dir, _) in &watched {
        let _ = watcher.watch(dir, RecursiveMode::NonRecursive);
    }

    while let Some((path, cli)) = rx.recv().await {
        if !path.is_file() {
            continue;
        }
        if !is_agent_file(&path, cli) {
            continue;
        }
        // De-dupe — `notify` can fire multiple times per save.
        {
            let mut set = known.lock().await;
            if !set.insert(path.clone()) {
                continue;
            }
        }
        if let Some(detected) = handle_new_file(&path, cli, &home).await {
            let _ = app.emit("agents:detected", &detected);
        }
    }

    Ok(())
}

fn is_agent_file(path: &Path, cli: AgentCli) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    match cli {
        AgentCli::Copilot => name.ends_with(".agent.md"),
        AgentCli::ClaudeCode => name.ends_with(".md") && !name.ends_with(".agent.md"),
        AgentCli::Gemini => name.ends_with(".toml"),
    }
}

async fn handle_new_file(path: &Path, cli: AgentCli, home: &Path) -> Option<DetectedAgent> {
    let raw = std::fs::read_to_string(path).ok()?;
    let parsed = parse_native(&raw, cli).ok()?;
    let drafts = home.join(".senda").join("drafts");
    let _ = std::fs::create_dir_all(&drafts);

    let mut name = parsed.agent.name.clone();
    let mut target = drafts.join(format!("{name}.agent.md"));
    if target.exists() {
        name = format!("{name}-imported");
        target = drafts.join(format!("{name}.agent.md"));
    }
    let mut to_write = parsed.agent.clone();
    to_write.name = name.clone();
    let serialized = serialize_canonical(&to_write).ok()?;
    std::fs::write(&target, serialized).ok()?;
    Some(DetectedAgent {
        original_path: path.to_string_lossy().to_string(),
        canonical_path: target.to_string_lossy().to_string(),
        cli,
        name,
    })
}
