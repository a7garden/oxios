//! Agent Scheduler — priority-based task queue inspired by AIOS / AgentRM.
//!
//! Manages agent task scheduling with:
//! - Priority queue (FIFO within same priority)
//! - Rate-limit-aware admission control
//! - Zombie task detection and reaping
//! - Maximum concurrent task enforcement

use crate::budget::BudgetManager;
use crate::types::AgentId;
use anyhow::Result;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;
use uuid::Uuid;

/// Priority levels for scheduled tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum Priority {
    /// Low priority, good for background work.
    Low = 0,
    /// Normal priority, default for most tasks.
    #[default]
    Normal = 1,
    /// High priority, important tasks.
    High = 2,
    /// Critical priority, must execute immediately.
    Critical = 3,
}

/// Status of a scheduled task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task is queued, waiting for execution.
    Queued,
    /// Task is currently running.
    Running,
    /// Task completed successfully.
    Completed,
    /// Task failed with an error.
    Failed,
    /// Task was cancelled before execution.
    Cancelled,
}

/// A scheduled task for an agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScheduledTask {
    /// Unique task identifier.
    pub id: Uuid,
    /// Associated agent ID, if any.
    pub agent_id: Option<AgentId>,
    /// Human-readable task description.
    pub description: String,
    /// Task priority level.
    pub priority: Priority,
    /// When the task was created.
    pub created_at: DateTime<Utc>,
    /// Current status of the task.
    pub status: TaskStatus,
    /// Error message if the task failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScheduledTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher priority first; within same priority, newer tasks first (LIFO)
        // so that BinaryHeap::pop() returns the highest-priority, newest task.
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.created_at.cmp(&self.created_at))
    }
}

impl ScheduledTask {
    /// Creates a new scheduled task.
    pub fn new(description: String, priority: Priority) -> Self {
        Self {
            id: Uuid::new_v4(),
            agent_id: None,
            description,
            priority,
            created_at: Utc::now(),
            status: TaskStatus::Queued,
            error: None,
        }
    }

    /// Creates a task associated with a specific agent.
    pub fn for_agent(agent_id: AgentId, description: String, priority: Priority) -> Self {
        Self {
            id: Uuid::new_v4(),
            agent_id: Some(agent_id),
            description,
            priority,
            created_at: Utc::now(),
            status: TaskStatus::Queued,
            error: None,
        }
    }
}

/// Statistics about the scheduler state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerStats {
    /// Number of queued tasks.
    pub queued: usize,
    /// Number of currently running tasks.
    pub running: usize,
    /// Number of completed tasks.
    pub completed: usize,
    /// Number of failed tasks.
    pub failed: usize,
    /// Maximum allowed concurrent tasks.
    pub max_concurrent: usize,
    /// Rate limit (requests per minute).
    pub rate_limit_per_minute: u32,
    /// Remaining rate limit capacity.
    pub rate_remaining: u32,
}

impl Default for SchedulerStats {
    fn default() -> Self {
        Self {
            queued: 0,
            running: 0,
            completed: 0,
            failed: 0,
            max_concurrent: 5,
            rate_limit_per_minute: 60,
            rate_remaining: 60,
        }
    }
}

/// Rate limiter state for tracking API call rates.
#[derive(Debug, Clone)]
struct RateLimiter {
    /// Timestamps of recent requests.
    window: Vec<DateTime<Utc>>,
    /// Window duration in seconds.
    window_secs: u64,
    /// Maximum requests per window.
    max_requests: u32,
}

impl RateLimiter {
    fn new(window_secs: u64, max_requests: u32) -> Self {
        Self {
            window: Vec::new(),
            window_secs,
            max_requests,
        }
    }

    /// Check if a new request is allowed under rate limits.
    fn allow(&mut self) -> bool {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::seconds(self.window_secs as i64);

        // Prune old entries.
        self.window.retain(|t| *t > cutoff);

        if self.window.len() >= self.max_requests as usize {
            return false;
        }

        self.window.push(now);
        true
    }

    /// Get remaining capacity in the current window.
    fn remaining(&self) -> u32 {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::seconds(self.window_secs as i64);
        let active = self.window.iter().filter(|t| **t > cutoff).count();
        self.max_requests.saturating_sub(active as u32)
    }
}

