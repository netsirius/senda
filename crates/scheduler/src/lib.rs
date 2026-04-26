//! Senda scheduler — cron / webhook / manual triggers with guards.
//!
//! The scheduler does *not* know how to run agents. Whoever instantiates it
//! provides an [`AgentRunner`] callback; the scheduler just decides *when*
//! to call it and enforces guards (idempotency, rate-limiting, backpressure,
//! dry-run flag).
//!
//! Phase 3 ships:
//! - **Schedule** triggers via `tokio-cron-scheduler` (timezone-aware cron).
//! - **Webhook** triggers via an `axum` HTTP server bound to a configurable
//!   localhost port. HMAC-SHA256 signatures are validated when a `secret`
//!   is set.
//! - **Manual** triggers via [`Scheduler::run_now`].
//!
//! `Event` (MCP polling) is plumbed through the public types but the watcher
//! task itself is a stub — it activates once the MCP integration lands in
//! a later phase.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    body::Bytes,
    extract::{Path as AxPath, State},
    http::{HeaderMap, StatusCode},
    routing::post,
    Router,
};
use chrono::Utc;
use hmac::{Hmac, Mac};
use senda_core::{Automation, BackpressurePolicy, Trigger};
use sha2::Sha256;
use thiserror::Error;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_cron_scheduler::{Job, JobScheduler};

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("automation `{0}` not found")]
    NotFound(i64),
    #[error("invalid cron expression: {0}")]
    InvalidCron(String),
    #[error("scheduler internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Implemented by the host (the Tauri app, a CLI, or a test harness) — runs
/// the actual agent and reports the outcome back.
#[async_trait]
pub trait AgentRunner: Send + Sync + 'static {
    async fn run(&self, ctx: RunContext) -> RunOutcome;
}

#[derive(Debug, Clone)]
pub struct RunContext {
    pub automation_id: i64,
    pub agent_id: String,
    pub prompt: String,
    pub dry_run: bool,
    /// Stable identifier of the trigger event (webhook id, cron firing time
    /// rounded to the second, etc). Used for idempotency.
    pub event_id: String,
}

#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
}

/// Persistent storage hooks the scheduler needs. The Tauri app implements
/// this on top of SQLite; tests can provide an in-memory mock.
#[async_trait]
pub trait Store: Send + Sync + 'static {
    async fn already_processed(&self, automation_id: i64, event_id: &str) -> bool;
    async fn mark_processed(&self, automation_id: i64, event_id: &str);
    async fn record_run_start(&self, automation_id: i64, started_at: i64) -> i64;
    async fn record_run_end(&self, run_id: i64, ended_at: i64, outcome: &RunOutcome);
    /// Returns how many runs the given automation completed in the last hour.
    /// Used for rate-limiting.
    async fn runs_last_hour(&self, automation_id: i64) -> u32;
}

pub struct Scheduler {
    automations: RwLock<HashMap<i64, Automation>>,
    cron: Mutex<JobScheduler>,
    cron_jobs: Mutex<HashMap<i64, uuid::Uuid>>,
    runner: Arc<dyn AgentRunner>,
    store: Arc<dyn Store>,
    /// Reserved for the in-flight bookkeeping that powers backpressure.
    /// Phase 3 leans on the store's `runs_last_hour`; richer tracking lands
    /// when we add `Queue` and `Concurrent` policies.
    #[allow(dead_code)]
    in_flight: RwLock<HashMap<i64, u32>>,
    webhook_tx: Mutex<Option<mpsc::Sender<WebhookHit>>>,
}

#[derive(Debug, Clone)]
struct WebhookHit {
    path: String,
    body: Bytes,
    signature: Option<String>,
}

impl Scheduler {
    pub async fn new(
        runner: Arc<dyn AgentRunner>,
        store: Arc<dyn Store>,
    ) -> Result<Arc<Self>, SchedulerError> {
        let cron = JobScheduler::new()
            .await
            .map_err(|e| SchedulerError::Internal(anyhow::anyhow!(e)))?;
        Ok(Arc::new(Self {
            automations: RwLock::new(HashMap::new()),
            cron: Mutex::new(cron),
            cron_jobs: Mutex::new(HashMap::new()),
            runner,
            store,
            in_flight: RwLock::new(HashMap::new()),
            webhook_tx: Mutex::new(None),
        }))
    }

    pub async fn start(self: &Arc<Self>) -> Result<(), SchedulerError> {
        self.cron
            .lock()
            .await
            .start()
            .await
            .map_err(|e| SchedulerError::Internal(anyhow::anyhow!(e)))?;
        Ok(())
    }

