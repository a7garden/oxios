# RFC-004: Oxios Knowledge System — Files.md 통합 설계

> **날짜**: 2026-05-19
> **상태**: 초안
> **관련**: RFC-001 (KernelHandle Facade), RFC-002 (Module Organization)
> **범위**: `crates/oxios-markdown/`, `kernel_handle/knowledge_api.rs`, `channels/oxios-web/`

---

## 1. 배경 및 동기

### 1.1 문제 공간

Oxios는 AI 에이전트가 실제 작업을 수행하는 OS다. 에이전트는 `MemoryManager`를 통해
구조화된 메모리(JSON)를 저장하고 검색한다. 하지만 현재 시스템에는 근본적인 간극이 있다:

**인간이 쓴 지식과 에이전트가 아는 지식이 단절되어 있다.**

- 인간은 마크다운 에디터, 옵시디언, 메모장 등으로 생각을 정리한다.
- 에이전트는 `MemoryManager`의 JSON 스토어에 기억을 저장한다.
- 이 둘은 같은 사용자의 머릿속에 있는 "같은 지식"이지만, 서로 다른 포맷으로,
  서로 다른 시스템에, 서로 다른 인터페이스로 존재한다.

### 1.2 Files.md

[files.md](https://github.com/zakirullin/files.md)는 Artem Zakirullin이 5년간 만든
오픈소스 마크다운 지식 관리 앱이다. MIT 라이선스.

핵심 철학:
- 모든 것을 plain `.md` 파일로 저장 (local-first)
- LLM-friendly (마크다운 포맷)
- "You should own your files, and the software that opens them"
- 극도로 단순한 코드 ("One person or an LLM can fit the whole project in head")
- 빌드 시스템 없음 ("in 10 years we will open /web/index.html and it should just work")

기술 스택:
- Go 백엔드 (서버, 동기화, Telegram 봇)
- JS 프론트엔드 (HyperMD 기반 WYSIWYG 에디터, PWA)
- mtime 기반 3-way merge 동기화
- 백링크, 저널, 습관 트래커

### 1.3 왜 Files.md인가

| 기준 | 옵시디언 | Files.md (Rust 포팅) |
|------|---------|---------------------|
| 오픈소스 | ❌ (코어 폐쇄) | ✅ (MIT) |
| Rust 생태계 | ❌ (Electron/JS) | ✅ (네이티브 크레이트화 가능) |
| 웹 통합 | ❌ (데스크톱 앱) | ✅ (PWA, 서버 동기화 내장) |
| 에이전트 제어 | 수동 (플러그인으로 제한적) | 완전 (파일 API 직접 제어) |
| 코드 복잡도 | 거대 (수십만 줄) | 미니멀 ("One person can fit in head") |
| 커스터마이징 | 플러그인 (JS) | 코드 수정 (Rust) |

Oxios 원칙과의 정합성:
- **Unix philosophy**: files.md의 각 모듈(fs, sync, journal, habits)은 한 가지 일만 한다.
- **No reimplementation**: files.md의 검증된 알고리즘(3-way merge, backlinks)을 재발명하지 않는다.
- **Channel agnostic**: 웹 에디터, CLI, API — 어디서든 같은 .md 파일에 접근.

### 1.4 핵심 통찰

> **Oxios는 24/7 데몬이다.** Files.md의 Rust 포팅이 이 데몬 안에서 돌면:
>
> 1. **인간**은 마크다운 에디터로 지식을 편집한다.
> 2. **에이전트**는 KnowledgeApi로 같은 지식에 접근한다.
> 3. **코파일럿**은 같은 프로세스에서 oxi 엔진을 직접 호출한다.
> 4. **파일 변경**이 실시간으로 메모리 인덱스에 반영된다.
>
> **하나의 진실 원천(single source of truth): `.md` 파일.**

---

## 2. 저작권 고려사항

### 2.1 라이선스 분석

Files.md는 **MIT License** (Copyright (c) 2023 Artem Zakirullin)를 사용한다.

MIT가 허용하는 것:
- ✅ 수정, 병합, 배포, 상업적 사용, 서브라이선스
- ✅ Go → Rust 포팅
- ✅ Oxios에 통합
- ✅ Oxios 자체의 MIT 라이선스와 완전 호환

### 2.2 의무사항

원본 저작권 고지와 MIT 라이선스 텍스트를 포함해야 한다.

**구체적 구현**:

```
crates/oxios-markdown/
├── LICENSE-THIRD-PARTY      ← "files.md by Artem Zakirullin, MIT License" 전문
├── README.md                ← 원 프로젝트 링크 및 원저자 표시
└── src/
    └── lib.rs               ← 모듈 헤더에 원출처 주석
```

각 포팅된 모듈 상단에 출처 명시:

```rust
//! LCS-based merge algorithm.
//!
//! Ported from files.md (server/sync/merge.go) by Artem Zakirullin.
//! Original: https://github.com/zakirullin/files.md
//! License: MIT — see LICENSE-THIRD-PARTY
```

### 2.3 크레이트 공개 여부

`oxios-markdown`을 crates.io에 공개하면:
- 원저자 기여가 명확히 드러남 (crates.io 메타데이터)
- 커뮤니티 기여 가능
- Oxios 생태계 독립성 확보

초기에는 workspace 내부 크레이트로 시작하고, 안정화 후 공개 검토.

---

## 3. 아키텍처 개요

### 3.1 3-레이어 모델

```
┌─────────────────────────────────────────────────────────┐
│                   Layer 3: Interface                     │
│                                                         │
│  ┌────────────────┐    ┌──────────────────────────────┐ │
│  │  Oxios Web     │    │  Knowledge Editor            │ │
│  │  Dashboard     │    │  (Files.md JS, HyperMD)      │ │
│  │  (Dioxus/WASM) │    │                              │ │
│  │  /*             │    │  /knowledge/*                │ │
│  └───────┬────────┘    └──────────────┬───────────────┘ │
│          │                             │                 │
│          │   단일 Axum 서버            │                 │
│          │   동일 인증 세션            │                 │
│          │   동일 WebSocket Hub        │                 │
└──────────┼─────────────────────────────┼─────────────────┘
           │                             │
┌──────────▼─────────────────────────────▼─────────────────┐
│                 Layer 2: Oxios Kernel                     │
│                                                          │
│  ┌──────────────────┐  ┌──────────────────────────────┐ │
│  │  KernelHandle    │  │  KnowledgeApi (NEW)           │ │
│  │  (기존 12개 API) │←─│                              │ │
│  │                  │  │  note_read / note_write        │ │
│  │  StateApi        │  │  note_search (HNSW+backlinks) │ │
│  │  AgentApi        │  │  note_link_graph              │ │
│  │  MemoryApi       │  │  copilot_chat → OxiEngine     │ │
│  │  SpaceApi        │  │  note_tree / note_delete      │ │
│  │  ExecApi         │  │  backlinks / note_move        │ │
│  │  ...             │  │                              │ │
│  └────────┬─────────┘  │  데이터 소스:                 │ │
│           │             │    .md 파일 (인간 편집)       │ │
│           │             │    MemoryManager (에이전트)   │ │
│           │             └──────────────┬───────────────┘ │
│           │                            │                 │
│  ┌────────▼────────────────────────────▼───────────────┐ │
│  │            oxios-markdown (NEW CRATE)                │ │
│  │                                                     │ │
│  │  Core (files.md Rust 포팅):                         │ │
│  │  • VirtualFs (샌드박스된 파일 I/O)                  │ │
│  │  • SyncEngine (mtime 기반 3-way merge)              │ │
│  │  • Merge (LCS conflict resolution)                  │ │
│  │  • BacklinkIndex (양방향 링크 분석)                 │ │
│  │  • MarkdownParser (링크/헤딩/체크리스트 추출)       │ │
│  │  • FuzzySearch (이름 기반 파일 검색)                │ │
│  │  • Journal / HabitTracker                           │ │
│  │                                                     │ │
│  │  Extensions (Oxios-specific, feature-gated):         │ │
│  │  • KnowledgeSync (.md ↔ MemoryManager bridge)       │ │
│  │  • GraphBridge (backlinks → MemoryGraph → PageRank) │ │
│  │  • FileWatcher (notify crate, hot re-index)         │ │
│  └─────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────┘
           │
┌──────────▼──────────────────────────────────────────────┐
│             Layer 1: Shared Storage                      │
│                                                         │
│  ~/.oxios/workspace/                                    │
│  ├── knowledge/          ← .md 파일 (인간의 1급 인터페이스)│
│  │   ├── Chat.md         ← 빠른 생각 덤프               │
│  │   ├── Later.md        ← 할 일                        │
│  │   ├── brain/          ← 아이디어 노트                │
│  │   ├── journal/        ← 일기                         │
│  │   ├── archive/        ← 보관소                       │
│  │   └── media/          ← 이미지                       │
│  ├── memory/             ← MemoryManager (에이전트 인덱스)│
│  ├── sessions/                                          │
│  ├── seeds/                                             │
│  └── programs/                                          │
│                                                         │
│  knowledge/ = 인간이 읽고 쓰는 1급 포맷                 │
│  memory/   = 에이전트가 빠르게 검색하는 인덱스           │
│  둘은 KnowledgeSync에 의해 항상 동기화됨                 │
└─────────────────────────────────────────────────────────┘
```

### 3.2 데이터 흐름

#### 인간 → .md 파일 → 에이전트

```
1. 사용자가 에디터에서 brain/Rust.md 작성/수정
2. PUT /api/knowledge/file/brain/Rust.md
3. FileWatcher(notify)가 변경 감지
4. KnowledgeSync.index_file("brain/Rust.md", content):
   a. MarkdownParser로 링크/헤딩/태그 추출
   b. BacklinkIndex 업데이트
   c. MemoryManager.remember_unique(Knowledge { content })
   d. MemoryGraph에 노드 + 엣지 추가
5. 에이전트가 knowledge_search("Rust ownership") 호출
   → HNSW 인덱스에서 brain/Rust.md 내용 반환
```

#### 에이전트 → MemoryManager → .md 파일

```
1. 에이전트가 knowledge_write("brain/NewInsight.md", content) 호출
2. KnowledgeApi:
   a. .md 파일 생성 (인간이 에디터에서 열람 가능)
   b. MemoryManager.remember(Knowledge { content })
   c. BacklinkIndex 업데이트
   d. FileWatcher에 "just written" 등록 (순환 방지)
3. 사용자가 에디터를 열면 brain/NewInsight.md가 보임
4. 사용자가 수정하면 → 다시 "인간 → 에이전트" 흐름
```

#### 순환 방지

FileWatcher는 파일 변경 이벤트의 소스를 추적한다:
- `NoteSource::Human` → FileWatcher가 재인덱싱 수행
- `NoteSource::Agent` → FileWatcher가 해당 이벤트 스킵
- 구현: in-memory `HashSet<PathBuf>`에 최근 에이전트 write 경로를 보관.
  write 후 2초 이내의 watch 이벤트는 스킵.

---

## 4. `oxios-markdown` 크레이트 상세 설계

### 4.1 Cargo.toml

```toml
[package]
name = "oxios-markdown"
version = "0.1.0"
edition = "2021"
license = "MIT"
description = "Markdown knowledge management — ported from files.md by Artem Zakirullin"

[dependencies]
tokio = { workspace = true, features = ["fs", "sync"] }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
anyhow = { workspace = true }
chrono = { workspace = true, features = ["serde"] }
parking_lot = { workspace = true }
uuid = { workspace = true, features = ["v4"] }
tracing = { workspace = true }
thiserror = { workspace = true }

# 파일 변경 감지 (optional)
notify = { version = "6", optional = true }

# Oxios 통합 (optional)
oxios-kernel = { path = "../oxios-kernel", optional = true }

[features]
default = ["watcher", "sync"]
watcher = ["dep:notify"]
sync = []
kernel = ["dep:oxios-kernel"]  # Oxios MemoryManager + MemoryGraph 통합

[dev-dependencies]
tempfile = { workspace = true }
```

### 4.2 모듈 구조

```rust
// crates/oxios-markdown/src/lib.rs

//! Markdown knowledge management library.
//!
//! Core algorithms ported from files.md (https://github.com/zakirullin/files.md)
//! by Artem Zakirullin. Licensed under MIT — see LICENSE-THIRD-PARTY.
//!
//! # Overview
//!
//! This crate provides:
//! - **VirtualFs**: Sandboxed filesystem abstraction for .md files
//! - **SyncEngine**: mtime-based 3-way merge synchronization
//! - **BacklinkIndex**: Bidirectional link tracking between notes
//! - **MarkdownParser**: Link/heading/checklist extraction from markdown
//! - **KnowledgeExtractor**: Convert freeform .md → structured insights
//!
//! With the `kernel` feature, also provides:
//! - **KnowledgeSync**: Bridge .md files ↔ Oxios MemoryManager
//! - **GraphBridge**: Backlinks → MemoryGraph → PageRank importance
//! - **FileWatcher**: Hot re-indexing on file changes
//!
//! # Example
//!
//! ```no_run
//! use oxios_markdown::VirtualFs;
//!
//! let fs = VirtualFs::new("/path/to/knowledge")?;
//! let content = fs.read("brain/Rust.md")?;
//! println!("{}", content);
//! ```

// ── Core (files.md Rust 포팅) ─────────────────────────────

pub mod fs;
pub mod sync;
pub mod merge;
pub mod backlinks;
pub mod parser;
pub mod search;
pub mod journal;
pub mod habits;
pub mod config;
pub mod types;

// ── Oxios Extensions (feature-gated) ──────────────────────

#[cfg(feature = "watcher")]
pub mod watcher;

#[cfg(feature = "kernel")]
pub mod knowledge_sync;

#[cfg(feature = "kernel")]
pub mod graph_bridge;

pub use types::*;
```

### 4.3 핵심 타입

```rust
// crates/oxios-markdown/src/types.rs

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Knowledge base 내의 단일 파일/디렉토리 항목.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteEntry {
    /// 파일명 (확장자 포함, 예: "Rust.md").
    pub name: String,
    /// 디스플레이 이름 (확장자 제외, 예: "Rust").
    pub display_name: String,
    /// 파일 내용 해시.
    pub hash: String,
    /// 생성/변경 시간 (Unix 밀리초).
    pub ctime: i64,
    /// 수정 시간 (Unix 밀리초).
    pub mtime: i64,
    /// 내용이 있는지 (비어있지 않은지).
    pub has_content: bool,
    /// 디렉토리 여부.
    pub is_dir: bool,
    /// 부모 디렉토리 경로.
    pub parent_dir: String,
}

/// 파일 변경 이벤트 소스.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteSource {
    /// 인간이 에디터에서 편집.
    Human,
    /// 에이전트가 API로 생성/수정.
    Agent,
    /// 시스템 자동 생성 (sync, import 등).
    System,
}