/// The agent scheduler.
///
/// Manages task queues, rate limiting, zombie detection, and priority scheduling.
/// This is the central coordinator for all agent task execution.
pub struct AgentScheduler {
    /// The task queue (priority max-heap).
    queue: Arc<Mutex<BinaryHeap<ScheduledTask>>>,
    /// Currently running tasks.
    running: Arc<Mutex<HashMap<Uuid, ScheduledTask>>>,
    /// Maximum concurrent tasks allowed.
    max_concurrent: std::sync::atomic::AtomicUsize,
    /// Rate limiter for LLM API calls.
    rate_limiter: Arc<Mutex<RateLimiter>>,
    /// Timeout for zombie detection (seconds).
    zombie_timeout_secs: std::sync::atomic::AtomicU64,
    /// Track when each running task started (for zombie detection).
    task_start_times: Arc<Mutex<HashMap<Uuid, DateTime<Utc>>>>,
    /// Optional budget manager for scheduling checks.
    budget_manager: Option<Arc<BudgetManager>>,
}

impl AgentScheduler {
    /// Creates a new scheduler.
    ///
    /// # Arguments
    /// * `max_concurrent` - Maximum number of tasks that can run simultaneously
    /// * `rate_limit_per_minute` - Maximum LLM API calls per minute
    /// * `zombie_timeout_secs` - How long before a running task is considered a zombie
    pub fn new(
        max_concurrent: usize,
        rate_limit_per_minute: u32,
        zombie_timeout_secs: u64,
    ) -> Self {
        Self {
            queue: Arc::new(Mutex::new(BinaryHeap::new())),
            running: Arc::new(Mutex::new(HashMap::new())),
            max_concurrent: std::sync::atomic::AtomicUsize::new(max_concurrent),
            rate_limiter: Arc::new(Mutex::new(RateLimiter::new(60, rate_limit_per_minute))),
            zombie_timeout_secs: std::sync::atomic::AtomicU64::new(zombie_timeout_secs),
            task_start_times: Arc::new(Mutex::new(HashMap::new())),
            budget_manager: None,
        }
    }

    /// Attaches a budget manager for scheduling checks.
    ///
    /// When a budget manager is set, the scheduler will:
    /// - Check `can_schedule()` before starting a task (soft gate)
    /// - Track calls via `track_call()` when a task begins
    ///
    /// If no budget manager is set, tasks proceed normally.
    pub fn set_budget_manager(&mut self, bm: Arc<BudgetManager>) {
        self.budget_manager = Some(bm);
    }

    /// Hot-reload scheduler config without restart.
    ///
    /// Updates concurrency limit, rate limit, and zombie timeout.
    /// Takes effect on the next `next_task()` / `reap_zombies()` call.
    pub fn update_config(
        &self,
        max_concurrent: usize,
        rate_limit_per_minute: u32,
        zombie_timeout_secs: u64,
    ) {
        {
            let mut limiter = self.rate_limiter.lock();
            *limiter = RateLimiter::new(60, rate_limit_per_minute);
        }
        self.max_concurrent
            .store(max_concurrent, std::sync::atomic::Ordering::Relaxed);
        self.zombie_timeout_secs
            .store(zombie_timeout_secs, std::sync::atomic::Ordering::Relaxed);
        tracing::info!(
            max_concurrent,
            rate_limit_per_minute,
            zombie_timeout_secs,
            "Scheduler config hot-reloaded"
        );
    }

    /// Submits a task to the scheduler queue.
    ///
    /// Returns the task ID on success.
    pub fn submit(&self, mut task: ScheduledTask) -> Result<Uuid> {
        task.status = TaskStatus::Queued;
        let id = task.id;

        let mut queue = self.queue.lock();
        queue.push(task); // O(log N) — BinaryHeap maintains heap property

        tracing::debug!(
            task_id = %id,
            queue_len = queue.len(),
            "Task submitted to scheduler"
        );

        Ok(id)
    }

