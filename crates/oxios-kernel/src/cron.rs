//! Cron scheduler for time-based autonomous agent execution.
//!
//! Allows scheduling agents to run on cron-like schedules without user intervention.
//! Supports 5-field (Linux cron) and 6-7 field expressions via the `cron` crate.

use crate::config::CronConfig;
use crate::scheduler::Priority;
use crate::state_store::StateStore;
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use cron::Schedule;
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use uuid::Uuid;

// ── Data types ─────────────────────────────────────────────

/// Source of a cron job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
#[serde(rename_all = "lowercase")]
pub enum JobSource {
    /// Defined in config.toml.
    Config,
    /// Created via API.
    #[default]
    Api,
}

/// A cron job definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: Uuid,
    pub name: String,
    pub schedule: String,
    pub goal: String,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(default = "default_toolchain")]
    pub toolchain: String,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run: Option<DateTime<Utc>>,
    #[serde(default)]
    pub run_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_success: Option<bool>,
    #[serde(default)]
    pub source: JobSource,
}

fn default_toolchain() -> String {
    "default".into()
}

fn default_true() -> bool {
    true
}

impl CronJob {
    /// Create a new cron job with a parsed schedule.
    pub fn new(name: String, schedule: String, goal: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            schedule,
            goal,
            constraints: vec![],
            acceptance_criteria: vec![],
            toolchain: default_toolchain(),
            priority: Priority::default(),
            enabled: true,
            last_run: None,
            next_run: None,
            run_count: 0,
            last_result: None,
            last_success: None,
            source: JobSource::Api,
        }
    }
}

/// Result of a single cron job execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobResult {
    pub job_id: Uuid,
    pub job_name: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub success: bool,
    pub summary: String,
}

/// Update fields for an existing cron job.
#[derive(Debug, Default, Deserialize)]
pub struct CronJobUpdate {
    pub name: Option<String>,
    pub schedule: Option<String>,
    pub goal: Option<String>,
    pub constraints: Option<Vec<String>>,
    pub acceptance_criteria: Option<Vec<String>>,
    pub toolchain: Option<String>,
    pub priority: Option<Priority>,
    pub enabled: Option<bool>,
}

// ── CronScheduler ───────────────────────────────────────────

/// The cron scheduler for time-based autonomous agent execution.
///
/// Allows scheduling agents to run on cron-like schedules without user intervention.
/// Supports 5-field (Linux cron) and 6-7 field expressions via the `cron` crate.
pub struct CronScheduler {
    jobs: Arc<RwLock<HashMap<Uuid, CronJob>>>,
    schedules: Arc<Mutex<HashMap<Uuid, Schedule>>>,
    running_jobs: Arc<Mutex<HashSet<Uuid>>>,
    state_store: Arc<StateStore>,
    cancel: Arc<AtomicBool>,
    dirty: Arc<AtomicBool>,
    tick_interval_secs: u64,
}

impl CronScheduler {
    /// Create a new CronScheduler.
    ///
    /// # Arguments
    /// * `state_store` - State store for persisting job definitions
    /// * `tick_interval_secs` - How often to check for due jobs (in seconds)
    pub fn new(state_store: Arc<StateStore>, tick_interval_secs: u64) -> Self {
        Self {
            jobs: Arc::new(RwLock::new(HashMap::new())),
            schedules: Arc::new(Mutex::new(HashMap::new())),
            running_jobs: Arc::new(Mutex::new(HashSet::new())),
            state_store,
            cancel: Arc::new(AtomicBool::new(false)),
            dirty: Arc::new(AtomicBool::new(false)),
            tick_interval_secs,
        }
    }

    /// Normalize a cron expression: prepend seconds field if 5-field (Linux style).
    fn normalize_expr(expr: &str) -> String {
        let fields: Vec<&str> = expr.split_whitespace().collect();
        match fields.len() {
            5 => format!("0 {}", expr),
            _ => expr.to_string(),
        }
    }

    /// Parse a cron expression into a `Schedule`.
    fn parse_schedule(&self, expr: &str) -> Result<Schedule> {
        let normalized = Self::normalize_expr(expr);
        Schedule::from_str(&normalized)
            .map_err(|e| anyhow::anyhow!("Invalid cron expression '{}': {}", expr, e))
    }

