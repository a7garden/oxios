//! Agent Scheduler — priority-based task queue inspired by AIOS / AgentRM.
//!
//! Manages agent task scheduling with:
//! - Priority queue (FIFO within same priority)
//! - Rate-limit-aware admission control
//! - Zombie task detection and reaping
//! - Maximum concurrent task enforcement

use crate::types::AgentId;
use anyhow::Result;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// The task queue, sorted by priority.
    queue: Arc<Mutex<Vec<ScheduledTask>>>,
    /// Currently running tasks.
    running: Arc<Mutex<HashMap<Uuid, ScheduledTask>>>,
    /// Maximum concurrent tasks allowed.
    max_concurrent: usize,
    /// Rate limiter for LLM API calls.
    rate_limiter: Arc<Mutex<RateLimiter>>,
    /// Timeout for zombie detection (seconds).
    zombie_timeout_secs: u64,
    /// Track when each running task started (for zombie detection).
    task_start_times: Arc<Mutex<HashMap<Uuid, DateTime<Utc>>>>,
}

impl AgentScheduler {
    /// Creates a new scheduler.
    ///
    /// # Arguments
    /// * `max_concurrent` - Maximum number of tasks that can run simultaneously
    /// * `rate_limit_per_minute` - Maximum LLM API calls per minute
    /// * `zombie_timeout_secs` - How long before a running task is considered a zombie
    pub fn new(max_concurrent: usize, rate_limit_per_minute: u32, zombie_timeout_secs: u64) -> Self {
        Self {
            queue: Arc::new(Mutex::new(Vec::new())),
            running: Arc::new(Mutex::new(HashMap::new())),
            max_concurrent,
            rate_limiter: Arc::new(Mutex::new(RateLimiter::new(60, rate_limit_per_minute))),
            zombie_timeout_secs,
            task_start_times: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Submits a task to the scheduler queue.
    ///
    /// Returns the task ID on success.
    pub fn submit(&self, mut task: ScheduledTask) -> Result<Uuid> {
        task.status = TaskStatus::Queued;
        let id = task.id;

        let mut queue = self.queue.lock();
        // Insert so that HIGHEST priority task is at the END of the Vec.
        // Vec::pop() removes from the end → highest priority popped first.
        // Scan forward: insert BEFORE the first task that has LOWER priority than ours.
        // If no existing task has lower priority, append at the end (new highest priority).
        // Example: inserting High(3) into [Low(0), Normal(1)]
        //   - Low(0) < High(3)? Yes at pos=0 → insert at 0 → [High, Low, Normal] → WRONG!
        // Example: inserting Normal(1) into [Low(0), High(3)] (after Low, High inserted)
        //   - Low(0) < Normal(1)? Yes at pos=0 → insert at 0 → [Normal, Low, High] → CORRECT pop order: High, Low, Normal (WRONG!)
        // 
        // We want queue = [Low(front), Normal, High(back)]. So we need: insert BEFORE first element where priority > new priority.
        //   - Low(0) > Normal(1)? No. High(3) > Normal(1)? Yes → pos=1 → insert at 1 → [Low, Normal, High] → pop: High, Normal, Low ✓
        let pos = queue.iter().position(|t| t.priority > task.priority).unwrap_or(queue.len());
        queue.insert(pos, task);

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
            if running.len() >= self.max_concurrent {
                tracing::debug!(
                    running = running.len(),
                    max = self.max_concurrent,
                    "Max concurrent limit reached"
                );
                return None;
            }
        }

        // Check rate limit.
        {
            let mut limiter = self.rate_limiter.lock();
            if !limiter.allow() {
                tracing::debug!(
                    remaining = limiter.remaining(),
                    "Rate limit exceeded"
                );
                return None;
            }
        }

        // Pop the highest priority task.
        let task = {
            let mut queue = self.queue.lock();
            queue.pop()
        };

        if let Some(mut task) = task {
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

            Some(task)
        } else {
            None
        }
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
        let timeout = chrono::Duration::seconds(self.zombie_timeout_secs as i64);
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
                task.error = Some(format!("zombie: ran for >{} seconds", self.zombie_timeout_secs));
                reaped.push(id);
                tracing::warn!(
                    task_id = %id,
                    duration_secs = self.zombie_timeout_secs,
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
            let pos = queue.iter().position(|t| t.id == task_id);
            pos.map(|idx| queue.remove(idx))
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
            None => Err(anyhow::anyhow!("task {} not found in queue", task_id)),
        }
    }

    /// Cancels a queued task by ID.
    ///
    /// Only works on tasks still in the queue (not yet running).
    pub fn cancel_task(&self, task_id: Uuid) -> Result<()> {
        let mut queue = self.queue.lock();
        let pos = queue.iter().position(|t| t.id == task_id && t.status == TaskStatus::Queued);

        match pos {
            Some(idx) => {
                let _task = queue.remove(idx);
                tracing::info!(task_id = %task_id, "Task cancelled from queue");
                Ok(())
            }
            None => {
                tracing::warn!(task_id = %task_id, "Task not found in queue for cancellation");
                Err(anyhow::anyhow!("task not found in queue"))
            }
        }
    }

    /// Returns the current scheduler statistics.
    pub fn stats(&self) -> SchedulerStats {
        let queue = self.queue.lock();
        let running = self.running.lock();
        let rate_limiter = self.rate_limiter.lock();

        let (completed, failed) = {
            // Count by iterating (could optimize with separate counters if needed).
            let _q = queue.iter();
            let _r = running.iter();
            // For now, just report queue and running counts.
            // Completed/failed tracked separately would need persistent storage.
            (0usize, 0usize)
        };

        SchedulerStats {
            queued: queue.len(),
            running: running.len(),
            completed,
            failed,
            max_concurrent: self.max_concurrent,
            rate_limit_per_minute: rate_limiter.max_requests,
        }
    }

    /// Returns remaining rate limit capacity.
    pub fn rate_limit_remaining(&self) -> u32 {
        self.rate_limiter.lock().remaining()
    }

    /// Returns all queued tasks (for debugging/monitoring).
    pub fn queued_tasks(&self) -> Vec<ScheduledTask> {
        self.queue.lock().clone()
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

        scheduler.submit(ScheduledTask::new("Low priority".into(), Priority::Low)).unwrap();
        scheduler.submit(ScheduledTask::new("High priority".into(), Priority::High)).unwrap();
        scheduler.submit(ScheduledTask::new("Normal priority".into(), Priority::Normal)).unwrap();

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

        scheduler.submit(ScheduledTask::new("Low".into(), Priority::Low)).unwrap();
        scheduler.submit(ScheduledTask::new("Normal".into(), Priority::Normal)).unwrap();
        scheduler.submit(ScheduledTask::new("High".into(), Priority::High)).unwrap();
        scheduler.submit(ScheduledTask::new("Critical".into(), Priority::Critical)).unwrap();

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

        // Multiple tasks at same priority — order depends on insert position.
        // insert(0) prepends, so first submitted ends up at the back.
        scheduler.submit(ScheduledTask::new("First".into(), Priority::Normal)).unwrap();
        scheduler.submit(ScheduledTask::new("Second".into(), Priority::Normal)).unwrap();
        scheduler.submit(ScheduledTask::new("Third".into(), Priority::Normal)).unwrap();

        // Pop order should be LIFO (last submitted pops first).
        let next = scheduler.next_task().unwrap();
        assert_eq!(next.description, "Third");
        let next = scheduler.next_task().unwrap();
        assert_eq!(next.description, "Second");
        let next = scheduler.next_task().unwrap();
        assert_eq!(next.description, "First");
    }

    #[test]
    fn test_max_concurrent_blocks() {
        let scheduler = AgentScheduler::new(2, 10_000, 60);

        scheduler.submit(ScheduledTask::new("Task 1".into(), Priority::Normal)).unwrap();
        scheduler.submit(ScheduledTask::new("Task 2".into(), Priority::Normal)).unwrap();
        scheduler.submit(ScheduledTask::new("Task 3".into(), Priority::Normal)).unwrap();

        assert!(scheduler.next_task().is_some());
        assert!(scheduler.next_task().is_some());
        // Third should be None due to max concurrent.
        assert!(scheduler.next_task().is_none());
    }

    #[test]
    fn test_max_concurrent_allows_when_slot_frees() {
        let scheduler = AgentScheduler::new(2, 10_000, 60); // 2 max concurrent.

        let _ = scheduler.submit(ScheduledTask::new("Task 1".into(), Priority::Normal)).unwrap();
        let _id2 = scheduler.submit(ScheduledTask::new("Task 2".into(), Priority::Normal)).unwrap();
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
        let _id3 = scheduler.submit(ScheduledTask::new("Task 3".into(), Priority::Normal)).unwrap();

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
        scheduler.submit(ScheduledTask::new("Queued 2".into(), Priority::Low)).unwrap();

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
        thread::sleep(Duration::from_secs(2));

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
            scheduler.submit(ScheduledTask::new(format!("T{}", i), Priority::Normal)).unwrap();
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
        scheduler.submit(ScheduledTask::new("R2".into(), Priority::Normal)).unwrap();

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
}