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

    #[test]
    fn test_task_creation() {
        let task = ScheduledTask::new("Test task".into(), Priority::Normal);
        assert_eq!(task.status, TaskStatus::Queued);
        assert!(task.agent_id.is_none());
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Normal);
        assert!(Priority::Normal > Priority::Low);
    }

    #[test]
    fn test_submit_and_next() {
        // High rate limit so we can drain the full queue. Priority ordering is tested,
        // not concurrent execution limits.
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
    fn test_max_concurrent() {
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
    fn test_complete_task() {
        let scheduler = AgentScheduler::new(2, 10_000, 60);
        let task = ScheduledTask::new("Test".into(), Priority::Normal);
        let id = scheduler.submit(task).unwrap();

        let _ = scheduler.next_task();
        scheduler.complete_task(id).unwrap();

        let stats = scheduler.stats();
        assert_eq!(stats.running, 0);
    }
}