/// 노트 검색 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteHit {
    /// 파일 경로.
    pub path: String,
    /// 파일명.
    pub name: String,
    /// 스니펫 (검색어 주변 텍스트).
    pub snippet: String,
    /// 의미적 유사도 점수 (0.0–1.0).
    pub semantic_score: Option<f32>,
    /// 백링크 수 (구조적 중요도 힌트).
    pub backlink_count: usize,
    /// 검색어와의 이름 유사도 (0–100).
    pub name_similarity: i32,
}

/// 백링크 정보.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backlink {
    /// 링크를 포함하는 파일 경로.
    pub source_path: String,
    /// 링크가 가리키는 대상 경로.
    pub target_path: String,
    /// 링크 주변 컨텍스트 (링크 앞뒤 텍스트).
    pub context: String,
    /// 링크 텍스트.
    pub link_text: String,
}

/// 링크 그래프 (시각화용).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkGraph {
    /// 노드 목록.
    pub nodes: Vec<LinkNode>,
    /// 엣지 목록.
    pub edges: Vec<LinkEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkNode {
    pub id: String,       // 파일 경로
    pub label: String,    // 디스플레이 이름
    pub group: String,    // 디렉토리 (brain, journal 등)
    pub importance: f64,  // PageRank 점수
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkEdge {
    pub source: String,   // 파일 경로
    pub target: String,   // 파일 경로
    pub label: String,    // 링크 텍스트
}

/// 동기화 결과.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncResult {
    /// 다운로드된 파일 수.
    pub downloaded: usize,
    /// 업로드된 파일 수.
    pub uploaded: usize,
    /// 병합된 파일 수.
    pub merged: usize,
    /// 삭제된 파일 수.
    pub deleted: usize,
    /// 건너뛴 파일 수 (이미 동기화됨).
    pub skipped: usize,
}