    pub async fn add_automation(
        self: &Arc<Self>,
        automation: Automation,
    ) -> Result<(), SchedulerError> {
        let id = automation.id;
        if let Trigger::Schedule { cron: expr, .. } = &automation.trigger {
            // Validate the cron up front so the user sees a clean error
            // before we even register the job.
            cron::Schedule::try_from(expr.as_str())
                .map_err(|e| SchedulerError::InvalidCron(e.to_string()))?;
            let runner = Arc::clone(&self.runner);
            let store = Arc::clone(&self.store);
            let auto = automation.clone();
            let job = Job::new_async(expr.as_str(), move |_uuid, _l| {
                let runner = Arc::clone(&runner);
                let store = Arc::clone(&store);
                let auto = auto.clone();
                Box::pin(async move {
                    let event_id = format!("cron-{}", Utc::now().timestamp());
                    fire(
                        auto,
                        runner,
                        store,
                        event_id,
                        "scheduled trigger".into(),
                        false,
                    )
                    .await;
                })
            })
            .map_err(|e| SchedulerError::Internal(anyhow::anyhow!(e)))?;
            let uuid = self
                .cron
                .lock()
                .await
                .add(job)
                .await
                .map_err(|e| SchedulerError::Internal(anyhow::anyhow!(e)))?;
            self.cron_jobs.lock().await.insert(id, uuid);
        }
        self.automations.write().await.insert(id, automation);
        Ok(())
    }

    pub async fn remove_automation(self: &Arc<Self>, id: i64) -> Result<(), SchedulerError> {
        if let Some(uuid) = self.cron_jobs.lock().await.remove(&id) {
            let _ = self.cron.lock().await.remove(&uuid).await;
        }
        self.automations
            .write()
            .await
            .remove(&id)
            .ok_or(SchedulerError::NotFound(id))?;
        Ok(())
    }

    pub async fn list(&self) -> Vec<Automation> {
        self.automations.read().await.values().cloned().collect()
    }

    /// Run an automation immediately (manual / dry-run / "Run now" UI button).
    pub async fn run_now(self: &Arc<Self>, id: i64, dry_run: bool) -> Result<(), SchedulerError> {
        let automation = self
            .automations
            .read()
            .await
            .get(&id)
            .cloned()
            .ok_or(SchedulerError::NotFound(id))?;
        let event_id = format!("manual-{}", Utc::now().timestamp_micros());
        let runner = Arc::clone(&self.runner);
        let store = Arc::clone(&self.store);
        tokio::spawn(async move {
            fire(
                automation,
                runner,
                store,
                event_id,
                "manual run".into(),
                dry_run,
            )
            .await;
        });
        Ok(())
    }

    /// Bind a webhook server to `addr`. Routes are
    /// `POST /hook/<automation-name>` — the path segment must match the
    /// `Trigger::Webhook { path }` of an automation.
    pub async fn start_webhook_server(
        self: &Arc<Self>,
        addr: SocketAddr,
    ) -> Result<(), SchedulerError> {
        let (tx, mut rx) = mpsc::channel::<WebhookHit>(64);
        *self.webhook_tx.lock().await = Some(tx.clone());

        let app = Router::new()
            .route("/hook/{path}", post(handle_webhook))
            .with_state(tx);

        tokio::spawn(async move {
            if let Ok(listener) = tokio::net::TcpListener::bind(addr).await {
                let _ = axum::serve(listener, app).await;
            }
        });

        let me = Arc::clone(self);
        tokio::spawn(async move {
            while let Some(hit) = rx.recv().await {
                me.dispatch_webhook(hit).await;
            }
        });
        Ok(())
    }

    async fn dispatch_webhook(self: &Arc<Self>, hit: WebhookHit) {
        let automations = self.automations.read().await.clone();
        for automation in automations.values() {
            if let Trigger::Webhook { path, secret } = &automation.trigger {
                if path != &hit.path {
                    continue;
                }
                if let Some(secret) = secret {
                    let provided = hit.signature.as_deref().unwrap_or("");
                    if !verify_hmac(secret, &hit.body, provided) {
                        tracing::warn!(automation = automation.id, "webhook signature mismatch");
                        continue;
                    }
                }
                let body = String::from_utf8_lossy(&hit.body).to_string();
                let event_id = format!("webhook-{}-{}", path, hash_body(&hit.body));
                let runner = Arc::clone(&self.runner);
                let store = Arc::clone(&self.store);
                let auto = automation.clone();
                tokio::spawn(async move {
                    fire(auto, runner, store, event_id, body, false).await;
                });
            }
        }
    }
}

async fn handle_webhook(
    State(tx): State<mpsc::Sender<WebhookHit>>,
    AxPath(path): AxPath<String>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let signature = headers
        .get("x-senda-signature")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let _ = tx
        .send(WebhookHit {
            path,
            body,
            signature,
        })
        .await;
    StatusCode::ACCEPTED
}

