//! Background sync loop. Every minute we wake up, look at every connected
//! repo whose `auto_sync` is on and whose `last_synced_at` is older than its
//! configured interval, and attempt a pull. Failures get persisted so the
//! sidebar can flag them; successes emit `repos:synced` so the frontend
//! refreshes its catalog.
//!
//! This is the simplest possible scheduler — Phase 3 introduces the fully
//! featured `senda-scheduler`. The sync loop is independent and stays here.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter};
use tokio::time::sleep;

use senda_git_providers::detect_provider;

use crate::db::Db;

const TICK: Duration = Duration::from_secs(60);

pub fn spawn_background_sync(app: AppHandle, db: Db) {
    tauri::async_runtime::spawn(async move {
        loop {
            if let Err(err) = tick(&app, &db).await {
                tracing::warn!(?err, "sync tick failed");
            }
            sleep(TICK).await;
        }
    });
}

async fn tick(app: &AppHandle, db: &Db) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now = unix_now();
    let repos = db.list_repos()?;
    for repo in repos {
        if !repo.auto_sync {
            continue;
        }
        let interval = repo.sync_interval_seconds as i64;
        let due = match repo.last_synced_at {
            Some(ts) => ts + interval <= now,
            None => true,
        };
        if !due {
            continue;
        }

        let provider = detect_provider(&repo.url);
        let result = provider.pull(std::path::Path::new(&repo.local_path)).await;

        match result {
            Ok(pull) => {
                db.record_sync(repo.id, unix_now(), None)?;
                if pull.updated {
                    let _ = app.emit("repos:synced", repo.id);
                }
            }
            Err(e) => {
                let _ = db.record_sync(repo.id, unix_now(), Some(&e.to_string()));
                tracing::warn!(repo = repo.url, ?e, "auto-sync failed");
            }
        }
    }
    Ok(())
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