    /// Compute the next fire time after `after`.
    fn next_fire_time(&self, schedule: &Schedule, after: &DateTime<Utc>) -> Option<DateTime<Utc>> {
        schedule.after(after).next()
    }

    /// Add a job. Parses schedule, computes next_run, stores.
    pub async fn add_job(&self, job: CronJob) -> Result<Uuid> {
        let schedule = self.parse_schedule(&job.schedule)?;
        let next = self.next_fire_time(&schedule, &Utc::now());
        let id = job.id;

        self.schedules.lock().insert(id, schedule);
        self.jobs
            .write()
            .insert(id, CronJob { next_run: next, ..job });
        self.dirty.store(true, Ordering::Relaxed);
        self.persist_jobs().await;

        tracing::info!(
            name = %self.jobs.read().get(&id).map(|j| j.name.as_str()).unwrap_or("?"),
            %id,
            "Cron job added"
        );
        Ok(id)
    }

    /// Remove a job by ID.
    pub async fn remove_job(&self, id: Uuid) -> Result<()> {
        self.schedules.lock().remove(&id);
        self.jobs
            .write()
            .remove(&id)
            .ok_or_else(|| anyhow::anyhow!("Job {} not found", id))?;
        self.dirty.store(true, Ordering::Relaxed);
        self.persist_jobs().await;
        tracing::info!(%id, "Cron job removed");
        Ok(())
    }

    /// Update a job's fields (enabled, schedule, goal, etc).
    pub async fn update_job(&self, id: Uuid, update: CronJobUpdate) -> Result<()> {
        // Separate the sync mutation from the async persist to avoid
        // holding a !Send RwLockWriteGuard across an await point.
        let should_persist = {
            let mut jobs = self.jobs.write();
            let job = jobs
                .get_mut(&id)
                .ok_or_else(|| anyhow::anyhow!("Job {} not found", id))?;

            if let Some(name) = update.name {
                job.name = name;
            }
            if let Some(schedule) = &update.schedule {
                let parsed = self.parse_schedule(schedule)?;
                self.schedules.lock().insert(id, parsed);
                job.schedule = schedule.clone();
                // Recompute next_run
                let sched = self.schedules.lock().get(&id).cloned();
                if let Some(s) = sched {
                    job.next_run = self.next_fire_time(&s, &Utc::now());
                }
            }
            if let Some(goal) = update.goal {
                job.goal = goal;
            }
            if let Some(constraints) = update.constraints {
                job.constraints = constraints;
            }
            if let Some(criteria) = update.acceptance_criteria {
                job.acceptance_criteria = criteria;
            }
            if let Some(toolchain) = update.toolchain {
                job.toolchain = toolchain;
            }
            if let Some(priority) = update.priority {
                job.priority = priority;
            }
            if let Some(enabled) = update.enabled {
                job.enabled = enabled;
            }

            self.dirty.store(true, Ordering::Relaxed);
            true
        }; // RwLockWriteGuard dropped here, before any .await

        if should_persist {
            self.persist_jobs().await;
        }
        Ok(())
    }

    /// Toggle a job enabled/disabled.
    pub async fn toggle_job(&self, id: Uuid, enabled: bool) -> Result<()> {
        self.update_job(id, CronJobUpdate { enabled: Some(enabled), ..Default::default() })
            .await
    }

    /// List all jobs.
    pub fn list_jobs(&self) -> Vec<CronJob> {
        self.jobs.read().values().cloned().collect()
    }

    /// Get a single job.
    pub fn get_job(&self, id: Uuid) -> Option<CronJob> {
        self.jobs.read().get(&id).cloned()
    }

    /// Check if a job is currently running.
    pub fn is_running(&self, id: Uuid) -> bool {
        self.running_jobs.lock().contains(&id)
    }

    /// Trigger a job immediately (manual execution, ignores schedule).
    /// Returns the job goal as a string for the caller to execute.
    /// The caller is responsible for calling `mark_job_completed` after execution.
    pub fn trigger_job(&self, id: Uuid) -> Result<CronJob> {
        let job = self
            .jobs
            .read()
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Job {} not found", id))?;

        if self.running_jobs.lock().contains(&id) {
            bail!("Job '{}' is already running", job.name);
        }

        self.running_jobs.lock().insert(id);
        Ok(job)
    }

