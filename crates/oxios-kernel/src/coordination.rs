//! Coordination primitives — multi-agent work distribution and consensus.
//!
//! Re-exports oxi-sdk 0.23.0's coordination primitives for use by the
//! orchestrator and other kernel components.
//!
//! # Available Primitives
//!
//! - **WorkQueue**: Priority-based atomic task queue with claim/complete lifecycle.
//!   Use for distributing independent subtasks across agents.
//! - **SharedMemory**: Versioned KV store with optimistic locking.
//!   Use for sharing state between agents in a group.
//! - **Consensus**: Simple majority/unanimity voting.
//!   Use for evaluation, approval, and decision-making.
//! - **CoordinatedGroup**: Fan-out, vote, and map-reduce over AgentHandles.
//!   Use for structured multi-agent workflows.
//!
//! # Usage
//!
//! ```ignore
//! use oxi_sdk::coordination::{WorkQueue, WorkQueueConfig};
//!
//! let queue = WorkQueue::new(WorkQueueConfig::default());
//! // queue.submit(WorkItem { id: "task-1".into(), .. });
//! // let item = queue.claim("agent-1");
//! // queue.complete("task-1", WorkResult { output: "...".into(), success: true });
//! ```

// Re-exports removed (#11) — use `oxi_sdk::` directly for coordination primitives.
// The types available: Consensus, CoordinatedGroup, CoordinatedGroupBuilder,
// GroupResult, MemoryEntry, MemoryEvent, MemoryKey, SharedMemory, VoteResult,
// WorkEvent, WorkItem, WorkQueue, WorkQueueConfig, WorkQueueStats, WorkResult,
// WorkStatus.