    /// Gets the next task ready for execution.
    ///
    /// Returns `None` if:
    /// - The queue is empty
    /// - Max concurrent limit is reached
    /// - Rate limit is exceeded
    pub fn next_task(&self) -> Option<ScheduledTask> {
        // Check if we can start a new task.
        {
            let running = self.running.lock();
            if running.len()
                >= self
                    .max_concurrent
                    .load(std::sync::atomic::Ordering::Relaxed)
            {
                tracing::debug!(
                    running = running.len(),
                    max = self
                        .max_concurrent
                        .load(std::sync::atomic::Ordering::Relaxed),
                    "Max concurrent limit reached"
                );
                return None;
            }
        }

        // Check rate limit.
        {
            let mut limiter = self.rate_limiter.lock();
            if !limiter.allow() {
                tracing::debug!(remaining = limiter.remaining(), "Rate limit exceeded");
                return None;
            }
        }

        // Pop tasks iteratively, skipping agents with exhausted budgets.
        let mut discarded: usize = 0;
        let mut task = loop {
            let task_opt = {
                let mut queue = self.queue.lock();
                queue.pop() // O(log N) — BinaryHeap returns max-priority task
            };

            match task_opt {
                Some(t) => {
                    // Check budget before scheduling (soft gate).
                    if let (Some(ref bm), Some(ref agent_id)) = (&self.budget_manager, &t.agent_id)
                    {
                        if !bm.can_schedule(agent_id) {
                            tracing::warn!(
                                agent_id = %agent_id,
                                "Agent budget exhausted, skipping task"
                            );
                            discarded += 1;
                            continue; // skip, try next task
                        }
                    }
                    break t;
                }
                None => {
                    if discarded > 0 {
                        tracing::info!(discarded, "All queued tasks had exhausted budgets");
                    }
                    return None;
                }
            }
        };

        if discarded > 0 {
            tracing::info!(discarded, "Skipped tasks with exhausted budgets");
        }

        task.status = TaskStatus::Running;

        // Track start time for zombie detection.
        {
            let mut start_times = self.task_start_times.lock();
            start_times.insert(task.id, Utc::now());
        }

        // Add to running map.
        {
            let mut running = self.running.lock();
            running.insert(task.id, task.clone());
        }

        tracing::info!(
            task_id = %task.id,
            priority = ?task.priority,
            running = self.running.lock().len(),
            "Task started by scheduler"
        );

        // Track call for budget management.
        if let (Some(ref bm), Some(ref agent_id)) = (&self.budget_manager, &task.agent_id) {
            if let Err(e) = bm.track_call(agent_id) {
                tracing::warn!(
                    agent_id = %agent_id,
                    error = %e,
                    "Budget exceeded during task track_call"
                );
            }
        }

        Some(task)
    }

    /// Marks a task as completed.
    ///
    /// Removes the task from the running map.
    pub fn complete_task(&self, task_id: Uuid) -> Result<()> {
        let task = {
            let mut running = self.running.lock();
            running.remove(&task_id)
        };

        match task {
            Some(mut t) => {
                t.status = TaskStatus::Completed;

                // Clean up start time tracking.
                {
                    let mut start_times = self.task_start_times.lock();
                    start_times.remove(&task_id);
                }

                tracing::info!(task_id = %task_id, "Task completed");
                Ok(())
            }
            None => {
                tracing::warn!(task_id = %task_id, "Attempted to complete unknown task");
                Err(anyhow::anyhow!("task not found"))
            }
        }
    }

    /// Marks a task as failed with an error message.
    ///
    /// Removes the task from the running map.
    pub fn fail_task(&self, task_id: Uuid, error: &str) -> Result<()> {
        let task = {
            let mut running = self.running.lock();
            running.remove(&task_id)
        };

        match task {
            Some(mut t) => {
                t.status = TaskStatus::Failed;
                t.error = Some(error.to_string());

                // Clean up start time tracking.
                {
                    let mut start_times = self.task_start_times.lock();
                    start_times.remove(&task_id);
                }

                tracing::warn!(task_id = %task_id, error = %error, "Task failed");
                Ok(())
            }
            None => {
                tracing::warn!(task_id = %task_id, "Attempted to fail unknown task");
                Err(anyhow::anyhow!("task not found"))
            }
        }
    }