/// 코파일럿 응답.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotResponse {
    /// 응답 텍스트.
    pub content: String,
    /// 참조된 노트 경로들.
    pub referenced_notes: Vec<String>,
    /// 참조된 메모리 엔트리 ID들.
    pub referenced_memories: Vec<String>,
}
```

### 4.4 VirtualFs

```rust
// crates/oxios-markdown/src/fs.rs

//! 샌드박스된 파일시스템 추상화.
//!
//! files.md: server/fs/fs.go 포팅.
//! 각 knowledge base는 자체 루트 디렉토리를 가지며,
//! 경로 순회(path traversal) 공격을 방지한다.

use std::path::{Path, PathBuf};

use anyhow::Result;
use tokio::fs;

use crate::types::NoteEntry;

/// Knowledge base의 파일시스템.
///
/// 루트 경로를 기준으로 샌드박스된 파일 I/O를 제공한다.
/// 모든 경로는 루트 내부에 있는지 검증된다.
pub struct VirtualFs {
    root: PathBuf,
    quota_kb: Option<i64>,
}

impl VirtualFs {
    /// 새 VirtualFs 생성.
    pub fn new(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root).await?; // TODO: remove await, use std::fs or make new() async
        Ok(Self { root, quota_kb: None })
    }

    /// 쿼터 설정 (KB 단위, None = 무제한).
    pub fn with_quota(mut self, kb: i64) -> Self {
        self.quota_kb = Some(kb);
        self
    }

    /// 파일 읽기.
    pub async fn read(&self, path: &str) -> Result<String> {
        let full = self.safe_path(path)?;
        let content = fs::read_to_string(&full).await?;
        Ok(content)
    }

    /// 파일 쓰기.
    pub async fn write(&self, path: &str, content: &str) -> Result<()> {
        let full = self.safe_path(path)?;
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(&full, content).await?;
        Ok(())
    }

    /// 파일 삭제.
    pub async fn delete(&self, path: &str) -> Result<()> {
        let full = self.safe_path(path)?;
        fs::remove_file(&full).await?;
        Ok(())
    }

    /// 파일 이름 변경/이동.
    pub async fn rename(&self, old_path: &str, new_path: &str) -> Result<()> {
        let old_full = self.safe_path(old_path)?;
        let new_full = self.safe_path(new_path)?;
        if let Some(parent) = new_full.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::rename(&old_full, &new_full).await?;
        Ok(())
    }

    /// 디렉토리 내용 조회.
    pub async fn list_dir(&self, dir: &str) -> Result<Vec<NoteEntry>> {
        // ...
    }

    /// 파일 존재 여부.
    pub async fn exists(&self, path: &str) -> Result<bool> {
        let full = self.safe_path(path)?;
        Ok(full.exists())
    }

    /// 파일 수정 시간 (mtime, Unix 밀리초).
    pub async fn mtime(&self, path: &str) -> Result<i64> {
        // ...
    }

    /// 재귀적으로 모든 .md 파일의 mtime 조회.
    pub async fn mtimes(&self) -> Result<std::collections::HashMap<String, i64>> {
        // files.md: server/fs/fs.go Mtimes() 포팅
    }

    /// 안전한 경로 검증 (path traversal 방지).
    fn safe_path(&self, relative: &str) -> Result<PathBuf> {
        let candidate = self.root.join(relative);
        let canonical_root = self.root.canonicalize()
            .unwrap_or_else(|_| self.root.clone());
        // canonicalize이 실패하면(파일이 아직 없음) root.join으로 기준
        let canonical = match candidate.canonicalize() {
            Ok(c) => c,
            Err(_) => {
                // 파일이 아직 없으면, parent를 canonicalize하고 join
                let mut c = canonical_root.clone();
                for part in relative.split('/') {
                    if part != "." && part != ".." && !part.is_empty() {
                        c = c.join(part);
                    }
                }
                // 최종 경로가 root 아래에 있는지 재확인
                let checked = c.to_string_lossy();
                let root_str = canonical_root.to_string_lossy();
                if !checked.starts_with(root_str.as_ref()) {
                    anyhow::bail!("unsafe path: {}", relative);
                }
                return Ok(c);
            }
        };
        if !canonical.starts_with(&canonical_root) {
            anyhow::bail!("unsafe path: {}", relative);
        }
        Ok(canonical)
    }
}
```

### 4.5 SyncEngine

```rust
// crates/oxios-markdown/src/sync.rs

//! mtime 기반 3-way merge 동기화 엔진.
//!
//! files.md: server/sync/sync.go 포팅.
//! 클라이언트-서버 간 .md 파일 동기화를 수행한다.
//! 충돌 시 LCS 기반 merge로 자동 해결.

use std::collections::HashMap;

use anyhow::Result;

use crate::fs::VirtualFs;
use crate::merge::merge;
use crate::types::SyncResult;

/// 단일 파일의 동기화 상태.
#[derive(Debug, Clone)]
pub struct FileSyncState {
    pub path: String,
    pub client_mtime: i64,
    pub server_mtime: i64,
    pub client_last_synced: i64,
}

/// 동기화 요청.
#[derive(Debug, Clone)]
pub struct SyncRequest {
    /// 클라이언트에서 수정된 파일들.
    pub modified: Vec<SyncFile>,
    /// 클라이언트에서 삭제된 파일 경로들.
    pub deleted: Vec<String>,
    /// 클라이언트가 알고 있는 각 디렉토리의 마지막 mtime.
    pub timestamps: HashMap<String, i64>,
}

/// 동기화할 단일 파일.
#[derive(Debug, Clone)]
pub struct SyncFile {
    pub path: String,
    pub content: String,
    pub last_modified: i64,
    pub client_last_synced: i64,
}

/// 동기화 응답.
#[derive(Debug, Clone)]
pub struct SyncResponse {
    /// 서버에서 클라이언트로 보낼 파일들.
    pub files: Vec<SyncFile>,
    /// 각 디렉토리의 최신 mtime.
    pub timestamps: HashMap<String, i64>,
    /// 이름 변경 맵 (old_path → new_path).
    pub renames: HashMap<String, String>,
}

/// 동기화 엔진.
pub struct SyncEngine {
    fs: VirtualFs,
}

impl SyncEngine {
    pub fn new(fs: VirtualFs) -> Self {
        Self { fs }
    }

    /// 전체 동기화 수행.
    ///
    /// 알고리즘 (files.md: server/sync/sync.go SyncFilenames):
    /// 1. 클라이언트 삭제를 서버에 반영
    /// 2. 클라이언트 수정 파일을 서버에 저장 (충돌 시 merge)
    /// 3. 서버의 최신 파일을 클라이언트에 응답
    /// 4. 이름 변경 로그를 응답에 포함
    pub async fn sync(&self, request: SyncRequest) -> Result<(SyncResponse, SyncResult)> {
        // ...
    }

    /// 단일 파일 동기화.
    ///
    /// 알고리즘 (files.md: server/sync/sync.go SyncFile):
    /// - 서버에 없으면 생성
    /// - 서버에 있고 클라이언트만 변경 → 서버에 저장
    /// - 서버에 있고 서버만 변경 → 서버 버전을 클라이언트에 전송
    /// - 둘 다 변경 → LCS merge
    pub async fn sync_file(&self, client_file: SyncFile) -> Result<SyncResponse> {
        // ...
    }
}
```

### 4.6 Merge 알고리즘

```rust
// crates/oxios-markdown/src/merge.rs

//! LCS 기반 텍스트 병합 알고리즘.
//!
//! files.md: server/sync/merge.go 포팅.
//! 두 버전의 텍스트에서 최장 공통 부분수열(LCS)을 찾아
//! 양쪽의 고유 내용을 모두 보존하며 병합한다.