    /// Mark a job execution as completed.
    pub async fn mark_job_completed(&self, id: Uuid, success: bool, summary: String) {
        self.running_jobs.lock().remove(&id);
        let new_next_run = {
            let mut jobs = self.jobs.write();
            if let Some(job) = jobs.get_mut(&id) {
                job.last_run = Some(Utc::now());
                job.last_result = Some(summary);
                job.last_success = Some(success);
                job.run_count += 1;
                // Recompute next_run
                let sched = self.schedules.lock().get(&id).cloned();
                sched.and_then(|s| self.next_fire_time(&s, &Utc::now()))
            } else {
                None
            }
        };
        if let Some(next_run) = new_next_run {
            let mut jobs = self.jobs.write();
            if let Some(job) = jobs.get_mut(&id) {
                job.next_run = Some(next_run);
            }
        }
        self.dirty.store(true, Ordering::Relaxed);
        self.persist_jobs().await;
    }

    /// Stop the scheduler loop.
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        tracing::info!("Cron scheduler stop requested");
    }

    /// Start the main loop. Must be called on an `Arc<Self>`.
    ///
    /// # Arguments
    /// * `executor` - Async closure `(Uuid, String) -> Fut` where args are `(job_id, goal)`,
    ///   returning `(success, summary)`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let scheduler = Arc::new(CronScheduler::new(state_store, 60));
    /// scheduler.clone().start(|id, goal| async move {
    ///     // execute the agent...
    ///     (true, "Done".to_string())
    /// }).await;
    /// ```
    pub async fn start<F, Fut>(self: Arc<Self>, executor: F)
    where
        F: Fn(Uuid, String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = (bool, String)> + Send + 'static,
    {
        let executor = Arc::new(executor);
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(self.tick_interval_secs));

        tracing::info!(interval_secs = self.tick_interval_secs, "Cron scheduler started");

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if self.cancel.load(Ordering::Relaxed) {
                        tracing::info!("Cron scheduler stopped");
                        return;
                    }
                    self.tick_inner(&executor).await;
                }
            }
        }
    }

    /// Single tick: find due jobs and spawn execution.
    async fn tick_inner<F, Fut>(&self, executor: &Arc<F>)
    where
        F: Fn(Uuid, String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = (bool, String)> + Send + 'static,
    {
        let now = Utc::now();

        let due: Vec<(Uuid, String)> = {
            let jobs = self.jobs.read();
            jobs.iter()
                .filter(|(_, job)| {
                    job.enabled
                        && job.next_run.is_some_and(|nr| nr <= now)
                        && !self.running_jobs.lock().contains(&job.id)
                })
                .map(|(_, job)| (job.id, job.goal.clone()))
                .collect()
        };

        for (id, goal) in due {
            self.running_jobs.lock().insert(id);
            let exec = executor.clone();
            let me = self.clone();
            tokio::spawn(async move {
                tracing::info!(%id, "Cron job triggered");
                let (success, summary) = exec(id, goal).await;
                tracing::info!(%id, success, "Cron job completed");
                me.mark_job_completed(id, success, summary).await;
            });
        }
    }

    /// Persist all jobs to disk.
    async fn persist_jobs(&self) {
        let job_list: Vec<CronJob> = {
            let jobs = self.jobs.read();
            jobs.values().cloned().collect()
        };
        if let Err(e) = self.state_store.save_json("cron", "jobs", &job_list).await {
            tracing::error!("Failed to persist cron jobs: {}", e);
        }
    }

    /// Restore jobs from disk on startup.
    pub async fn restore_jobs(&self) {
        match self.state_store.load_json::<Vec<CronJob>>("cron", "jobs").await {
            Ok(Some(job_list)) => {
                for mut job in job_list {
                    // Re-parse schedule and recompute next_run
                    match self.parse_schedule(&job.schedule) {
                        Ok(schedule) => {
                            job.next_run = self.next_fire_time(&schedule, &Utc::now());
                            self.schedules.lock().insert(job.id, schedule);
                            self.jobs.write().insert(job.id, job);
                        }
                        Err(e) => {
                            tracing::error!(job = %job.name, error = %e, "Skipping job with invalid schedule");
                        }
                    }
                }
                tracing::info!(count = self.jobs.read().len(), "Cron jobs restored");
            }
            Ok(None) => {
                tracing::info!("No saved cron jobs found");
            }
            Err(e) => {
                tracing::error!("Failed to restore cron jobs: {}", e);
            }
        }
    }

    /// Load jobs defined in config (called during startup).
    /// Config-defined jobs are only added if they don't already exist (API wins on conflict).
    pub async fn load_from_config(&self, config: &CronConfig) {
        if !config.enabled {
            tracing::info!("Cron scheduler is disabled in config");
            return;
        }

        for (name, inline) in &config.jobs {
            let schedule = inline.schedule.clone();
            let goal = inline.goal.clone();

            let job = CronJob {
                id: Uuid::new_v4(),
                name: name.clone(),
                schedule: schedule.clone(),
                goal,
                constraints: inline.constraints.clone(),
                acceptance_criteria: inline.acceptance_criteria.clone(),
                toolchain: inline.toolchain.clone(),
                priority: inline.priority,
                enabled: inline.enabled,
                last_run: None,
                next_run: None,
                run_count: 0,
                last_result: None,
                last_success: None,
                source: JobSource::Config,
            };

            // Check if a job with this name already exists (from API)
            {
                let jobs = self.jobs.read();
                if jobs.values().any(|j| j.name == *name) {
                    tracing::debug!(name = %name, "Skipping config job — already exists via API");
                    continue;
                }
            }

            if let Err(e) = self.add_job(job).await {
                tracing::error!(name = %name, error = %e, "Failed to load config job");
            } else {
                tracing::info!(name = %name, "Loaded cron job from config");
            }
        }
    }
}