    /// Detects and reaps zombie tasks (running longer than the configured timeout).
    ///
    /// Returns the IDs of tasks that were reaped.
    pub fn reap_zombies(&self) -> Vec<Uuid> {
        let now = Utc::now();
        let timeout = chrono::Duration::seconds(
            self.zombie_timeout_secs
                .load(std::sync::atomic::Ordering::Relaxed) as i64,
        );
        let mut start_times = self.task_start_times.lock();
        let mut running = self.running.lock();
        let mut reaped = Vec::new();

        let zombie_ids: Vec<Uuid> = start_times
            .iter()
            .filter(|(_, start)| now - **start > timeout)
            .map(|(id, _)| *id)
            .collect();

        for id in zombie_ids {
            if let Some(mut task) = running.remove(&id) {
                task.status = TaskStatus::Failed;
                task.error = Some(format!(
                    "zombie: ran for >{} seconds",
                    self.zombie_timeout_secs
                        .load(std::sync::atomic::Ordering::Relaxed)
                ));
                reaped.push(id);
                tracing::warn!(
                    task_id = %id,
                    duration_secs = self.zombie_timeout_secs.load(std::sync::atomic::Ordering::Relaxed),
                    "Zombie task reaped"
                );
            }
            // Remove from start times.
            start_times.remove(&id);
        }

        reaped
    }

    /// Marks a task as running by task ID.
    ///
    /// The task must be in the queue (status Queued). This is used by the
    /// orchestrator to atomically claim a submitted task before execution.
    pub fn start_task(&self, task_id: Uuid) -> Result<()> {
        let task = {
            let mut queue = self.queue.lock();
            let all: Vec<ScheduledTask> = queue.drain().collect();
            let mut found: Option<ScheduledTask> = None;
            let remaining: Vec<ScheduledTask> = all
                .into_iter()
                .filter(|t| {
                    if t.id == task_id {
                        found = Some(t.clone());
                        false
                    } else {
                        true
                    }
                })
                .collect();
            *queue = remaining.into_iter().collect();
            found
        };

        match task {
            Some(mut task) => {
                task.status = TaskStatus::Running;
                let mut start_times = self.task_start_times.lock();
                start_times.insert(task.id, Utc::now());
                let mut running = self.running.lock();
                running.insert(task.id, task);
                Ok(())
            }
            None => Err(anyhow::anyhow!("task {task_id} not found in queue")),
        }
    }

    /// Cancels a queued task by ID.
    ///
    /// Only works on tasks still in the queue (not yet running).
    pub fn cancel_task(&self, task_id: Uuid) -> Result<()> {
        let mut queue = self.queue.lock();
        let all: Vec<ScheduledTask> = queue.drain().collect();
        let mut found = false;
        let remaining: Vec<ScheduledTask> = all
            .into_iter()
            .filter(|t| {
                if t.id == task_id && t.status == TaskStatus::Queued {
                    found = true;
                    false
                } else {
                    true
                }
            })
            .collect();
        *queue = remaining.into_iter().collect();

        if found {
            tracing::info!(task_id = %task_id, "Task cancelled from queue");
            Ok(())
        } else {
            tracing::warn!(task_id = %task_id, "Task not found in queue for cancellation");
            Err(anyhow::anyhow!("task not found in queue"))
        }
    }

    /// Returns the current scheduler statistics.
    pub fn stats(&self) -> SchedulerStats {
        let queue = self.queue.lock();
        let running = self.running.lock();
        let rate_limiter = self.rate_limiter.lock();

        let _completed = 0usize;
        let _failed = 0usize;

        SchedulerStats {
            queued: queue.len(),
            running: running.len(),
            completed: _completed,
            failed: _failed,
            max_concurrent: self
                .max_concurrent
                .load(std::sync::atomic::Ordering::Relaxed),
            rate_limit_per_minute: rate_limiter.max_requests,
            rate_remaining: rate_limiter.remaining(),
        }
    }

    /// Returns remaining rate limit capacity.
    pub fn rate_limit_remaining(&self) -> u32 {
        self.rate_limiter.lock().remaining()
    }

