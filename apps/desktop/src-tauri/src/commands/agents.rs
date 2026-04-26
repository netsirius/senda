//! `read_catalog` — scan the canonical store + the three native CLI folders
//! and return everything we find as a flat list. Anything that fails to parse
//! becomes a [`CatalogEntry::Error`] so the UI can surface the problem
//! instead of silently dropping the file.

use std::path::Path;

use senda_agent_parser::{parse_canonical, parse_native, ParseError};
use senda_core::{Agent, AgentCli, AgentSource, CanonicalAgent, CatalogEntry};

#[tauri::command]
pub async fn read_catalog(
    db: tauri::State<'_, crate::db::Db>,
) -> Result<Vec<CatalogEntry>, String> {
    let home = dirs::home_dir().ok_or_else(|| "could not resolve home directory".to_string())?;
    let mut entries = Vec::new();

    // Canonical store (source of truth) — agents here are Personal.
    let senda_agents = home.join(".senda").join("agents");
    if senda_agents.is_dir() {
        scan_canonical_dir(&senda_agents, &mut entries);
    }

    // Native CLI folders — anything we find here that isn't in the manifest is
    // External (Phase 1 has no manifest yet, so everything is External).
    for cli in AgentCli::ALL {
        let dir = home.join(cli.agents_dir());
        if !dir.is_dir() {
            continue;
        }
        scan_native_dir(&dir, cli, &mut entries);
    }

    // Connected repos — scan their `agents/` subdirectory and badge each
    // result with the originating repo id.
    let repos = db.list_repos().map_err(|e| format!("db: {e}"))?;
    for repo in repos {
        let agents_dir = std::path::Path::new(&repo.local_path).join("agents");
        if !agents_dir.is_dir() {
            continue;
        }
        scan_repo_dir(&agents_dir, repo.id, &repo.repo, &mut entries);
    }

    tracing::info!(found = entries.len(), "read_catalog complete");
    Ok(entries)
}

fn scan_canonical_dir(dir: &Path, out: &mut Vec<CatalogEntry>) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(err) => {
            tracing::warn!(?err, ?dir, "cannot read canonical agents dir");
            return;
        }
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() || !path_has_canonical_ext(&path) {
            continue;
        }
        let id = id_from_path(&path);
        match std::fs::read_to_string(&path).map_err(io_to_parse) {
            Ok(raw) => match parse_canonical(&raw) {
                Ok(canonical) => out.push(CatalogEntry::Agent(Box::new(make_agent(
                    id,
                    canonical,
                    AgentSource::Personal,
                    Some(path.to_string_lossy().to_string()),
                )))),
                Err(err) => out.push(CatalogEntry::Error {
                    id,
                    path: path.to_string_lossy().to_string(),
                    source: AgentSource::Personal,
                    message: err.to_string(),
                }),
            },
            Err(err) => out.push(CatalogEntry::Error {
                id,
                path: path.to_string_lossy().to_string(),
                source: AgentSource::Personal,
                message: err.to_string(),
            }),
        }
    }
}

fn scan_native_dir(dir: &Path, cli: AgentCli, out: &mut Vec<CatalogEntry>) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(err) => {
            tracing::warn!(?err, ?dir, "cannot read native agents dir");
            return;
        }
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() || !path_matches_cli_ext(&path, cli) {
            continue;
        }
        let id = format!("{}/{}", cli.as_id(), id_from_path(&path));
        match std::fs::read_to_string(&path).map_err(io_to_parse) {
            Ok(raw) => match parse_native(&raw, cli) {
                Ok(parsed) => out.push(CatalogEntry::Agent(Box::new(make_agent(
                    id,
                    parsed.agent,
                    AgentSource::External { original_cli: cli },
                    Some(path.to_string_lossy().to_string()),
                )))),
                Err(err) => out.push(CatalogEntry::Error {
                    id,
                    path: path.to_string_lossy().to_string(),
                    source: AgentSource::External { original_cli: cli },
                    message: err.to_string(),
                }),
            },
            Err(err) => out.push(CatalogEntry::Error {
                id,
                path: path.to_string_lossy().to_string(),
                source: AgentSource::External { original_cli: cli },
                message: err.to_string(),
            }),
        }
    }
}

fn scan_repo_dir(dir: &Path, repo_id: i64, repo_name: &str, out: &mut Vec<CatalogEntry>) {
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(err) => {
            tracing::warn!(?err, ?dir, "cannot read repo agents dir");
            return;
        }
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() || !path_has_canonical_ext(&path) {
            continue;
        }
        let bare = id_from_path(&path);
        let id = format!("{repo_name}/{bare}");
        let source = AgentSource::Repo {
            repo_id,
            path: path.to_string_lossy().to_string(),
        };
        match std::fs::read_to_string(&path).map_err(io_to_parse) {
            Ok(raw) => match parse_canonical(&raw) {
                Ok(canonical) => out.push(CatalogEntry::Agent(Box::new(make_agent(
                    id,
                    canonical,
                    source,
                    Some(path.to_string_lossy().to_string()),
                )))),
                Err(err) => out.push(CatalogEntry::Error {
                    id,
                    path: path.to_string_lossy().to_string(),
                    source,
                    message: err.to_string(),
                }),
            },
            Err(err) => out.push(CatalogEntry::Error {
                id,
                path: path.to_string_lossy().to_string(),
                source,
                message: err.to_string(),
            }),
        }
    }
}

fn path_has_canonical_ext(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".agent.md"))
}

fn path_matches_cli_ext(path: &Path, cli: AgentCli) -> bool {
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return false,
    };
    match cli {
        AgentCli::Copilot => name.ends_with(".agent.md"),
        AgentCli::ClaudeCode => name.ends_with(".md") && !name.ends_with(".agent.md"),
        AgentCli::Gemini => name.ends_with(".toml"),
    }
}

fn id_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        // `triage.agent` -> `triage`
        .trim_end_matches(".agent")
        .to_string()
}

fn make_agent(
    id: String,
    canonical: CanonicalAgent,
    source: AgentSource,
    canonical_path: Option<String>,
) -> Agent {
    Agent {
        id,
        agent: canonical,
        source,
        canonical_path,
        warnings_count: 0,
    }
}

fn io_to_parse(err: std::io::Error) -> ParseError {
    ParseError::Io(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn scan_canonical_dir_returns_agent_for_valid_doc() {
        let dir = tempdir().unwrap();
        let valid = "---\nname: foo\ndescription: x\ntargets: [copilot]\n---\nbody\n";
        write(dir.path(), "foo.agent.md", valid);

        let mut out = Vec::new();
        scan_canonical_dir(dir.path(), &mut out);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], CatalogEntry::Agent(_)));
    }

    #[test]
    fn scan_canonical_dir_returns_error_for_invalid_doc() {
        let dir = tempdir().unwrap();
        write(dir.path(), "bad.agent.md", "not even close");

        let mut out = Vec::new();
        scan_canonical_dir(dir.path(), &mut out);
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0], CatalogEntry::Error { .. }));
    }

    #[test]
    fn scan_native_dir_picks_only_extension_for_cli() {
        let dir = tempdir().unwrap();
        // Claude Code dir contains a stray `.toml` — must be ignored.
        let valid = "---\nname: foo\ndescription: x\n---\nbody\n";
        write(dir.path(), "foo.md", valid);
        write(dir.path(), "ignore.toml", "not relevant");

        let mut out = Vec::new();
        scan_native_dir(dir.path(), AgentCli::ClaudeCode, &mut out);
        assert_eq!(out.len(), 1);
    }
}
