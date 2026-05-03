---
name: debug
description: Systematic approach to debugging: finding root causes and fixing bugs
---

# Debugging Skill

## Overview

Good debugging is systematic, not magical. When something breaks, resist the urge to guess — follow a process. Most bugs are simpler than they appear.

## The Debugging Process

### 1. Gather Information

Before making any changes, collect facts:

- **What happened?** (actual behavior)
- **What should have happened?** (expected behavior)
- **When did it start?** (recent changes?)
- **How can it be reproduced?** (steps to repeat)
- **What environment?** (OS, versions, config)

### 2. Isolate the Problem

Reduce the search space:

- Can you reproduce it in isolation?
- Can you narrow down where it occurs?
- Can you identify which inputs trigger it?
- Is it consistent or intermittent?

### 3. Form a Hypothesis

Based on the facts:

- What's the most likely cause?
- What evidence supports or refutes this theory?
- Can you design a test to verify?

### 4. Test and Fix

- Make one change at a time
- Verify each fix works
- Ensure no regressions are introduced

### 5. Understand the Root Cause

Fix the symptom or fix the cause?

- **Symptom fix:** Makes the error go away but doesn't prevent recurrence
- **Root cause fix:** Addresses why the bug occurred in the first place

Always prefer root cause fixes when possible.

## Debugging Techniques

### Print Debugging
Quick but messy. Use sparingly.

```rust
println!("DEBUG: x = {:?}, y = {:?}", x, y);
```

For production, use structured logging instead.

### Using the Debugger
Rust supports GDB/LLDB. Basic workflow:

```bash
# Compile with debug symbols
RUSTFLAGS="-C debuginfo=2" cargo build

# Run in debugger
lldb ./target/debug/my_program
(lldb) breakpoint set --name main
(lldb) run
```

### Binary Search
When narrowing down problems:
- Halve the input or codebase
- Check which half still exhibits the issue
- Repeat until isolated

### Rubber Duck Debugging
Explain the problem out loud (or to a rubber duck). The act of articulating often reveals the issue.

### Version Control Bisect
When a regression is suspected:

```bash
git bisect start
git bisect bad  # current version is broken
git bisect good v1.0.0  # last known good version
# git will guide you through testing
```

## Common Bug Patterns

### Off-by-One Errors
Check loop boundaries and array indices carefully.

### Null/None Handling
Assume optional values can be `None`. Handle gracefully.

### Concurrency Issues
- Race conditions
- Deadlocks (A waiting for B, B waiting for A)
- Use `Mutex` or `RwLock` for shared state
- Check ordering of lock acquisition

### Async Bugs
- Forgetting to `.await`
- Blocking in async code
- Tasks not spawned properly

### Type Mismatches
- Comparing incompatible types
- Integer overflow
- Floating point precision

### Resource Leaks
- Files not closed
- Connections not released
- Memory not freed (in non-GC languages)

## When Stuck

1. **Sleep on it.** Take a break, the answer often comes after.

2. **Search.** Someone has likely encountered this before.

3. **Ask for help.** Articulate the problem clearly.

4. **Simplify.** Create a minimal reproduction case.

5. **Look at similar code.** Patterns often reveal solutions.

## Debugging Checklist

- [ ] Can you reproduce the issue consistently?
- [ ] Isolate where in the code the problem occurs
- [ ] Form a hypothesis about the root cause
- [ ] Verify your hypothesis with a test
- [ ] Fix the root cause (not just the symptom)
- [ ] Ensure all tests pass afterward
- [ ] Document what you learned