impl Clone for CronScheduler {
    fn clone(&self) -> Self {
        Self {
            jobs: self.jobs.clone(),
            schedules: self.schedules.clone(),
            running_jobs: self.running_jobs.clone(),
            state_store: self.state_store.clone(),
            cancel: self.cancel.clone(),
            dirty: self.dirty.clone(),
            tick_interval_secs: self.tick_interval_secs,
        }
    }
}

// ── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    fn test_store() -> Arc<StateStore> {
        let temp_dir = tempfile::tempdir().unwrap();
        Arc::new(StateStore::new(temp_dir.path().to_path_buf()).unwrap())
    }

    #[test]
    fn test_normalize_5field() {
        assert_eq!(CronScheduler::normalize_expr("0 9 * * *"), "0 0 9 * * *");
    }

    #[test]
    fn test_normalize_6field() {
        assert_eq!(CronScheduler::normalize_expr("0 0 9 * * *"), "0 0 9 * * *");
    }

    #[test]
    fn test_normalize_7field() {
        assert_eq!(
            CronScheduler::normalize_expr("0 0 9 * * * 2026"),
            "0 0 9 * * * 2026"
        );
    }

    #[test]
    fn test_parse_valid() {
        let cs = CronScheduler::new(test_store(), 60);
        assert!(cs.parse_schedule("0 9 * * *").is_ok());
    }

    #[test]
    fn test_parse_invalid() {
        let cs = CronScheduler::new(test_store(), 60);
        assert!(cs.parse_schedule("invalid").is_err());
    }

    #[test]
    fn test_next_fire_time_daily() {
        let cs = CronScheduler::new(test_store(), 60);
        let schedule = cs.parse_schedule("0 9 * * *").unwrap();
        let now = chrono::NaiveDate::from_ymd_opt(2026, 5, 6)
            .unwrap()
            .and_hms_opt(8, 0, 0)
            .unwrap();
        let now_utc = DateTime::<Utc>::from_naive_utc_and_offset(now, Utc);
        let next = cs.next_fire_time(&schedule, &now_utc);
        assert!(next.is_some());
        let next = next.unwrap();
        assert_eq!(next.hour(), 9);
    }

    #[test]
    fn test_next_fire_time_every_15min() {
        let cs = CronScheduler::new(test_store(), 60);
        let schedule = cs.parse_schedule("*/15 * * * *").unwrap();
        let now = chrono::NaiveDate::from_ymd_opt(2026, 5, 6)
            .unwrap()
            .and_hms_opt(10, 7, 0)
            .unwrap();
        let now_utc = DateTime::<Utc>::from_naive_utc_and_offset(now, Utc);
        let next = cs.next_fire_time(&schedule, &now_utc);
        assert!(next.is_some());
        let next = next.unwrap();
        assert_eq!(next.minute(), 15);
    }

    #[test]
    fn test_add_job_computes_next_run() {
        let job = CronJob::new("test".into(), "0 9 * * *".into(), "Test goal".into());
        assert!(job.next_run.is_none()); // not computed yet
        assert!(job.enabled);
        assert_eq!(job.run_count, 0);
    }

    #[test]
    fn test_job_source_default() {
        let job = CronJob::new("test".into(), "0 9 * * *".into(), "goal".into());
        assert_eq!(job.source, JobSource::Api);
    }

    #[tokio::test]
    async fn test_add_job() {
        let store = test_store();
        let cs = CronScheduler::new(store, 60);
        let job = CronJob::new("test-job".into(), "0 9 * * *".into(), "Run me".into());
        let id = cs.add_job(job).await.unwrap();
        assert!(cs.get_job(id).is_some());
        assert_eq!(cs.list_jobs().len(), 1);
    }

    #[tokio::test]
    async fn test_remove_job() {
        let store = test_store();
        let cs = CronScheduler::new(store, 60);
        let job = CronJob::new("remove-me".into(), "0 10 * * *".into(), "Gone".into());
        let id = cs.add_job(job).await.unwrap();
        cs.remove_job(id).await.unwrap();
        assert!(cs.get_job(id).is_none());
    }

    #[tokio::test]
    async fn test_trigger_job() {
        let store = test_store();
        let cs = CronScheduler::new(store, 60);
        let job = CronJob::new("trigger-me".into(), "0 11 * * *".into(), "Goal text".into());
        let id = cs.add_job(job).await.unwrap();

        let triggered = cs.trigger_job(id).unwrap();
        assert_eq!(triggered.goal, "Goal text");
        assert!(cs.is_running(id));

        cs.mark_job_completed(id, true, "ok".into()).await;
        assert!(!cs.is_running(id));
    }

    #[tokio::test]
    async fn test_trigger_already_running() {
        let store = test_store();
        let cs = CronScheduler::new(store, 60);
        let job = CronJob::new("running".into(), "0 12 * * *".into(), "goal".into());
        let id = cs.add_job(job).await.unwrap();
        cs.trigger_job(id).unwrap();
        let result = cs.trigger_job(id);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_job() {
        let store = test_store();
        let cs = CronScheduler::new(store, 60);
        let job = CronJob::new("old-name".into(), "0 9 * * *".into(), "old goal".into());
        let id = cs.add_job(job).await.unwrap();

        cs.update_job(
            id,
            CronJobUpdate {
                name: Some("new-name".into()),
                goal: Some("new goal".into()),
                enabled: Some(false),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let updated = cs.get_job(id).unwrap();
        assert_eq!(updated.name, "new-name");
        assert_eq!(updated.goal, "new goal");
        assert!(!updated.enabled);
    }

    #[tokio::test]
    async fn test_toggle_job() {
        let store = test_store();
        let cs = CronScheduler::new(store, 60);
        let job = CronJob::new("toggle".into(), "0 9 * * *".into(), "goal".into());
        let id = cs.add_job(job).await.unwrap();
        assert!(cs.get_job(id).unwrap().enabled);

        cs.toggle_job(id, false).await.unwrap();
        assert!(!cs.get_job(id).unwrap().enabled);

        cs.toggle_job(id, true).await.unwrap();
        assert!(cs.get_job(id).unwrap().enabled);
    }

    #[tokio::test]
    async fn test_mark_completed_updates_next_run() {
        let store = test_store();
        let cs = CronScheduler::new(store, 60);
        let job = CronJob::new("comp".into(), "*/5 * * * *".into(), "goal".into());
        let id = cs.add_job(job).await.unwrap();

        let before = cs.get_job(id).unwrap().next_run;
        assert!(before.is_some());

        // Simulate time passing: set next_run to 5 minutes in the past
        let now = Utc::now();
        {
            let mut jobs = cs.jobs.write();
            if let Some(j) = jobs.get_mut(&id) {
                j.next_run = Some(now - chrono::Duration::minutes(5));
            }
        }

        cs.mark_job_completed(id, true, "ok".into()).await;
        let after = cs.get_job(id).unwrap().next_run;
        assert!(after.is_some());
        // After completion, next_run should be set to a future time (>= now)
        assert!(after.unwrap() >= now);
    }
}