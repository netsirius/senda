//! Publish a canonical draft to a connected repository as a pull request.
//!
//! Flow:
//! 1. Read the draft canonical document from `~/.senda/drafts/`.
//! 2. Look up the destination repo + retrieve its PAT/OAuth token from the keychain.
//! 3. Create a fresh branch off `main` (configurable), drop the doc into
//!    `agents/<name>.agent.md`, commit, push.
//! 4. Call the provider's `create_pr`. For Generic providers this returns
//!    `Unsupported`; we surface that so the UI can render
//!    "push complete; open the PR in your provider".
//! 5. Move the draft to `drafts/published/<name>.agent.md` for traceability.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use senda_git_providers::{commit_and_push, detect_provider, Auth, PrRequest};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::db::Db;
use crate::secrets;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PublishArgs {
    pub agent_name: String,
    pub repo_id: i64,
    pub branch_name: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub reviewers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PublishResult {
    pub pr_url: Option<String>,
    pub pr_number: Option<u32>,
    pub branch: String,
    /// Set when the provider can't create a PR (Generic git). Frontend
    /// renders a friendly fallback in that case.
    pub manual_pr_required: bool,
}

#[tauri::command]
pub async fn publish_agent(
    db: tauri::State<'_, Db>,
    app: AppHandle,
    args: PublishArgs,
) -> Result<PublishResult, String> {
    let home = dirs::home_dir().ok_or_else(|| "no home".to_string())?;
    let draft_path: PathBuf = home
        .join(".senda")
        .join("drafts")
        .join(format!("{}.agent.md", args.agent_name));
    if !draft_path.exists() {
        return Err(format!("draft not found: {}", draft_path.display()));
    }
    let canonical_doc =
        std::fs::read_to_string(&draft_path).map_err(|e| format!("read draft: {e}"))?;

    // Resolve the destination repo + auth.
    let repos = db.list_repos().map_err(|e| format!("db: {e}"))?;
    let repo = repos
        .into_iter()
        .find(|r| r.id == args.repo_id)
        .ok_or_else(|| format!("repo {} not found", args.repo_id))?;

    let auth = match repo.auth_kind.as_str() {
        "none" => Auth::None,
        "pat" | "oauth" => {
            let key = format!("{}:repo:{}", repo.provider, repo.id);
            let token = secrets::load(&key)
                .map_err(|e| format!("keychain: {e}"))?
                .ok_or_else(|| format!("no token for repo {}", repo.id))?;
            if repo.auth_kind == "oauth" {
                Auth::DeviceFlow {
                    access_token: token,
                }
            } else {
                Auth::PersonalAccessToken(token)
            }
        }
        other => return Err(format!("unknown auth_kind: {other}")),
    };

    let provider = detect_provider(&repo.url);
    let identity = provider
        .parse_url(&repo.url)
        .await
        .map_err(|e| format!("parse url: {e}"))?;

    let rel_path = format!("agents/{}.agent.md", args.agent_name);
    let local_repo_path = PathBuf::from(&repo.local_path);

    // Compute author info from system git config — fall back to a generic
    // "Senda" identity if neither's set.
    // whoami returns plain Strings — `realname()` falls back to the username
    // when the OS doesn't expose a full name, so this is always populated.
    let author = (
        whoami::realname(),
        format!("{}@senda.local", whoami::username()),
    );

    let local_repo_path_clone = local_repo_path.clone();
    let branch_name = args.branch_name.clone();
    let base_branch = repo.branch.clone();
    let auth_clone = auth.clone();
    let title = args.title.clone();
    tokio::task::spawn_blocking(move || {
        commit_and_push(
            &local_repo_path_clone,
            &base_branch,
            &branch_name,
            &[(rel_path.clone(), canonical_doc)],
            &title,
            &author,
            auth_clone,
        )
    })
    .await
    .map_err(|e| format!("spawn: {e}"))?
    .map_err(|e| format!("commit/push: {e}"))?;

    if !provider.supports_pr_creation() {
        // Move draft to published dir even when no PR API is available.
        archive_draft(&home, &args.agent_name)?;
        let _ = app.emit("publish:complete", &args.repo_id);
        return Ok(PublishResult {
            pr_url: None,
            pr_number: None,
            branch: args.branch_name,
            manual_pr_required: true,
        });
    }

    let pr = provider
        .create_pr(
            &identity,
            PrRequest {
                title: args.title.clone(),
                body: args.body.clone(),
                base: repo.branch.clone(),
                head: args.branch_name.clone(),
                reviewers: args.reviewers,
            },
            &auth,
        )
        .await
        .map_err(|e| format!("create_pr: {e}"))?;

    let now = unix_now();
    if let Err(e) = db.0.lock().execute(
        "INSERT INTO published_agents (agent_name, repo_id, pr_url, pr_number, pr_state, draft_path, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, 'open', ?5, ?6, ?6)",
        rusqlite::params![
            args.agent_name,
            args.repo_id,
            pr.url,
            pr.number,
            draft_path.to_string_lossy(),
            now,
        ],
    ) {
        tracing::warn!(?e, "failed to record published agent");
    }

    archive_draft(&home, &args.agent_name)?;
    let _ = app.emit("publish:complete", &args.repo_id);

    Ok(PublishResult {
        pr_url: Some(pr.url),
        pr_number: Some(pr.number),
        branch: args.branch_name,
        manual_pr_required: false,
    })
}

fn archive_draft(home: &std::path::Path, name: &str) -> Result<(), String> {
    let drafts = home.join(".senda").join("drafts");
    let archive = drafts.join("published");
    std::fs::create_dir_all(&archive).map_err(|e| format!("mkdir archive: {e}"))?;
    let from = drafts.join(format!("{name}.agent.md"));
    let to = archive.join(format!("{name}.agent.md"));
    if from.exists() {
        std::fs::rename(&from, &to).map_err(|e| format!("archive draft: {e}"))?;
    }
    Ok(())
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