/// 두 텍스트를 LCS 기반으로 병합한다.
///
/// 알고리즘:
/// 1. 두 입력을 줄 단위로 분리
/// 2. DP로 LCS 테이블 구성
/// 3. 백트래킹으로 병합 결과 구성
/// 4. 저널 헤더 이모지 병합 적용
pub fn merge(s1: &str, s2: &str) -> String {
    if s1.is_empty() { return s2.to_string(); }
    if s2.is_empty() { return s1.to_string(); }

    let lines1: Vec<&str> = s1.lines().collect();
    let lines2: Vec<&str> = s2.lines().collect();

    // DP 테이블
    let mut lcs = vec![vec![0; lines2.len() + 1]; lines1.len() + 1];
    for i in 1..=lines1.len() {
        for j in 1..=lines2.len() {
            if lines1[i-1] == lines2[j-1] {
                lcs[i][j] = lcs[i-1][j-1] + 1;
            } else {
                lcs[i][j] = lcs[i-1][j].max(lcs[i][j-1]);
            }
        }
    }

    let mut result = backtrack(&lines1, &lines2, &lcs, lines1.len(), lines2.len());
    result = merge_journal_headers(&result);
    result.join("\n")
}

/// 백트래킹으로 병합 결과 구성.
fn backtrack<'a>(
    l1: &[&'a str], l2: &[&'a str],
    lcs: &[Vec<i32>], i: usize, j: usize,
) -> Vec<&'a str> {
    if i == 0 && j == 0 { return vec![]; }
    if i == 0 { return [&backtrack(l1, l2, lcs, 0, j-1), l2[j-1]].concat(); }
    if j == 0 { return [&backtrack(l1, l2, lcs, i-1, 0), l1[i-1]].concat(); }
    if l1[i-1] == l2[j-1] {
        return [&backtrack(l1, l2, lcs, i-1, j-1), l1[i-1]].concat();
    }
    if lcs[i-1][j] > lcs[i][j-1] {
        [&backtrack(l1, l2, lcs, i-1, j), l1[i-1]].concat()
    } else {
        [&backtrack(l1, l2, lcs, i, j-1), l2[j-1]].concat()
    }
}

/// 연속된 저널 헤더에서 중복 이모지를 병합.
/// 예: "## 23 May, Friday 🤸" + "## 23 May, Friday 🤸🍽" → "## 23 May, Friday 🤸🍽"
fn merge_journal_headers(lines: &[&str]) -> Vec<&str> {
    // files.md: mergeEmojisInJournalHeaders 포팅
    // ...
}
```

### 4.7 BacklinkIndex

```rust
// crates/oxios-markdown/src/backlinks.rs

//! 양방향 링크 추적 시스템.
//!
//! files.md의 `[link](path.md)` 링크를 파싱하여
//! 어떤 파일이 어떤 파일을 참조하는지 추적한다.
//! 역방향(backlink) 조회를 O(1)에 수행.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use parking_lot::RwLock;

use crate::parser::extract_links;
use crate::types::Backlink;

/// 양방향 링크 인덱스.
///
/// 파일 경로 → 해당 파일이 참조하는 링크들 (forward links)
/// 파일 경로 → 해당 파일을 참조하는 링크들 (backward links)
pub struct BacklinkIndex {
    /// path → Set of target paths (forward links).
    forward: RwLock<HashMap<String, HashSet<String>>>,
    /// path → Set of source paths (backward links).
    backward: RwLock<HashMap<String, HashSet<String>>>,
    /// path → Vec of Backlink details.
    backlinks: RwLock<HashMap<String, Vec<Backlink>>>,
}

impl BacklinkIndex {
    pub fn new() -> Self {
        Self {
            forward: RwLock::new(HashMap::new()),
            backward: RwLock::new(HashMap::new()),
            backlinks: RwLock::new(HashMap::new()),
        }
    }

    /// 파일의 모든 링크를 인덱싱.
    ///
    /// 파일 내용에서 `[text](path.md)` 패턴을 파싱하여
    /// forward/backward 인덱스를 업데이트한다.
    pub fn index_file(&self, path: &str, content: &str) {
        let links = extract_links(content);

        // 기존 forward links 제거 (incremental update)
        {
            let mut fwd = self.forward.write();
            if let Some(old_targets) = fwd.remove(path) {
                let mut bwd = self.backward.write();
                let mut bl = self.backlinks.write();
                for target in &old_targets {
                    if let Some(sources) = bwd.get_mut(target) {
                        sources.remove(path);
                    }
                    bl.remove(&format!("{}→{}", path, target));
                }
            }
        }

        // 새 forward links 등록
        let targets: HashSet<String> = links.iter()
            .map(|l| l.target_path.clone())
            .collect();

        {
            let mut fwd = self.forward.write();
            fwd.insert(path.to_string(), targets.clone());
        }
        {
            let mut bwd = self.backward.write();
            let mut bl = self.backlinks.write();
            for link in &links {
                bwd.entry(link.target_path.clone())
                    .or_default()
                    .insert(path.to_string());
                bl.insert(
                    format!("{}→{}", link.source_path, link.target_path),
                    link.clone()
                );
            }
        }
    }

    /// 파일 삭제 시 인덱스에서 제거.
    pub fn remove_file(&self, path: &str) { /* ... */ }

    /// 특정 파일의 백링크(역방향 참조) 조회.
    pub fn backlinks_for(&self, path: &str) -> Vec<Backlink> { /* ... */ }

    /// 특정 파일의 순방향 링크 조회.
    pub fn forward_links_for(&self, path: &str) -> Vec<String> { /* ... */ }

    /// 전체 링크 그래프 반환 (시각화용).
    pub fn link_graph(&self) -> (Vec<String>, Vec<(String, String)>) { /* ... */ }

    /// 두 파일 간의 연결 강도 (공통 백링크 수).
    pub fn connection_strength(&self, path_a: &str, path_b: &str) -> usize { /* ... */ }
}
```

### 4.8 KnowledgeSync (Oxios 통합)

```rust
// crates/oxios-markdown/src/knowledge_sync.rs

//! .md 파일과 MemoryManager 간의 양방향 동기화.
//!
//! 파일이 변경되면 자동으로 MemoryManager에 인덱싱하고,
//! 에이전트가 knowledge_write를 호출하면 .md 파일도 함께 생성한다.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use parking_lot::RwLock;
use tokio::sync::Mutex;

use oxios_kernel::memory::{MemoryEntry, MemoryManager, MemoryType};

use crate::backlinks::BacklinkIndex;
use crate::fs::VirtualFs;
use crate::parser::extract_headings;
use crate::types::NoteSource;

/// .md ↔ MemoryManager 동기화 브릿지.
pub struct KnowledgeSync {
    /// 파일시스템.
    fs: VirtualFs,
    /// Oxios 메모리 매니저.
    memory: Arc<MemoryManager>,
    /// 백링크 인덱스.
    backlinks: Arc<BacklinkIndex>,
    /// 최근 에이전트 write 경로 (순환 방지).
    agent_writes: Mutex<HashSet<String>>,
}

impl KnowledgeSync {
    pub fn new(
        fs: VirtualFs,
        memory: Arc<MemoryManager>,
        backlinks: Arc<BacklinkIndex>,
    ) -> Self {
        Self {
            fs,
            memory,
            backlinks,
            agent_writes: Mutex::new(HashSet::new()),
        }
    }

    /// 파일을 인덱싱 (MemoryManager + BacklinkIndex 업데이트).
    pub async fn index_file(&self, path: &str, content: &str, source: NoteSource) -> Result<()> {
        // 1. BacklinkIndex 업데이트
        self.backlinks.index_file(path, content);

        // 2. MemoryManager에 지식으로 저장
        let entry = MemoryEntry {
            id: format!("note-{}", path.replace('/', "-").trim_end_matches(".md")),
            memory_type: MemoryType::Knowledge,
            content: content.to_string(),
            source: format!("knowledge:{}", match source {
                NoteSource::Human => "human",
                NoteSource::Agent => "agent",
                NoteSource::System => "system",
            }),
            session_id: None,
            tags: extract_headings(content).into_iter().take(5).collect(),
            importance: self.compute_importance(path, content),
            created_at: chrono::Utc::now(),
            accessed_at: chrono::Utc::now(),
            access_count: 0,
        };

        self.memory.remember_unique(entry).await?;

        tracing::debug!(path = %path, source = ?source, "Knowledge file indexed");
        Ok(())
    }

