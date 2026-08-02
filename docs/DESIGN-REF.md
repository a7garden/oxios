# Design System Reference

> **Pointer file** — this project follows the **Oxi Ecosystem Unified Design System**.

## Canonical document

`project-oxi/.github/DESIGN.md` (v1.0 · 2026-07-31) — the unified design system for
the oxi ecosystem (oxinot · oxipage · oxios). **Single source of truth.**

A full canonical-spec copy also lives at `web/DESIGN.md` (~1150 lines, with a 16-line
oxios migration-status header prepended). `web/UNIFIED-DESIGN.md` documents the
oxios-specific surface identity (status colors, dashboard density, chart/message tokens)
and the residual migration steps; common tokens / components / philosophy come from the
canonical document above.

## This project's adaptation

`web/UNIFIED-DESIGN.md` — oxios-specific surfaces + remaining migration.

oxios is the **most complete implementation** of the unified system:
- 3-tier token architecture fully implemented in `web/src/index.css`
- **Status colors** (APCA-optimized) — oxios dashboard measured values are the canonical source
- **Dashboard density** — `gap-2` rhythm, 3-zone sidebar+main+inspector layout
- **Chart/message tokens** (`--chart-1..5`, `--message-*`) — oxios-exclusive

## Migration status (remaining only)

3-tier tokens are done. Remaining: `dark:` literal sweep → semantic, storage key `oxios-theme`→`oxi-theme`,
editor font preset Serif removal, Geist→SUIT (10 refs in 3 files, zero in `.tsx`). See `web/UNIFIED-DESIGN.md`.
