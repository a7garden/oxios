# Progress

## Status
In Progress

## Tasks
- [x] RFC-025 frontend review fixes (all 7 applied, tsc clean)
- [x] RFC-025 orchestrator review fixes (M1, m2, M4, m1, m5, m6, n1, n2 — all applied, build + 4 tests pass)

## Files Changed
- `surface/oxios-web/web/src/stores/chat.ts` — Web-C2 (project_id singular), Web-M4 (mount_ids JSON-array parse), Web-loadSession (project_id fallbacks), new `clearDetectedMount` action
- `surface/oxios-web/web/src/components/layout/chat-session-nav.tsx` — Web-M2 (drag-to-move syncs activeProjectId)
- `surface/oxios-web/web/src/components/mount/mount-detection-badge.tsx` — Web-m2 (clear detectedMountIds on accept/dismiss)
- `surface/oxios-web/web/src/types/index.ts` — Web-m6 (SessionDetail.agent_responses optional fields)
- `surface/oxios-web/web/src/hooks/use-mounts.ts` — Web-n4 (useRescanMount hook)
- `surface/oxios-web/web/src/routes/mounts/index.tsx` — Web-n4 (rescan RefreshCw button)
- `crates/oxios-kernel/src/orchestrator.rs` — Orchestrator RFC-025 fixes:
    - M1: project-referenced Mount activation moved before mounts/tag/context derivation; redundant path re-collect block deleted
    - m2: touch loop moved after project block (covers project-referenced Mounts)
    - M4: 6000-char hard limit on context body + 2000-char cap on project.instructions (char-boundary-safe truncation)
    - m1: order-preserving dedup via HashSet (replaces `ids.dedup()`)
    - m5: removed misleading empty `### Active Mounts` header
    - m6: `.expect()` → `.ok_or_else(anyhow)?` in execute_single_subtask
    - n1: stray `)` removed in summary render (`_{}_\n`); test assertion updated
    - n2: sticky-primary doc comment corrected

## Notes
- `npx tsc --noEmit` (filtered) reports zero errors after fixes.
- No emoji used; lucide-react icons only (RefreshCw for rescan).
- `cargo build` succeeds; `cargo test -p oxios-kernel --lib orchestrator` → 4/4 pass.
- Pre-existing `unused import: super::MountSource` warning in mount/detection.rs is unrelated (present before and after).
- Detailed findings: /tmp/fix-frontend.md and /tmp/fix-orch.md
- No commits made.
