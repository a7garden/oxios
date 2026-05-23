# Progress

## Status
In Progress

## Tasks

- [x] Cross-crate dependency analysis — analyzed all 9 workspace crates, traced imports, wrote report
- [x] Kernel directory module analysis — analyzed all 9 subdirectories under `crates/oxios-kernel/src/`, 72 .rs files, ~24K LOC

## Files Changed

- `analysis/cross-crate-deps.md` — Full cross-crate dependency report
- `analysis/dir-modules.md` — Detailed kernel directory module analysis with extraction candidates

## Notes

- CLI and Telegram channels are well-isolated (gateway-only dependency)
- Web channel is the most coupled (imports from 4 workspace crates directly)
- No circular dependencies in the workspace
- Leaf crates: ouroboros, markdown (no workspace deps)
- 5 modules are strong extraction candidates: workers (0 deps), access_manager (1 dep), program (1 dep), mcp (1 dep), capability (2 deps)
- memory/ is the largest module (6.8K LOC) with clean internal architecture — extractable with trait abstraction
- tools/ and kernel_handle/ are not extractable by design (facade and hands of the kernel)