    /// 에이전트가 노트를 작성 (.md 파일 + MemoryManager 동시 저장).
    pub async fn write_note(
        &self,
        path: &str,
        content: &str,
    ) -> Result<()> {
        // 순환 방지: 등록
        {
            let mut writes = self.agent_writes.lock().await;
            writes.insert(path.to_string());
        }

        // .md 파일 저장
        self.fs.write(path, content).await?;

        // MemoryManager + BacklinkIndex 업데이트
        self.index_file(path, content, NoteSource::Agent).await?;

        // 순환 방지: 2초 후 제거
        let path_owned = path.to_string();
        let agent_writes = self.agent_writes.clone(); // TODO: Arc<Mutex>
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let mut writes = agent_writes.lock().await;
            writes.remove(&path_owned);
        });

        Ok(())
    }

    /// FileWatcher 호출용: 에이전트 write인지 확인.
    pub async fn is_agent_write(&self, path: &str) -> bool {
        let writes = self.agent_writes.lock().await;
        writes.contains(path)
    }

    /// 중요도 계산 (백링크 수 + 콘텐츠 길이 + 파일 위치 기반).
    fn compute_importance(&self, path: &str, content: &str) -> f32 {
        let backlinks = self.backlinks.backlinks_for(path).len() as f32;
        let length_bonus = (content.len() as f32 / 1000.0).min(1.0);
        let location_bonus = if path.starts_with("brain/") { 0.7 }
                            else if path.starts_with("journal/") { 0.5 }
                            else { 0.3 };
        (0.3 + (backlinks * 0.1).min(0.3) + length_bonus * 0.2 + location_bonus * 0.2).min(1.0)
    }

    /// 전체 knowledge/ 디렉토리를 인덱싱 (초기 로드용).
    pub async fn index_all(&self) -> Result<usize> {
        let mut count = 0;
        let entries = self.fs.list_dir("").await?;
        for entry in entries {
            if entry.is_dir {
                let sub_entries = self.fs.list_dir(&entry.name).await?;
                for sub in sub_entries {
                    if !sub.is_dir && sub.name.ends_with(".md") {
                        let path = format!("{}/{}", entry.name, sub.name);
                        let content = self.fs.read(&path).await?;
                        self.index_file(&path, &content, NoteSource::System).await?;
                        count += 1;
                    }
                }
            } else if entry.name.ends_with(".md") {
                let content = self.fs.read(&entry.name).await?;
                self.index_file(&entry.name, &content, NoteSource::System).await?;
                count += 1;
            }
        }
        tracing::info!(files = count, "Knowledge base indexed");
        Ok(count)
    }
}
```

### 4.9 GraphBridge (Oxios 통합)

```rust
// crates/oxios-markdown/src/graph_bridge.rs

//! 백링크를 MemoryGraph에 연결하여 PageRank 기반 중요도를 계산.

use std::collections::HashMap;
use std::sync::Arc;

use crate::backlinks::BacklinkIndex;
use crate::fs::VirtualFs;

/// 마크다운 백링크 → PageRank 그래프 브릿지.
///
/// 파일 간 링크를 그래프의 엣지로 모델링하고,
/// PageRank를 돌려 각 노트의 중요도를 계산한다.
pub struct GraphBridge {
    backlinks: Arc<BacklinkIndex>,
    /// path → u64 (그래프 노드 ID).
    path_to_node: HashMap<String, u64>,
    /// u64 → path (역매핑).
    node_to_path: HashMap<u64, String>,
    /// 다음 노드 ID.
    next_id: u64,
}

impl GraphBridge {
    pub fn new(backlinks: Arc<BacklinkIndex>) -> Self {
        Self {
            backlinks,
            path_to_node: HashMap::new(),
            node_to_path: HashMap::new(),
            next_id: 1,
        }
    }

    /// 파일의 노드 ID를 가져오거나 생성.
    fn get_or_create_node(&mut self, path: &str) -> u64 {
        if let Some(&id) = self.path_to_node.get(path) {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.path_to_node.insert(path.to_string(), id);
        self.node_to_path.insert(id, path.to_string());
        id
    }

    /// 전체 백링크를 그래프로 빌드하고 PageRank 계산.
    ///
    /// 반환: path → importance score.
    pub fn compute_importance(&mut self) -> HashMap<String, f64> {
        use crate::fs_to_graph; // or inline MemoryGraph

        let (nodes, edges) = self.backlinks.link_graph();
        let mut graph = crate::MemoryGraph::new();

        // 노드 등록
        for path in &nodes {
            let id = self.get_or_create_node(path);
            let _ = id; // 노드 존재 보장
        }

        // 엣지 등록 (양방향)
        for (source, target) in &edges {
            let s = self.get_or_create_node(source);
            let t = self.get_or_create_node(target);
            graph.link(s, t);
        }

        // PageRank 계산
        let scores = graph.pagerank(0.85, 30, None);

        // path → score 매핑
        scores.into_iter()
            .filter_map(|(node_id, score)| {
                self.node_to_path.get(&node_id)
                    .map(|path| (path.clone(), score))
            })
            .collect()
    }
}
```

### 4.10 FileWatcher (Oxios 통합)

```rust
// crates/oxios-markdown/src/watcher.rs

//! 파일 변경 감지 및 자동 재인덱싱.
//!
//! notify 크레이트를 사용하여 knowledge/ 디렉토리의 변경을 감지하고,
//! KnowledgeSync를 통해 자동으로 MemoryManager와 BacklinkIndex를 업데이트.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::mpsc;
use tracing;

use crate::knowledge_sync::KnowledgeSync;

/// 파일 변경 감시자.
pub struct FileWatcher {
    knowledge_dir: PathBuf,
    sync: Arc<KnowledgeSync>,
    debounce: Duration,
}

impl FileWatcher {
    pub fn new(knowledge_dir: PathBuf, sync: Arc<KnowledgeSync>) -> Self {
        Self {
            knowledge_dir,
            sync,
            debounce: Duration::from_millis(200),
        }
    }

    /// 백그라운드에서 파일 감시 시작.
    pub async fn start(&self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel::<PathBuf>(256);

        // notify 백엔드 스폰
        let watch_path = self.knowledge_dir.clone();
        std::thread::spawn(move || {
            use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event};
            let mut watcher = RecommendedWatcher::new(
                move |res: Result<Event, notify::Error>| {
                    if let Ok(event) = res {
                        for path in event.paths {
                            if path.extension().is_some_and(|e| e == "md") {
                                let _ = tx.blocking_send(path);
                            }
                        }
                    }
                },
                notify::Config::default(),
            ).unwrap();
            watcher.watch(&watch_path, RecursiveMode::Recursive).unwrap();
            std::thread::park();
        });

        // 디바운스 처리 루프
        let mut pending: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
        let mut debounce_timer = tokio::time::interval(self.debounce);

        loop {
            tokio::select! {
                Some(path) = rx.recv() => {
                    pending.insert(path);
                }
                _ = debounce_timer.tick() => {
                    if pending.is_empty() { continue; }
                    let paths: Vec<PathBuf> = pending.drain().collect();
                    for path in paths {
                        self.process_change(&path).await;
                    }
                }
            }
        }
    }

    /// 변경된 파일 처리.
    async fn process_change(&self, path: &PathBuf) {
        let relative = path.strip_prefix(&self.knowledge_dir)
            .unwrap_or(path);
        let path_str = relative.to_string_lossy();

        // 에이전트 write인지 확인 (순환 방지)
        if self.sync.is_agent_write(&path_str).await {
            tracing::debug!(path = %path_str, "Skipping agent-written file");
            return;
        }

        match tokio::fs::read_to_string(path).await {
            Ok(content) => {
                if let Err(e) = self.sync.index_file(&path_str, &content, NoteSource::Human).await {
                    tracing::warn!(path = %path_str, error = %e, "Failed to index changed file");
                }
            }
            Err(_) => {
                // 파일이 삭제된 경우
                tracing::debug!(path = %path_str, "File deleted, removing from index");
                // TODO: self.sync.remove_file(&path_str).await
            }
        }
    }
}
```

---

## 5. KernelHandle 통합: KnowledgeApi

### 5.1 API 정의

```rust
// kernel_handle/knowledge_api.rs

//! KnowledgeApi — KernelHandle의 13번째 API 도메인.
//!
//! 마크다운 지식 베이스에 대한 모든 접근을 제공한다.
//! .md 파일 I/O, 백링크, 검색, 코파일럿을 통합.

use std::sync::Arc;

use anyhow::Result;
use parking_lot::RwLock;

use crate::engine::EngineProvider;
use crate::memory::MemoryManager;
use oxios_markdown::{
    BacklinkIndex, CopilotResponse, GraphBridge, KnowledgeSync, LinkGraph,
    NoteEntry, NoteHit, NoteSource, VirtualFs,
};

