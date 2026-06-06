# RFC-018 Memory Extraction — 진행 기록

> **시작일**: 2026-06-04
> **완료일**: 2026-06-05
> **목표**: oxios-memory에 메모리의 전부를 넣고, kernel/memory는 trait impl + re-export만 남긴다.

## 최종 구조

```
oxios-memory/memory/                   kernel/memory/
├── manager/                            mod.rs (152줄)
│   ├── mod.rs   (struct + ctor)        ├── impl MemoryStorage for StateStore
│   ├── store.rs (CRUD + search)        ├── impl MemoryGit for GitLayer
│   └── ops.rs   (tier, HNSW, pin)      ├── impl From for DreamConfig
├── dream.rs                            ├── MarkdownKnowledgeBase wrapper
├── proactive.rs                        ├── pub use oxios_memory::*
├── auto_bridge.rs  ← P2에서 이동 완료  └── pub mod auto_memory_bridge { re-export }
├── storage.rs  (MemoryStorage, MemoryGit, MarkdownSource)
├── types.rs, embedding.rs, ...
└── sqlite/     (feature-gated)

kernel/memory/auto_memory_bridge.rs → 삭제
```

## Phase 진행표

| Phase | 내용 | 상태 |
|-------|------|------|
| 0 | 사전 작업 (b.1–b.7) | ✅ |
| 1 | database.rs 분리 | ✅ |
| 2 | MemoryConfigProvider trait | ⬜ 보류 |
| 3 | SQLite 백엔드 이동 | ✅ |
| 4 | MemoryManager + store + ops 이동 | ✅ |
| 5 | dream, proactive 이동 | ✅ |
| 6 | kernel/memory 정리 | ✅ |

## 정리 작업 (Cleanup)

| # | 문제 | 해결 | 상태 |
|---|------|------|------|
| P5 | lib.rs에 MemoryManager/Dream/Proactive re-export 누락 | lib.rs에 추가 | ✅ |
| P3 | DreamConfig::from_consolidation 누락 (binary 안 돌아감) | `From<&ConsolidationConfig>` in kernel | ✅ |
| P4 | re-export 폭발 (94줄 중 50줄이 pub use) | kernel에서 필요 타입만 노출, 나머지 oxios_memory 직접 | ✅ |
| P1 | manager*.rs 3파일 분할 부자연스러움 | manager/ 디렉토리 모듈로 전환 | ✅ |
| P2 | auto_bridge 1002줄 kernel 잔존 | MarkdownSource trait → oxios-memory 이동 | ✅ |

## 검증

```
cargo build -p oxios-kernel --features sqlite-memory  ✅
cargo build -p oxios-memory --features sqlite-memory  ✅
cargo test  --workspace --exclude oxios               ✅ (0 failures)
oxios-memory: 209 tests passed
oxios-kernel: 20 tests passed
```