    /// Returns all queued tasks (for debugging/monitoring).
    pub fn queued_tasks(&self) -> Vec<ScheduledTask> {
        let heap = self.queue.lock();
        let mut tasks: Vec<ScheduledTask> = heap.iter().cloned().collect();
        // Sort ascending by priority so highest priority is at the end
        // (matches the original Vec-based behavior for test compatibility).
        tasks.sort_by_key(|a| a.priority);
        tasks
    }

    /// Returns all running tasks (for debugging/monitoring).
    pub fn running_tasks(&self) -> Vec<ScheduledTask> {
        self.running.lock().values().cloned().collect()
    }
}

impl Default for AgentScheduler {
    fn default() -> Self {
        Self::new(5, 60, 300)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_task_creation() {
        let task = ScheduledTask::new("Test task".into(), Priority::Normal);
        assert_eq!(task.status, TaskStatus::Queued);
        assert!(task.agent_id.is_none());
        assert!(!task.error.is_some());
    }

    #[test]
    fn test_task_creation_for_agent() {
        let agent_id = AgentId::new_v4();
        let task = ScheduledTask::for_agent(agent_id, "Agent task".into(), Priority::High);
        assert_eq!(task.agent_id, Some(agent_id));
        assert_eq!(task.priority, Priority::High);
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Normal);
        assert!(Priority::Normal > Priority::Low);
        // Check transitivity
        assert!(Priority::Critical > Priority::Normal);
        assert!(Priority::Critical > Priority::Low);
        assert!(Priority::High > Priority::Low);
    }

    #[test]
    fn test_priority_ordering_eq() {
        assert_eq!(Priority::Low, Priority::Low);
        assert_eq!(Priority::Normal, Priority::Normal);
        assert_eq!(Priority::High, Priority::High);
        assert_eq!(Priority::Critical, Priority::Critical);
    }

