# Guardian

## Purpose
Background daemon that periodically verifies system integrity,
monitors resources, and enforces budgets.

## Checks (every 5 minutes)

### Audit Chain Integrity
Verify the blake3 hash-chain has not been tampered with.
If broken, log a critical audit entry.

### Resource Overload
Check CPU, memory, and load average.
If overloaded, log warning and potentially throttle scheduling.

### Budget Status
Check all agent budgets.
If any agent exceeds budget, flag in audit log.

### Git Integrity
Verify the git repository is not corrupted.
If corrupted, log critical alert.

### Periodic Checkpoint
Auto-commit all pending state changes.
Tag with guardian checkpoint timestamp.

## Alerts
All check results are audit-logged.
Critical failures trigger KernelEvent broadcast.

## Implementation Note
Guardian runs as a tokio::spawn background task in the kernel.
It does NOT need agent tools — it only uses Kernel System Calls.
It is started automatically when the kernel boots.
