# Follow-up: `oxios-gateway` web-dist metric test flake

> Discovered during RFC-027 migration cleanup (post-commit verification
> of `413428d` and earlier RFC-024 work).

## Symptom

`cargo test --workspace` may intermittently fail on:

```
active_web_dist::tests::swap_increments_metric_only_after_initial_publish
assertion `left == right` failed: swap must count
  left: 3
 right: 1
```

The test asserts that exactly one `swap` increments the
`oxios_web_dist_swaps_total` counter by 1. When failing, the observed
delta is 2 or 3 instead of 1.

## Root cause

`oxios_kernel::metrics` exposes a process-wide counter that the
`oxios_gateway` test suite shares with sibling tests in the same test
binary:

- `swap_returns_previous_and_updates_current` (line 181 in
  `crates/oxios-gateway/src/active_web_dist.rs`) calls `h.swap(...)`,
  which increments the counter.
- `clones_share_state` (line 189) calls `b.swap(...)`, which also
  increments the counter.

When `cargo test --lib` runs these in parallel, the count observed by
`sawp_increments_metric_only_after_initial_publish` includes the swaps
issued by its siblings, so the "delta == 1" assertion fails. The test
is not actually broken in isolation — `cargo test -p oxios-gateway
--lib swap_increments_metric` passes every time. The full
`cargo test --workspace` happened to hit an ordering that exposed the
race.

## Scope

- Introduced in `bb6224e feat(gateway,web): RFC-024 web↔daemon
  reliability` (the test is a new RFC-024 §11 metric assertion).
- Not caused by the RFC-027 migration in this commit series.
- Does not block the migration — verified passing 1324/1324 in the
  post-commit `cargo test --workspace` run.

## Suggested fix (out of scope for this commit series)

Two options, in order of preference:

1. **Process-isolated counter**: Replace the process-wide
   `oxios_kernel::metrics::get_metrics()` with a thread-local test
   counter for the test that needs strict delta semantics. This keeps
   the assertion sharp without serializing the test runner.
2. **Mutex / barrier**: Wrap the metric check in a `static` `Mutex`
   and serialize the three test cases that touch the same counter.
   Simpler but slower under `cargo test`.

The test should also be moved under `#[serial]` (the `serial_test`
crate) or similar if the project is open to adding that dependency,
which is the standard idiom in Rust test suites that touch shared
state.

## Tracking

- File: `crates/oxios-gateway/src/active_web_dist.rs:208`
- Sibling tests: lines 181 (`swap_returns_previous_and_updates_current`),
  189 (`clones_share_state`)
- Tracked as a follow-up; not in the current RFC-027 commit series.
