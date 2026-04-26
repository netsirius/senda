//! senda-scheduler — Phase 0 ships only the public surface and an in-memory
//! registry. Phase 3 wires `tokio-cron-scheduler`, axum, and MCP watchers.

use senda_core::{Automation, Trigger};
use std::collections::HashMap;
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Debug, Error)]
pub enum SchedulerError {
    #[error("automation `{0}` not found")]
    NotFound(i64),
    #[error("scheduler is not running")]
    NotRunning,
}

#[derive(Default)]
pub struct Scheduler {
    automations: RwLock<HashMap<i64, Automation>>,
    running: RwLock<bool>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn start(&self) -> Result<(), SchedulerError> {
        let mut running = self.running.write().await;
        *running = true;
        tracing::info!(
            "scheduler started ({} automation(s))",
            self.automations.read().await.len()
        );
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), SchedulerError> {
        let mut running = self.running.write().await;
        *running = false;
        Ok(())
    }

    pub async fn add_automation(&self, automation: Automation) -> Result<(), SchedulerError> {
        let mut map = self.automations.write().await;
        map.insert(automation.id, automation);
        Ok(())
    }

    pub async fn remove_automation(&self, id: i64) -> Result<(), SchedulerError> {
        let mut map = self.automations.write().await;
        map.remove(&id).ok_or(SchedulerError::NotFound(id))?;
        Ok(())
    }

    pub async fn list(&self) -> Vec<Automation> {
        self.automations.read().await.values().cloned().collect()
    }
}

/// Phase 0 helper to classify a [`Trigger`] for logging / debugging.
pub fn trigger_kind(trigger: &Trigger) -> &'static str {
    match trigger {
        Trigger::Schedule { .. } => "schedule",
        Trigger::Event { .. } => "event",
        Trigger::Webhook { .. } => "webhook",
        Trigger::Manual => "manual",
    }
}