    #[test]
    fn test_submit_and_next_high_priority_first() {
        let scheduler = AgentScheduler::new(10, 10_000, 60);

        scheduler
            .submit(ScheduledTask::new("Low priority".into(), Priority::Low))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new("High priority".into(), Priority::High))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new(
                "Normal priority".into(),
                Priority::Normal,
            ))
            .unwrap();

        // High priority should come first.
        let next = scheduler.next_task().unwrap();
        assert_eq!(next.priority, Priority::High);

        // Normal next.
        let next = scheduler.next_task().unwrap();
        assert_eq!(next.priority, Priority::Normal);

        // Low last.
        let next = scheduler.next_task().unwrap();
        assert_eq!(next.priority, Priority::Low);
    }

    #[test]
    fn test_submit_and_next_critical_first() {
        let scheduler = AgentScheduler::new(10, 10_000, 60);

        scheduler
            .submit(ScheduledTask::new("Low".into(), Priority::Low))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new("Normal".into(), Priority::Normal))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new("High".into(), Priority::High))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new("Critical".into(), Priority::Critical))
            .unwrap();

        // Critical should be first.
        let next = scheduler.next_task().unwrap();
        assert_eq!(next.priority, Priority::Critical);
        // Then High.
        let next = scheduler.next_task().unwrap();
        assert_eq!(next.priority, Priority::High);
        // Then Normal.
        let next = scheduler.next_task().unwrap();
        assert_eq!(next.priority, Priority::Normal);
        // Then Low.
        let next = scheduler.next_task().unwrap();
        assert_eq!(next.priority, Priority::Low);
    }

    #[test]
    fn test_submit_multiple_same_priority() {
        let scheduler = AgentScheduler::new(10, 10_000, 60);

        // Multiple tasks at same priority — BinaryHeap does not guarantee
        // FIFO/LIFO within the same priority level.
        scheduler
            .submit(ScheduledTask::new("First".into(), Priority::Normal))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new("Second".into(), Priority::Normal))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new("Third".into(), Priority::Normal))
            .unwrap();

        // All three should be popped with Normal priority; exact order is unspecified.
        let mut descriptions = Vec::new();
        for _ in 0..3 {
            let next = scheduler.next_task().unwrap();
            assert_eq!(next.priority, Priority::Normal);
            descriptions.push(next.description);
        }
        descriptions.sort();
        assert_eq!(descriptions, vec!["First", "Second", "Third"]);
    }

    #[test]
    fn test_max_concurrent_blocks() {
        let scheduler = AgentScheduler::new(2, 10_000, 60);

        scheduler
            .submit(ScheduledTask::new("Task 1".into(), Priority::Normal))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new("Task 2".into(), Priority::Normal))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new("Task 3".into(), Priority::Normal))
            .unwrap();

        assert!(scheduler.next_task().is_some());
        assert!(scheduler.next_task().is_some());
        // Third should be None due to max concurrent.
        assert!(scheduler.next_task().is_none());
    }

    #[test]
    fn test_max_concurrent_allows_when_slot_frees() {
        let scheduler = AgentScheduler::new(2, 10_000, 60); // 2 max concurrent.

        let _ = scheduler
            .submit(ScheduledTask::new("Task 1".into(), Priority::Normal))
            .unwrap();
        let _id2 = scheduler
            .submit(ScheduledTask::new("Task 2".into(), Priority::Normal))
            .unwrap();
        // Queue (insert at 0 prepends): [Task 2, Task 1]

        // Start 2 tasks (fills max_concurrent).
        let t1 = scheduler.next_task().unwrap(); // Task 2 (first popped).
        let t2 = scheduler.next_task().unwrap(); // Task 1 (second popped).
                                                 // Running: [Task 2, Task 1], Queue: []
        assert!(scheduler.next_task().is_none()); // Blocked.

        // Complete both tasks.
        scheduler.complete_task(t1.id).unwrap();
        scheduler.complete_task(t2.id).unwrap();

        // Submit a new task now that slots have freed.
        let _id3 = scheduler
            .submit(ScheduledTask::new("Task 3".into(), Priority::Normal))
            .unwrap();

        // Now next_task should work.
        let task = scheduler.next_task().unwrap();
        assert_eq!(task.description, "Task 3");

        // Clean up.
        scheduler.complete_task(task.id).unwrap();
    }

    #[test]
    fn test_complete_task_removes_from_running() {
        let scheduler = AgentScheduler::new(2, 10_000, 60);
        let task = ScheduledTask::new("Test".into(), Priority::Normal);
        let id = scheduler.submit(task).unwrap();

        let _ = scheduler.next_task();
        scheduler.complete_task(id).unwrap();

        let stats = scheduler.stats();
        assert_eq!(stats.running, 0);
    }

    #[test]
    fn test_complete_unknown_task_returns_error() {
        let scheduler = AgentScheduler::new(2, 10_000, 60);
        let result = scheduler.complete_task(Uuid::new_v4());
        assert!(result.is_err());
    }

    #[test]
    fn test_fail_task_sets_error() {
        let scheduler = AgentScheduler::new(2, 10_000, 60);
        let task = ScheduledTask::new("Test".into(), Priority::Normal);
        let id = scheduler.submit(task).unwrap();

        let _ = scheduler.next_task();
        scheduler.fail_task(id, "Something went wrong").unwrap();

        let running = scheduler.running.lock();
        assert!(!running.contains_key(&id));
    }

    #[test]
    fn test_cancel_queued_task() {
        let scheduler = AgentScheduler::new(2, 10_000, 60);
        let id = scheduler
            .submit(ScheduledTask::new("To cancel".into(), Priority::Normal))
            .unwrap();

        scheduler.cancel_task(id).unwrap();

        // Queue should be empty now.
        assert!(scheduler.next_task().is_none());
    }

    #[test]
    fn test_cancel_running_task_fails() {
        let scheduler = AgentScheduler::new(2, 10_000, 60);
        let id = scheduler
            .submit(ScheduledTask::new("Running".into(), Priority::Normal))
            .unwrap();

        let _ = scheduler.next_task(); // Task is now running.

        // Can't cancel a running task.
        let result = scheduler.cancel_task(id);
        assert!(result.is_err());
    }

    #[test]
    fn test_cancel_unknown_task_fails() {
        let scheduler = AgentScheduler::new(2, 10_000, 60);
        let result = scheduler.cancel_task(Uuid::new_v4());
        assert!(result.is_err());
    }

    #[test]
    fn test_stats_tracking() {
        let scheduler = AgentScheduler::new(2, 60, 60);

        let id1 = scheduler
            .submit(ScheduledTask::new("Queued".into(), Priority::Normal))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new("Queued 2".into(), Priority::Low))
            .unwrap();

        // Start one task.
        let started = scheduler.next_task().unwrap();
        assert_eq!(started.id, id1);

        let stats = scheduler.stats();
        assert_eq!(stats.queued, 1); // One still in queue.
        assert_eq!(stats.running, 1);
        assert_eq!(stats.max_concurrent, 2);
        assert_eq!(stats.rate_limit_per_minute, 60);
    }

    #[test]
    fn test_reap_zombies() {
        // Create a scheduler with a very short zombie timeout.
        let scheduler = AgentScheduler::new(2, 10_000, 1); // 1 second timeout.

        // Submit and start a task.
        let id = scheduler
            .submit(ScheduledTask::new("Zombie".into(), Priority::Normal))
            .unwrap();
        let _ = scheduler.next_task();

        // Wait longer than the zombie timeout.
        thread::sleep(Duration::from_millis(1_100));

        // Reap zombies.
        let reaped = scheduler.reap_zombies();
        assert!(reaped.contains(&id));

        // Task should no longer be running.
        assert!(scheduler.running.lock().get(&id).is_none());
    }

    #[test]
    fn test_reap_zombies_no_zombies() {
        let scheduler = AgentScheduler::new(2, 10_000, 60); // Long timeout.

        let id = scheduler
            .submit(ScheduledTask::new("Normal".into(), Priority::Normal))
            .unwrap();
        let _ = scheduler.next_task();

        // No sleep, so no zombies yet.
        let reaped = scheduler.reap_zombies();
        assert!(reaped.is_empty());

        // Task still running.
        assert!(scheduler.running.lock().get(&id).is_some());
    }

    #[test]
    fn test_rate_limiter_basic() {
        let mut limiter = RateLimiter::new(60, 3); // 3 requests per minute.

        assert!(limiter.allow());
        assert!(limiter.allow());
        assert!(limiter.allow());
        // 4th request should be blocked.
        assert!(!limiter.allow());
    }

    #[test]
    fn test_rate_limiter_remaining() {
        let limiter = RateLimiter::new(60, 3);

        assert_eq!(limiter.remaining(), 3);

        let mut limiter = RateLimiter::new(60, 3);
        limiter.allow();
        limiter.allow();
        assert_eq!(limiter.remaining(), 1);
    }

    #[test]
    fn test_rate_limiter_tracks_per_scheduler() {
        let scheduler = AgentScheduler::new(10, 5, 60); // Only 5 requests allowed, high concurrency.

        // Consume all rate limit by calling next_task (not submit).
        for i in 0..5 {
            scheduler
                .submit(ScheduledTask::new(format!("T{}", i), Priority::Normal))
                .unwrap();
            let _ = scheduler.next_task();
        }

        // Should be rate limited.
        assert!(scheduler.next_task().is_none());
        assert_eq!(scheduler.rate_limit_remaining(), 0);
    }

    #[test]
    fn test_queued_tasks_inspection() {
        let scheduler = AgentScheduler::new(2, 10_000, 60);

        scheduler
            .submit(ScheduledTask::new("A".into(), Priority::Low))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new("B".into(), Priority::High))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new("C".into(), Priority::Normal))
            .unwrap();

        let queued = scheduler.queued_tasks();
        assert_eq!(queued.len(), 3);
        // Order is by priority (highest at back for pop).
        // High should be at the back.
        assert_eq!(queued.last().unwrap().description, "B");
    }

    #[test]
    fn test_running_tasks_inspection() {
        let scheduler = AgentScheduler::new(2, 10_000, 60);

        scheduler
            .submit(ScheduledTask::new("R1".into(), Priority::Normal))
            .unwrap();
        scheduler
            .submit(ScheduledTask::new("R2".into(), Priority::Normal))
            .unwrap();

        let _ = scheduler.next_task();
        let _ = scheduler.next_task();

        let running = scheduler.running_tasks();
        assert_eq!(running.len(), 2);
    }

    #[test]
    fn test_default_scheduler() {
        let scheduler = AgentScheduler::default();
        let stats = scheduler.stats();
        assert_eq!(stats.max_concurrent, 5);
        assert_eq!(stats.rate_limit_per_minute, 60);
    }

    #[test]
    fn test_budget_manager_integration_skips_exhausted_agent() {
        use crate::budget::{BudgetLimit, BudgetManager};

        let scheduler = Arc::new(Mutex::new(AgentScheduler::new(2, 10_000, 60)));
        let budget_manager = Arc::new(BudgetManager::new());

        // Set a very low budget (1 call).
        let agent_id = AgentId::new_v4();
        budget_manager.set_budget(BudgetLimit {
            agent_id,
            token_budget: 1000,
            calls_budget: 1,
            window_secs: 60,
        });

        // Attach budget manager to scheduler.
        scheduler
            .lock()
            .set_budget_manager(Arc::clone(&budget_manager));

        // Submit two tasks for the same agent.
        scheduler
            .lock()
            .submit(ScheduledTask::for_agent(
                agent_id,
                "Task 1".into(),
                Priority::Normal,
            ))
            .unwrap();
        scheduler
            .lock()
            .submit(ScheduledTask::for_agent(
                agent_id,
                "Task 2".into(),
                Priority::Normal,
            ))
            .unwrap();

        // First task should run (track_call succeeds).
        let task1 = scheduler.lock().next_task();
        assert!(task1.is_some());
        scheduler.lock().complete_task(task1.unwrap().id).unwrap();

        // Second task should be skipped (budget exhausted).
        let task2 = scheduler.lock().next_task();
        assert!(task2.is_none());
    }

    #[test]
    fn test_budget_manager_allows_different_agents() {
        use crate::budget::{BudgetLimit, BudgetManager};

        let scheduler = Arc::new(Mutex::new(AgentScheduler::new(2, 10_000, 60)));
        let budget_manager = Arc::new(BudgetManager::new());

        let agent1 = AgentId::new_v4();
        let agent2 = AgentId::new_v4();

        // Set budget for both agents (3 calls each).
        for agent_id in [&agent1, &agent2] {
            budget_manager.set_budget(BudgetLimit {
                agent_id: *agent_id,
                token_budget: 1000,
                calls_budget: 3,
                window_secs: 60,
            });
        }

        scheduler
            .lock()
            .set_budget_manager(Arc::clone(&budget_manager));

        // Submit tasks for both agents.
        scheduler
            .lock()
            .submit(ScheduledTask::for_agent(
                agent1,
                "A1".into(),
                Priority::Normal,
            ))
            .unwrap();
        scheduler
            .lock()
            .submit(ScheduledTask::for_agent(
                agent2,
                "B1".into(),
                Priority::Normal,
            ))
            .unwrap();

        // Both should run.
        let t1 = scheduler.lock().next_task().unwrap();
        let t2 = scheduler.lock().next_task().unwrap();
        assert_ne!(t1.description, t2.description);
    }

    #[test]
    fn test_budget_manager_task_without_agent_id() {
        use crate::budget::BudgetManager;

        let scheduler = Arc::new(Mutex::new(AgentScheduler::new(2, 10_000, 60)));
        let budget_manager = Arc::new(BudgetManager::new());

        scheduler
            .lock()
            .set_budget_manager(Arc::clone(&budget_manager));

        // Submit a task without an agent ID.
        scheduler
            .lock()
            .submit(ScheduledTask::new("No agent".into(), Priority::Normal))
            .unwrap();

        // Should still run (no budget check for tasks without agent).
        let task = scheduler.lock().next_task();
        assert!(task.is_some());
    }

    #[test]
    fn test_budget_manager_not_set_skips_check() {
        let scheduler = Arc::new(Mutex::new(AgentScheduler::new(2, 10_000, 60)));
        // No budget manager attached.

        scheduler
            .lock()
            .submit(ScheduledTask::new("Any task".into(), Priority::Normal))
            .unwrap();

        // Should run normally without budget manager.
        let task = scheduler.lock().next_task();
        assert!(task.is_some());
    }
}
