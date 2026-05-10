//! Infra API — Git, scheduler, cron, resources, events, system.

use std::sync::Arc;
use std::time::{Duration, Instant};
use crate::git_layer::{GitLayer, CommitInfo, LogEntry};
use crate::scheduler::{AgentScheduler, SchedulerStats, ScheduledTask};
use crate::cron::{CronScheduler, CronJob, CronJobUpdate};
use crate::resource_monitor::{ResourceMonitor, ResourceSnapshot};
use crate::event_bus::{EventBus, KernelEvent};
use crate::config::OxiosConfig;

/// Infrastructure system calls.
pub struct InfraApi {
    pub(crate) git_layer: Arc<GitLayer>,
    pub(crate) scheduler: Arc<AgentScheduler>,
    pub(crate) cron_scheduler: Arc<CronScheduler>,
    pub(crate) resource_monitor: Arc<ResourceMonitor>,
    pub(crate) event_bus: EventBus,
    pub(crate) config: OxiosConfig,
    pub(crate) start_time: Instant,
}

impl InfraApi {
    /// Get a reference to the GitLayer.
    pub fn git(&self) -> &GitLayer {
        &self.git_layer
    }

    /// Get commit log.
    pub fn git_log(&self, max: usize) -> anyhow::Result<Vec<LogEntry>> {
        self.git_layer.log(max)
    }

    /// Tag current state.
    pub fn git_tag(&self, name: &str, message: &str) -> anyhow::Result<()> {
        self.git_layer.tag(name, message)
    }

    /// Restore file from commit.
    pub fn git_restore(&self, path: &str, hash: &str) -> anyhow::Result<()> {
        self.git_layer.restore_file(path, hash)
    }

    /// Verify git repository integrity.
    pub fn git_verify(&self) -> anyhow::Result<bool> {
        self.git_layer.verify()
    }

    /// List git tags.
    pub fn git_tags(&self) -> anyhow::Result<Vec<String>> {
        self.git_layer.list_tags()
    }

    /// Get scheduler stats.
    pub fn scheduler_stats(&self) -> SchedulerStats {
        self.scheduler.stats()
    }

    /// Get queued tasks.
    pub fn queued_tasks(&self) -> Vec<ScheduledTask> {
        self.scheduler.queued_tasks()
    }

    /// Get running tasks.
    pub fn running_tasks(&self) -> Vec<ScheduledTask> {
        self.scheduler.running_tasks()
    }

    /// Add a cron job.
    pub async fn add_cron(&self, job: CronJob) -> anyhow::Result<uuid::Uuid> {
        self.cron_scheduler.add_job(job).await
    }

    /// Get a cron job by ID.
    pub fn get_cron(&self, id: uuid::Uuid) -> Option<CronJob> {
        self.cron_scheduler.get_job(id)
    }

    /// Update a cron job.
    pub async fn update_cron(&self, id: uuid::Uuid, update: CronJobUpdate) -> anyhow::Result<()> {
        self.cron_scheduler.update_job(id, update).await
    }

    /// Remove a cron job by ID.
    pub async fn remove_cron(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.cron_scheduler.remove_job(id).await
    }

    /// Trigger a cron job manually.
    pub fn trigger_cron(&self, id: uuid::Uuid) -> anyhow::Result<CronJob> {
        self.cron_scheduler.trigger_job(id)
    }

    /// Mark cron job completed.
    pub async fn complete_cron(&self, id: uuid::Uuid, success: bool, summary: String) {
        self.cron_scheduler.mark_job_completed(id, success, summary).await
    }

    /// List all cron jobs.
    pub fn list_crons(&self) -> Vec<CronJob> {
        self.cron_scheduler.list_jobs()
    }

    /// Get resource snapshot.
    pub fn resource_snapshot(&self) -> ResourceSnapshot {
        self.resource_monitor.snapshot()
    }

    /// Get resource history snapshots.
    pub fn resource_history(&self, last_n: usize) -> Vec<ResourceSnapshot> {
        self.resource_monitor.history(last_n)
    }

    /// Check if system is overloaded.
    pub fn is_overloaded(&self) -> bool {
        self.resource_monitor.is_overloaded()
    }

    /// Subscribe to kernel events.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<KernelEvent> {
        self.event_bus.subscribe()
    }

    /// Publish a kernel event.
    pub fn publish(&self, event: KernelEvent) -> anyhow::Result<()> {
        self.event_bus.publish(event).map_err(|e| anyhow::anyhow!("broadcast error: {e}"))
    }

    /// Get config reference.
    pub fn config(&self) -> &OxiosConfig {
        &self.config
    }

    /// Get system uptime.
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }
}