/// 마크다운 지식 베이스 API.
pub struct KnowledgeApi {
    /// 파일시스템.
    fs: Arc<VirtualFs>,
    /// 메모리 매니저.
    memory: Arc<MemoryManager>,
    /// 백링크 인덱스.
    backlinks: Arc<BacklinkIndex>,
    /// 동기화 브릿지.
    sync: Arc<KnowledgeSync>,
    /// 그래프 브릿지 (PageRank).
    graph: RwLock<GraphBridge>,
    /// LLM 엔진 (코파일럿용).
    engine: Arc<dyn EngineProvider>,
    /// 기본 모델.
    default_model: String,
}

impl KnowledgeApi {
    /// 새 KnowledgeApi 생성.
    pub fn new(
        knowledge_dir: std::path::PathBuf,
        memory: Arc<MemoryManager>,
        engine: Arc<dyn EngineProvider>,
        default_model: String,
    ) -> Self {
        let fs = Arc::new(VirtualFs::new(knowledge_dir).expect("Failed to create VirtualFs"));
        let backlinks = Arc::new(BacklinkIndex::new());
        let sync = Arc::new(KnowledgeSync::new(
            (*fs).clone(),
            memory.clone(),
            backlinks.clone(),
        ));
        let graph = RwLock::new(GraphBridge::new(backlinks.clone()));

        Self { fs, memory, backlinks, sync, graph, engine, default_model }
    }

    // ── 파일 I/O ───────────────────────────────────────────

    /// 노트 읽기.
    pub async fn note_read(&self, path: &str) -> Result<Option<String>> {
        match self.fs.read(path).await {
            Ok(content) => Ok(Some(content)),
            Err(_) => Ok(None),
        }
    }

    /// 노트 쓰기.
    pub async fn note_write(
        &self, path: &str, content: &str, source: NoteSource,
    ) -> Result<()> {
        self.fs.write(path, content).await?;
        self.sync.index_file(path, content, source).await?;
        Ok(())
    }

    /// 노트 삭제.
    pub async fn note_delete(&self, path: &str) -> Result<()> {
        self.fs.delete(path).await?;
        self.backlinks.remove_file(path);
        // TODO: MemoryManager에서도 해당 지식 제거
        Ok(())
    }

    /// 노트 이동/이름 변경.
    pub async fn note_move(&self, old_path: &str, new_path: &str) -> Result<()> {
        self.fs.rename(old_path, new_path).await?;
        self.backlinks.remove_file(old_path);
        let content = self.fs.read(new_path).await?;
        self.sync.index_file(new_path, &content, NoteSource::System).await?;
        Ok(())
    }

    /// 파일 트리 조회.
    pub async fn note_tree(&self, dir: &str) -> Result<Vec<NoteEntry>> {
        self.fs.list_dir(dir).await
    }

    // ── 검색 ───────────────────────────────────────────────

    /// 통합 검색 (의미 + 백링크 + 이름).
    pub async fn note_search(&self, query: &str, limit: usize) -> Result<Vec<NoteHit>> {
        let mut hits = Vec::new();

        // 1. 의미적 검색 (MemoryManager → HNSW)
        let semantic = self.memory.search(query, None, limit).await
            .unwrap_or_default();
        for entry in semantic {
            if entry.source.starts_with("knowledge:") {
                hits.push(NoteHit {
                    path: entry.id.trim_start_matches("note-").replace("-", "/") + ".md",
                    name: entry.id.clone(),
                    snippet: entry.content.chars().take(200).collect(),
                    semantic_score: Some(entry.importance),
                    backlink_count: 0,
                    name_similarity: 0,
                });
            }
        }

        // 2. 이름 기반 fuzzy search
        let by_name = self.fs.search_by_name(query).await
            .unwrap_or_default();
        for entry in by_name {
            // 이름 검색 결과가 이미 semantic에 있으면 스킵
            if hits.iter().any(|h| h.path == entry.name) { continue; }
            hits.push(NoteHit {
                path: entry.display_name.clone(),
                name: entry.display_name.clone(),
                snippet: String::new(),
                semantic_score: None,
                backlink_count: 0,
                name_similarity: 100,
            });
        }

        // 3. 백링크 기반 보강
        for hit in &mut hits {
            hit.backlink_count = self.backlinks.backlinks_for(&hit.path).len();
        }

        hits.truncate(limit);
        Ok(hits)
    }

    // ── 백링크 & 그래프 ────────────────────────────────────

    /// 백링크 조회.
    pub fn backlinks_for(&self, path: &str) -> Vec<crate::types::Backlink> {
        self.backlinks.backlinks_for(path)
    }

    /// 링크 그래프 (시각화용).
    pub fn link_graph(&self) -> LinkGraph {
        let mut graph = self.graph.write();
        let importance = graph.compute_importance();
        let (nodes, edges) = self.backlinks.link_graph();

        LinkGraph {
            nodes: nodes.iter().map(|path| {
                let importance = importance.get(path).copied().unwrap_or(0.0);
                LinkNode {
                    id: path.clone(),
                    label: path.trim_end_matches(".md")
                        .rsplit('/').next().unwrap_or(path).to_string(),
                    group: path.split('/').next().unwrap_or("").to_string(),
                    importance,
                }
            }).collect(),
            edges: edges.into_iter().map(|(source, target)| {
                LinkEdge {
                    source,
                    target,
                    label: String::new(), // TODO: 링크 텍스트 포함
                }
            }).collect(),
        }
    }

    // ── 코파일럿 ───────────────────────────────────────────

    /// 코파일럿 질의응답.
    ///
    /// 현재 편집 컨텍스트 + 관련 노트 + 관련 기억을 조합하여
    /// oxi 엔진으로 질의응답을 수행한다.
    pub async fn copilot_chat(
        &self,
        question: &str,
        context_path: Option<&str>,
    ) -> Result<CopilotResponse> {
        let mut context_parts = Vec::new();

        // 1. 현재 편집 중인 파일
        if let Some(path) = context_path {
            if let Ok(Some(content)) = self.note_read(path).await {
                context_parts.push(format!("## Current file: {}\n\n{}", path, content));
            }
        }

        // 2. 관련 노트 검색
        let related = self.note_search(question, 5).await.unwrap_or_default();
        for hit in &related {
            if let Ok(Some(content)) = self.note_read(&hit.path).await {
                let snippet: String = content.chars().take(500).collect();
                context_parts.push(format!("## Related: {}\n\n{}", hit.path, snippet));
            }
        }

        // 3. 관련 메모리 검색
        let memories = self.memory.search(question, None, 3).await.unwrap_or_default();
        for mem in &memories {
            context_parts.push(format!(
                "## Memory [{}]: {}",
                mem.memory_type.label(),
                mem.content.chars().take(300).collect::<String>()
            ));
        }

        // 4. 시스템 프롬프트
        let system_prompt = format!(
            "You are a knowledge assistant embedded in a markdown editor. \
             Answer questions about the user's notes and help them think deeply. \
             Reference specific notes when relevant. Respond in the same language as the question.\n\n\
             ## User's knowledge context:\n\n{}",
            context_parts.join("\n\n")
        );

        // 5. oxi 엔진 호출
        let provider = self.engine.create_provider("anthropic")?;
        let model = self.engine.resolve_model(&self.default_model)?;
        let response = provider.chat(&system_prompt, question, &model).await?;

        Ok(CopilotResponse {
            content: response,
            referenced_notes: related.iter().map(|h| h.path.clone()).collect(),
            referenced_memories: memories.iter().map(|m| m.id.clone()).collect(),
        })
    }
}
```

### 5.2 KernelHandle에 통합

```rust
// kernel_handle/mod.rs에 추가:

pub mod knowledge_api;
pub use knowledge_api::KnowledgeApi;

// KernelHandle struct에 필드 추가:
pub struct KernelHandle {
    // 기존 12개 API...
    pub knowledge: KnowledgeApi,  // NEW: 13번째 API
}
```

### 5.3 Agent Tool 등록

```rust
// tools/kernel_bridge.rs의 tool_names()에 추가:
"knowledge_read",
"knowledge_write",
"knowledge_search",
"knowledge_backlinks",

// tools/kernel/knowledge_tool.rs (NEW):
// 각 툴은 KnowledgeApi의 메서드를 래핑.

