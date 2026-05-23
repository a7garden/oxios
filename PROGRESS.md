# Progress

## Status
In Progress

## Tasks

### Phase: Exhaustive Frontend-Backend Static Analysis
- [x] Read all 29 knowledge hooks in `use-knowledge.ts`
- [x] Compare each hook against `knowledge_routes.rs` backend handlers
- [x] Read knowledge types in `types/knowledge.ts`
- [x] Analyze chat WebSocket stream (`use-chat-stream.ts` + `chat.rs`)
- [x] Analyze settings page (`settings.tsx` + `system.rs` config handlers)
- [x] Check sidebar/header/app-layout for API calls
- [x] Verify seed detail page (`$seedId.tsx` + `workspace.rs`)
- [x] Verify space detail page (`$spaceId.tsx` + `space_routes.rs` + `space.rs`)
- [x] Check all 6 knowledge components (copilot, graph, habits, chat, sidebar, info-panel)
- [x] Review WS client (`ws-client.ts`) and SSE client (`sse-client.ts`)
- [x] Verify backend config struct (`config.rs` OxiosConfig) against frontend settings
- [x] Write final report to `/tmp/final-static-analysis.md`

## Findings Summary (5 total)

| ID | Severity | Area | Issue |
|----|----------|------|-------|
| K1 | 🟡 Minor | Knowledge Search | `semantic_score` field doesn't exist in backend |
| K2 | 🟠 Wrong data | Knowledge File | Non-markdown files parsed as JSON instead of text |
| C1 | 🟠 Wrong data | Chat Stream | WS sends `OutgoingMessage`, frontend expects `StreamChunk` |
| S1 | 🟠 Wrong data | Settings | Section/field names don't match `OxiosConfig` schema |
| SP1 | 🟠 Wrong data | Space Detail | `tag`/`status` fields don't match backend `tags`/`active` |

## Files Changed
- `/tmp/final-static-analysis.md` — Full analysis report

## Notes
- No 🔴 crash-level issues found
- 29 knowledge hooks verified — all match backend except K1 (cosmetic) and K2 (edge case)
- Sidebar/Header/Layout make zero API calls (Zustand only) — clean
- Settings page is essentially non-functional against real backend config schema
- WS streaming is a placeholder — REST chat works fine