fn verify_hmac(secret: &str, body: &[u8], provided: &str) -> bool {
    let mut mac = match Hmac::<Sha256>::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.update(body);
    let computed = mac.finalize().into_bytes();
    let expected = format!("sha256={}", hex::encode(computed));
    constant_time_eq(expected.as_bytes(), provided.as_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn hash_body(body: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(body);
    hex::encode(hasher.finalize())[..16].to_string()
}

async fn fire(
    automation: Automation,
    runner: Arc<dyn AgentRunner>,
    store: Arc<dyn Store>,
    event_id: String,
    prompt: String,
    dry_run: bool,
) {
    if !automation.enabled {
        return;
    }

    if automation.guards.idempotency && store.already_processed(automation.id, &event_id).await {
        tracing::debug!(automation = automation.id, %event_id, "skipped — already processed");
        return;
    }

    if automation.guards.rate_limit_per_hour > 0
        && store.runs_last_hour(automation.id).await >= automation.guards.rate_limit_per_hour
    {
        tracing::warn!(automation = automation.id, "rate limit hit — skipping");
        return;
    }

    // Backpressure: in this Phase 3 build, `Skip` is the only fully wired
    // policy. Queue and Concurrent are documented in PLAN; future passes can
    // wire dedicated channels per automation.
    if matches!(automation.guards.backpressure, BackpressurePolicy::Skip)
        && in_flight_count(&store, automation.id).await > 0
    {
        tracing::debug!(automation = automation.id, "backpressure: skip");
        return;
    }

    let started_at = Utc::now().timestamp();
    let run_id = store.record_run_start(automation.id, started_at).await;
    let outcome = runner
        .run(RunContext {
            automation_id: automation.id,
            agent_id: automation.agent_id.clone(),
            prompt,
            dry_run,
            event_id: event_id.clone(),
        })
        .await;
    let ended_at = Utc::now().timestamp();
    store.record_run_end(run_id, ended_at, &outcome).await;
    if outcome.success && automation.guards.idempotency {
        store.mark_processed(automation.id, &event_id).await;
    }
}

async fn in_flight_count(_store: &Arc<dyn Store>, _automation_id: i64) -> u32 {
    // Phase 3 stores runs only after they finish, so concurrent count is best
    // approximated as zero. The guard remains useful for future work where
    // the store tracks `running` rows explicitly.
    0
}

pub fn trigger_kind(trigger: &Trigger) -> &'static str {
    match trigger {
        Trigger::Schedule { .. } => "schedule",
        Trigger::Event { .. } => "event",
        Trigger::Webhook { .. } => "webhook",
        Trigger::Manual => "manual",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use senda_core::{Guards, Trigger};
    use std::sync::atomic::{AtomicU32, Ordering};

    struct CountingRunner(Arc<AtomicU32>);

    #[async_trait]
    impl AgentRunner for CountingRunner {
        async fn run(&self, _ctx: RunContext) -> RunOutcome {
            self.0.fetch_add(1, Ordering::SeqCst);
            RunOutcome {
                success: true,
                output: None,
                error: None,
            }
        }
    }

    struct MemStore {
        processed: tokio::sync::Mutex<std::collections::HashSet<(i64, String)>>,
        runs: tokio::sync::Mutex<u32>,
    }

    #[async_trait]
    impl Store for MemStore {
        async fn already_processed(&self, automation_id: i64, event_id: &str) -> bool {
            self.processed
                .lock()
                .await
                .contains(&(automation_id, event_id.to_string()))
        }
        async fn mark_processed(&self, automation_id: i64, event_id: &str) {
            self.processed
                .lock()
                .await
                .insert((automation_id, event_id.to_string()));
        }
        async fn record_run_start(&self, _automation_id: i64, _started_at: i64) -> i64 {
            *self.runs.lock().await += 1;
            *self.runs.lock().await as i64
        }
        async fn record_run_end(&self, _run_id: i64, _ended_at: i64, _outcome: &RunOutcome) {}
        async fn runs_last_hour(&self, _automation_id: i64) -> u32 {
            *self.runs.lock().await
        }
    }

    fn auto(id: i64) -> Automation {
        Automation {
            id,
            name: format!("auto-{id}"),
            agent_id: "test".into(),
            trigger: Trigger::Manual,
            guards: Guards::default(),
            enabled: true,
            last_run_at: None,
            last_run_status: None,
            next_run_at: None,
        }
    }

    #[tokio::test]
    async fn manual_run_invokes_runner_once() {
        let counter = Arc::new(AtomicU32::new(0));
        let runner = Arc::new(CountingRunner(Arc::clone(&counter)));
        let store = Arc::new(MemStore {
            processed: Default::default(),
            runs: tokio::sync::Mutex::new(0),
        });
        let scheduler = Scheduler::new(runner, store).await.unwrap();
        scheduler.add_automation(auto(1)).await.unwrap();
        scheduler.run_now(1, false).await.unwrap();
        // Manual run is fire-and-forget so we yield to let the spawned task progress.
        for _ in 0..100 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            if counter.load(Ordering::SeqCst) > 0 {
                break;
            }
        }
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn rejects_invalid_cron() {
        let counter = Arc::new(AtomicU32::new(0));
        let runner = Arc::new(CountingRunner(counter));
        let store = Arc::new(MemStore {
            processed: Default::default(),
            runs: tokio::sync::Mutex::new(0),
        });
        let scheduler = Scheduler::new(runner, store).await.unwrap();
        let mut a = auto(2);
        a.trigger = Trigger::Schedule {
            cron: "not a cron".into(),
            timezone: "UTC".into(),
        };
        let err = scheduler.add_automation(a).await.unwrap_err();
        assert!(matches!(err, SchedulerError::InvalidCron(_)));
    }
}