/// `knowledge_search` 툴 — 에이전트가 지식 베이스를 검색.
pub struct KnowledgeSearchTool { /* ... */ }

impl AgentTool for KnowledgeSearchTool {
    fn name(&self) -> &str { "knowledge_search" }
    fn description(&self) -> &'static str {
        "Search the user's knowledge base (markdown notes). \
         Returns relevant notes with snippets and semantic scores."
    }
    // ...
}
```

---

## 6. Web UI 통합

### 6.1 아키텍처 결정: 하이브리드 프론트엔드

**결정**: 단일 Axum 서버에서 Dioxus 대시보드와 Files.md JS 에디터를 모두 서빙.

근거:
1. **Copilot은 같은 프로세스에서 oxi 엔진을 직접 호출** — 외부 API가 아닌 in-process 호출. 지연 없음.
2. **24/7 데몬 = 상시 인덱싱** — 파일 변경 시 백그라운드에서 자동 임베딩.
3. **인증 하나** — 같은 쿠키/토큰.
4. **WYSIWYG 에디터를 Rust/WASM으로 재작성하는 건 비현실적** — HyperMD/CodeMirror는 수 년간 다듬어진 소프트웨어.
5. **files.md 철학 유지** — "no build systems" JS 코드를 그대로 사용.

### 6.2 라우트 구성

```rust
// channels/oxios-web/src/routes/mod.rs에 추가:

// Knowledge API routes
.route("/api/knowledge/tree", get(handle_knowledge_tree))
.route("/api/knowledge/file/*path", get(handle_knowledge_file_get))
.route("/api/knowledge/file/*path", put(handle_knowledge_file_put))
.route("/api/knowledge/file/*path", delete(handle_knowledge_file_delete))
.route("/api/knowledge/search", post(handle_knowledge_search))
.route("/api/knowledge/backlinks", get(handle_knowledge_backlinks))
.route("/api/knowledge/graph", get(handle_knowledge_graph))
.route("/api/knowledge/copilot", post(handle_copilot_chat))
.route("/ws/copilot", get(handle_copilot_ws))
```

### 6.3 Files.md JS 에디터 임베드

```
channels/oxios-web/
├── static/
│   └── knowledge/           ← files.md web/ 내용
│       ├── index.html       ← files.md의 index.html (수정: API 엔드포인트 변경)
│       ├── editor.js        ← 그대로
│       ├── app.js           ← API 경로 수정 (/api/knowledge/file/*)
│       ├── chat.js          ← Copilot → /api/knowledge/copilot 로 연결
│       ├── files.js         ← 그대로
│       ├── app.css          ← 그대로
│       ├── chat.css         ← 그대로
│       └── lib/             ← CodeMirror, HyperMD 등 그대로
├── src/
│   └── routes/
│       └── knowledge_routes.rs  ← NEW: Knowledge API 핸들러
└── frontend/
    └── src/
        └── views/
            └── knowledge.rs     ← Dioxus에서 /knowledge 링크만 제공
```

**핵심 변경점**: files.md의 `app.js`에서 API 호출 경로를 Oxios API로 변경:

```javascript
// 기존 files.md:
// const API_HOST = "https://api.files.md";

// Oxios 통합:
const API_HOST = "";  // same origin
const API_BASE = "/api/knowledge";

// 파일 읽기: GET /api/knowledge/file/{path}
// 파일 쓰기: PUT /api/knowledge/file/{path}
// 파일 목록: GET /api/knowledge/tree?dir={dir}
// 검색: POST /api/knowledge/search
// 코파일럿: POST /api/knowledge/copilot
```

### 6.4 네비게이션 통합

Dioxus 대시보드의 사이드바에 "Knowledge" 링크를 추가한다.
클릭하면 `/knowledge/`로 이동 → files.md 에디터가 로드됨.
에디터 안에서 코파일럿 채팅이 가능 (WebSocket).

```rust
// channels/oxios-web/frontend/src/components/sidebar.rs
// 기존 네비게이션 항목들에 추가:
"Knowledge" → href = "/knowledge/"
```

### 6.5 나중에 점진적 Dioxus 전환 (optional)

에디터를 제외한 UI 영역을 Dioxus 컴포넌트로 만들 수 있다:
- 사이드바 파일 트리 → Dioxus 컴포넌트
- 검색 결과 패널 → Dioxus 컴포넌트
- 백링크 패널 → Dioxus 컴포넌트
- 에디터 영역만 JS (CodeMirror 6 in a web component or iframe)

하지만 이건 Phase 5+에서 검토. 초기에는 files.md JS를 그대로 쓰는 게 실용적.

---

## 7. Space × Knowledge Base

### 7.1 Space-scoped Knowledge

각 Space는 자체 `knowledge/` 디렉토리를 가진다:

```
~/.oxios/workspace/spaces/
├── {default-space}/
│   ├── knowledge/       ← 일상/개인 지식
│   │   ├── Chat.md
│   │   ├── brain/
│   │   └── journal/
│   └── memory/
├── {rust-project-space}/
│   ├── knowledge/       ← Rust 프로젝트 관련 지식
│   │   └── brain/
│   │       ├── Ownership.md
│   │       └── Async.md
│   └── memory/
└── {work-space}/
    ├── knowledge/       ← 업무 지식
    └── memory/
```

### 7.2 KnowledgeApi의 Space 인식

```rust
impl KnowledgeApi {
    /// Space-scoped KnowledgeApi 생성.
    pub fn for_space(
        space_dir: &std::path::Path,
        memory: Arc<MemoryManager>,
        engine: Arc<dyn EngineProvider>,
        model: String,
    ) -> Self {
        let knowledge_dir = space_dir.join("knowledge");
        Self::new(knowledge_dir, memory, engine, model)
    }
}
```

### 7.3 KnowledgeBridge 실제 구현

현재 `knowledge_bridge.rs`의 `reference()` / `transfer()`는 스텁이다.
KnowledgeApi가 도입되면 이 스텁을 실제로 구현할 수 있다:

```rust
impl KnowledgeBridge {
    /// 다른 Space의 지식을 참조 (검색).
    pub async fn reference(
        &self,
        from_space_id: SpaceId,
        to_space_id: SpaceId,
        query: &str,
    ) -> Result<Vec<NoteHit>> {
        // 1. from_space의 KnowledgeApi로 검색
        // 2. 권한 확인 (knowledge_visible)
        // 3. AuditTrail에 기록
        // 4. 결과 반환
    }

    /// 한 Space의 지식을 다른 Space로 복사.
    pub async fn transfer(
        &self,
        from_space_id: SpaceId,
        to_space_id: SpaceId,
        paths: &[String],
    ) -> Result<usize> {
        // 1. from_space에서 파일 읽기
        // 2. to_space에 파일 쓰기 (충돌 시 merge)
        // 3. 양쪽 MemoryManager 업데이트
        // 4. AuditTrail에 기록
    }
}
```

---

## 8. 데이터 흐름 시나리오

### 8.1 시나리오: 사용자가 노트를 작성하고 에이전트가 활용

```
1. 사용자가 /knowledge/ 에디터를 열음
2. brain/OxiosDesign.md 작성:
   "# Oxios Design
   
   Oxios는 AI 에이전트 OS다...
   
   See [Architecture](brain/Architecture.md)"
   
3. PUT /api/knowledge/file/brain/OxiosDesign.md
4. FileWatcher 감지 → KnowledgeSync.index_file():
   a. BacklinkIndex: brain/OxiosDesign.md → brain/Architecture.md 링크 등록
   b. MemoryManager: Knowledge 엔트리 저장 + HNSW 인덱스 업데이트
   c. GraphBridge: 노드 추가, PageRank 재계산

5. 시간이 흐름... 사용자가 다른 에이전트 작업을 시작

6. 에이전트가 작업 중 "사용자가 Oxios 설계에 대해 뭘 알고 있나?" 궁금
7. knowledge_search("Oxios design architecture") 호출
8. KnowledgeApi:
   - HNSW에서 brain/OxiosDesign.md 검색 → hit (score: 0.87)
   - BacklinkIndex에서 관련 노트 → brain/Architecture.md
9. 에이전트가 사용자의 설계 철학을 이해하고 더 나은 응답 생성
```

### 8.2 시나리오: 에이전트가 새 지식을 발견하고 사용자가 확인

```
1. 에이전트가 코드 리뷰 중 새로운 패턴 발견:
   "Rust에서는 orphan rule 때문에 foreign type에 대한 trait impl이 제한된다"

2. knowledge_write("brain/OrphanRule.md", content) 호출
3. .md 파일 생성 + MemoryManager 저장

4. 사용자가 /knowledge/ 열람
5. brain/OrphanRule.md가 새로 생겨 있음
6. 사용자가 열어보고 내용을 보완/수정
7. 수정 → 다시 인덱싱 → 에이전트의 다음 검색에 반영
```

### 8.3 시나리오: 코파일럿

```
1. 사용자가 brain/RustOwnership.md 편집 중
2. 코파일럿 패널에 질문: "move와 Copy trait의 차이가 뭐야?"
3. /api/knowledge/copilot 호출:
   - context_path = "brain/RustOwnership.md"
   - question = "move와 Copy trait의 차이"
4. KnowledgeApi.copilot_chat():
   a. 현재 파일 내용 로드
   b. note_search("move Copy trait") → brain/RustOwnership.md, brain/OrphanRule.md
   c. memory_search("move Copy trait") → 과거 대화 기록
   d. 전체 컨텍스트 조합 → OxiEngine.chat()
   e. 응답: "move는 소유권 이전, Copy는 bitwise 복사...
      참고: 당신의 OrphanRule 노트에서도 관련 내용을 다루고 있습니다"
5. 사용자가 [삽입] 클릭 → 현재 에디터에 응답 텍스트 삽입
6. 또는 [새 노트] 클릭 → brain/MoveVsCopy.md로 저장
```

---

## 9. 구현 로드맵

### Phase 0: Files.md → Rust 포팅 (현재 진행 중)

**목표**: 독립적으로 동작하는 Rust 라이브러리.

포팅 대상:
- `server/fs/fs.go` → `fs.rs` (VirtualFs)
- `server/sync/merge.go` → `merge.rs` (LCS merge)
- `server/sync/sync.go` → `sync.rs` (SyncEngine)
- `server/fs/fs.go` SearchFilesByName → `search.rs` (FuzzySearch)
- `server/journal/` → `journal.rs`
- `server/habits/` → `habits.rs`
- `server/pkg/txt/md.go` → `parser.rs` (MarkdownParser)

**완료 기준**: `cargo test` 통과, 기존 Go 테스트와 동등한 커버리지.

### Phase 1: `oxios-markdown` 크레이트 추출

**목표**: Oxios workspace 내에 독립 크레이트로 정리.

작업:
- [x] 포팅된 Rust 코드를 `crates/oxios-markdown/`으로 이동
- [ ] `Cargo.toml` 설정 (workspace.dependencies, features)
- [ ] `LICENSE-THIRD-PARTY` 작성
- [ ] 모듈 공개 API 정리 (pub, docs)
- [ ] `kernel` feature 구현 (KnowledgeSync, GraphBridge, FileWatcher)
- [ ] `oxios-kernel/Cargo.toml`에 `oxios-markdown` 의존성 추가

**완료 기준**: `cargo test --workspace` 통과, `oxios-markdown` 독립 컴파일.

### Phase 2: KernelHandle에 KnowledgeApi 추가

**목표**: 에이전트가 knowledge 툴을 사용할 수 있게.

작업:
- [ ] `kernel_handle/knowledge_api.rs` 구현
- [ ] `KernelHandle` struct에 `knowledge` 필드 추가
- [ ] `tools/kernel/knowledge_tool.rs` 구현
- [ ] `tools/kernel_bridge.rs`에 tool_names() 추가
- [ ] `tools/registration.rs`에 knowledge 툴 등록
- [ ] 통합 테스트

**완료 기준**: `oxios run --json "내 brain 노트에서 Rust에 대해 검색해줘"` 동작.

### Phase 3: Web UI 통합

**목표**: 브라우저에서 마크다운 에디터 사용 가능.

작업:
- [ ] files.md `web/` → `channels/oxios-web/static/knowledge/` 복사
- [ ] API 경로 수정 (app.js, files.js)
- [ ] `routes/knowledge_routes.rs` 구현
- [ ] Axum 라우트에 `/knowledge/*` 추가
- [ ] `/api/knowledge/*` 라우트 추가
- [ ] 인증 미들웨어 공유
- [ ] 사이드바에 "Knowledge" 링크 추가
- [ ] E2E 테스트

**완료 기준**: 브라우저에서 `/knowledge/` 열면 files.md 에디터 동작.

### Phase 4: 코파일럿

**목표**: 에디터 내장 AI 어시스턴트.

작업:
- [ ] `/ws/copilot` WebSocket 엔드포인트
- [ ] `KnowledgeApi.copilot_chat()` 구현
- [ ] chat.js를 Oxios 코파일럿 API에 연결
- [ ] 스트리밍 응답 (SSE or WebSocket)
- [ ] [삽입] / [새 노트] 버튼 동작

**완료 기준**: 에디터에서 코파일럿에게 질문 → 컨텍스트 기반 응답.

### Phase 5: 고급 기능

**목표**: Space 연동, 그래프 시각화, 습관 분석.

작업:
- [ ] Space-scoped KnowledgeApi
- [ ] KnowledgeBridge reference/transfer 실제 구현
- [ ] 링크 그래프 시각화 (D3.js)
- [ ] 습관/일기 분석 → Sona engine 피드백
- [ ] files.md Telegram 봇 채널과 연동 (oxios-telegram 재활용)
- [ ] 크로스 디바이스 동기화 (files.md sync 프로토콜 활용)

---

## 10. 리스크 및 대안

### 10.1 WYSIWYG 에디터 복잡도

**리스크**: CodeMirror/HyperMD 의존성을 JS로 유지하는 것이 기술 부채가 될 수 있다.

**대안**:
- 단기: JS 그대로 사용 (현실적)
- 중기: CodeMirror 6의 WASM 빌드를 Dioxus에서 직접 호출
- 장기: oxios-markdown이 충분히 안정되면, 최소한의 에디터를 Rust/WASM으로 구현

### 10.2 인덱싱 성능

**리스크**: 대량의 .md 파일(수천 개)을 인덱싱할 때 지연.

**대안**:
- FileWatcher는 증분 인덱싱만 수행 (변경된 파일만)
- 초기 로드 시 `index_all()`은 백그라운드 태스크로 실행
- HNSW 인덱스를 디스크에 영속화 (`persist()`)하여 재시작 시 빠른 로드

### 10.3 동기화 충돌

**리스크**: 인간과 에이전트가 동시에 같은 파일을 수정.

**대안**:
- files.md의 LCS merge가 자동 해결
- 해결 불가능한 충돌 시 → 인간에게 알림 (에디터에 충돌 마커 표시)
- 파일 락은 오버헤드가 크니, merge-first 정책

### 10.4 Oxios 없이 files.md 단독 사용

**리스크**: files.md Rust 포팅이 Oxios에 종속되면 단독 사용자가 불편.

**대안**:
- `oxios-markdown`의 `kernel` feature가 꺼져 있으면 Oxios 의존성 없이 동작
- 단독 사용 시: `VirtualFs` + `SyncEngine` + `BacklinkIndex`만으로 충분
- CLI 바이너리 제공: `cargo run --bin filesmd -- /path/to/knowledge`

---

## 11. 요약

| 항목 | 내용 |
|------|------|
| **목표** | 인간-에이전트 공유 지식 베이스 구축 |
| **수단** | files.md Rust 포팅 → `oxios-markdown` 크레이트 |
| **통합점** | KnowledgeApi (KernelHandle 13번째 도메인) |
| **UI** | 단일 Axum 서버에 files.md JS 에디터 임베드 |
| **코파일럿** | 같은 프로세스에서 oxi 엔진 직접 호출 |
| **진실 원천** | `.md` 파일 (인간이 읽고, 에이전트가 검색) |
| **저작권** | MIT 호환, 원저자 표시 |
| **로드맵** | 6 Phase (포팅 → 크레이트 → API → UI → 코파일럿 → 고급) |

> **핵심 문장**: 인간은 files.md의 에디터로 생각을 정리하고, 에이전트는 KnowledgeApi로 같은 지식에 접근하며, 코파일럿은 둘 사이의 다리 역할을 한다. `.md` 파일이라는 가장 단순하고 개방적인 포맷이 하나의 진실 원천이다.
