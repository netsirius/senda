//! Tauri commands for managing connected git repositories: add, list,
//! disconnect, sync. The actual git operations live in `senda-git-providers`;
//! this module deals only with persistence (DB + keyring) and the IPC shape.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use senda_git_providers::{detect_provider, Auth};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

use crate::db::{Db, NewRepoRow, RepoRow};
use crate::secrets;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AddRepoArgs {
    pub url: String,
    /// `none`, `pat`, `oauth`. Empty/None for public repos.
    pub auth_kind: String,
    /// Raw token / access token. Stored in the OS keychain, never in SQLite.
    pub auth_token: Option<String>,
    pub branch: Option<String>,
    #[serde(default = "default_auto_sync")]
    pub auto_sync: bool,
}

fn default_auto_sync() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AddRepoResult {
    pub id: i64,
    pub local_path: String,
}

#[tauri::command]
pub async fn add_repo(
    db: tauri::State<'_, Db>,
    app: tauri::AppHandle,
    args: AddRepoArgs,
) -> Result<AddRepoResult, String> {
    let provider = detect_provider(&args.url);
    let identity = provider
        .parse_url(&args.url)
        .await
        .map_err(|e| format!("invalid url: {e}"))?;

    let home = dirs::home_dir().ok_or_else(|| "no home directory".to_string())?;
    let local_path: PathBuf = home.join(".senda").join("cache").join(format!(
        "{}_{}_{}",
        identity.provider_id(),
        identity.org.replace('/', "_"),
        identity.repo
    ));

    let auth = match (args.auth_kind.as_str(), args.auth_token.as_ref()) {
        ("none", _) | ("", _) => Auth::None,
        ("pat", Some(t)) => Auth::PersonalAccessToken(t.clone()),
        ("oauth", Some(t)) => Auth::DeviceFlow {
            access_token: t.clone(),
        },
        ("pat", None) | ("oauth", None) => {
            return Err("auth token required for pat/oauth".into());
        }
        (other, _) => return Err(format!("unknown auth kind: {other}")),
    };

    provider
        .clone(&identity, &local_path, &auth)
        .await
        .map_err(|e| format!("clone: {e}"))?;

    let branch = args.branch.unwrap_or_else(|| "main".to_string());
    let created_at = unix_now();
    let auth_kind = match auth {
        Auth::None => "none",
        Auth::PersonalAccessToken(_) => "pat",
        Auth::DeviceFlow { .. } => "oauth",
    };

    // Persist the row first so we have an id to derive the keyring slot.
    let id = db
        .insert_repo(&NewRepoRow {
            provider: provider.provider_name(),
            org: &identity.org,
            project: identity.project.as_deref(),
            repo: &identity.repo,
            url: &identity.url,
            local_path: &local_path.to_string_lossy(),
            branch: &branch,
            auth_kind,
            auth_keyring_id: None,
            auto_sync: args.auto_sync,
            created_at,
        })
        .map_err(|e| format!("db: {e}"))?;

    if matches!(auth, Auth::PersonalAccessToken(_) | Auth::DeviceFlow { .. }) {
        let keyring_id = format!("{}:repo:{id}", provider.provider_name());
        let token = match args.auth_token {
            Some(t) => t,
            None => return Err("missing token after auth check".into()),
        };
        secrets::save(&keyring_id, &token).map_err(|e| format!("keychain: {e}"))?;
    }

    let _ = app.emit("repos:changed", ());
    Ok(AddRepoResult {
        id,
        local_path: local_path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub async fn list_repos(db: tauri::State<'_, Db>) -> Result<Vec<RepoRow>, String> {
    db.list_repos().map_err(|e| format!("db: {e}"))
}

#[tauri::command]
pub async fn disconnect_repo(
    db: tauri::State<'_, Db>,
    app: tauri::AppHandle,
    id: i64,
) -> Result<(), String> {
    let repos = db.list_repos().map_err(|e| format!("db: {e}"))?;
    let repo = repos
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("repo {id} not found"))?;

    if repo.auth_keyring_id.is_some() {
        let key = format!("{}:repo:{id}", repo.provider);
        let _ = secrets::delete(&key);
    }
    let _ = std::fs::remove_dir_all(&repo.local_path);
    db.delete_repo(id).map_err(|e| format!("db: {e}"))?;
    let _ = app.emit("repos:changed", ());
    Ok(())
}

#[tauri::command]
pub async fn sync_repo(
    db: tauri::State<'_, Db>,
    app: tauri::AppHandle,
    id: i64,
) -> Result<bool, String> {
    let repos = db.list_repos().map_err(|e| format!("db: {e}"))?;
    let repo = repos
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| format!("repo {id} not found"))?;

    let provider = detect_provider(&repo.url);
    let result = provider.pull(std::path::Path::new(&repo.local_path)).await;

    match result {
        Ok(pull) => {
            db.record_sync(id, unix_now(), None)
                .map_err(|e| format!("db: {e}"))?;
            let _ = app.emit("repos:synced", id);
            Ok(pull.updated)
        }
        Err(e) => {
            let msg = e.to_string();
            db.record_sync(id, unix_now(), Some(&msg))
                .map_err(|e| format!("db: {e}"))?;
            Err(msg)
        }
    }
}

trait RepoIdentityExt {
    fn provider_id(&self) -> &str;
}
impl RepoIdentityExt for senda_git_providers::RepoIdentity {
    fn provider_id(&self) -> &str {
        match self.provider {
            senda_core::RepoProviderKind::Github => "github",
            senda_core::RepoProviderKind::Azure => "azure",
            senda_core::RepoProviderKind::Generic => "generic",
        }
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
