# Phase 4: Project System Web UI

> **날짜:** 2026-05-30
> **범위:** Project CRUD + Session ↔ Project 연동 + AI detection 미리보기
> **전제:** Phase 1 (Core) 완료 — `ProjectManager`, `Orchestrator` project 연동, API contracts 모두 완료
> **리뷰:** v2 — 11건 리뷰 이슈 반영 (커널 API 확장, 중복 제거, 실제 코드베이스 정합)

---

## 0. PI 세션 컨텍스트

### 프로젝트 스택

| 영역 | 기술 |
|------|------|
| Framework | React 19 + TypeScript 6 |
| 라우팅 | TanStack Router (파일 기반, `autoCodeSplitting`) |
| 데이터 | TanStack Query (`useQuery` / `useMutation` + 캐시 무효화) |
| 상태 | Zustand 5 (persist 미들웨어) |
| 스타일 | Tailwind CSS v4 + shadcn/ui |
| i18n | i18next + react-i18next (EN/KO) |
| 빌드 | Vite 8 + Bun |
| 백엔드 | Axum (Rust) |

### 반드시 읽을 파일 (구현 전)

| 파일 | 이유 |
|------|------|
| `crates/oxios-kernel/src/project/manager.rs` | ProjectManager 전체 API (CRUD, link/unlink, detect) |
| `crates/oxios-kernel/src/project/mod.rs` | `Project` struct, `ProjectSource`, `ProjectId` |
| `crates/oxios-kernel/src/kernel_handle/project_api.rs` | **수정 대상** — list/get만 있음, CRUD 확장 필요 |
| `surface/oxios-web/src/server.rs` | `AppState` 필드 구성 |
| `surface/oxios-web/src/error.rs` | `AppError` variant |
| `surface/oxios-web/src/routes/mod.rs` | `PageParams`, `paginate()`, 라우트 등록 패턴 |
| `surface/oxios-web/src/routes/workspace.rs` | 메모리 조회 패턴 (`kernel.state.load`) |
| `surface/oxios-web/web/src/routes/chat.tsx` | **이미 project sidebar 구현됨** — 수정 불필요 |
| `surface/oxios-web/web/src/routes/sessions/$sessionId.tsx` | 세션 상세 (project 연결 UI 추가) |
| `surface/oxios-web/web/src/components/layout/sidebar.tsx` | navGroups 구조 |
| `surface/oxios-web/web/src/stores/chat.ts` | activeProjectId 추적 |

### 핵심 아키텍처 제약

```
Web Backend (Axum)
  └── State<Arc<AppState>
        └── kernel: Arc<KernelHandle>     ← 유일한 접근 경로
              ├── state: StateApi          ← 세션/메모리/시드
              ├── projects: Option<ProjectApi>  ← 프로젝트 (여기로!)
              ├── agents: AgentApi
              └── ...

  ❌ state.kernel.project_manager()   — binary 커널에만 있음
  ✅ state.kernel.projects.as_ref()   — web 라우트에서 접근
```

---

## 1. 개요

### 1.1 현재 상태

| 구성 요소 | 상태 | 비고 |
|-----------|------|------|
| `ProjectManager` (커널) | ✅ 완료 | CRUD, detection, SQLite persistence |
| `projects` + `project_memory` 테이블 | ✅ 완료 | SQLite schema |
| `OrchestrationResult` → `primary_project_id` | ✅ 완료 | API contract |
| `SessionContext` → `primary_project_id` | ✅ 완료 | 세션-프로젝트 연결 |
| `ProjectApi` (커널 핸들) | ⚠️ 불충분 | list/get만 있음 → CRUD 확장 필요 |
| Backend project routes | ❌ 없음 | `project_routes.rs` 미존재 |
| Frontend `/projects/` page | ❌ 없음 | `routes/projects/` 미존재 |
| Chat project sidebar | ✅ 이미 구현됨 | `chat.tsx`에 `ProjectSessionSidebar` 있음 |
| AI detection 미리보기 | ❌ 없음 | UI 없음 |

### 1.2 목표

1. **커널:** `ProjectApi`에 CRUD 6개 메서드 추가
2. **Backend:** `project_routes.rs` — REST API 8개 엔드포인트
3. **Frontend:** `/projects/` — 프로젝트 목록 + 상세 + 생성/편집/삭제
4. **Frontend:** 세션 상세에 project 연결 UI
5. **Frontend:** AI detection 미리보기 badge (stub)

---

## 2. 커널 수정: ProjectApi 확장

**파일:** `crates/oxios-kernel/src/kernel_handle/project_api.rs`

현재 `ProjectApi`는 list/get만 있습니다. 나머지 6개 메서드를 추가합니다.

### 2.1 ProjectInfo 확장

기존 `ProjectInfo`에 `created_at`, `updated_at` 추가:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub paths: Vec<String>,
    pub tags: Vec<String>,
    pub emoji: String,
    pub memory_visible: bool,
    pub created_at: String,      // ← 추가
    pub updated_at: String,      // ← 추가
    pub last_active_at: String,  // ← 추가
}

impl From<&Project> for ProjectInfo {
    fn from(project: &Project) -> Self {
        Self {
            // ... 기존 필드 ...
            created_at: project.created_at.to_rfc3339(),
            updated_at: project.updated_at.to_rfc3339(),
            last_active_at: project.last_active_at.to_rfc3339(),
        }
    }
}
```

### 2.2 추가 메서드

```rust
impl ProjectApi {
    // ─── 기존 (변경 없음) ───────────────────────────────
    pub fn list_projects(&self) -> Vec<ProjectInfo> { ... }
    pub fn get_project(&self, id: &str) -> Option<ProjectInfo> { ... }

    // ─── 신규 ───────────────────────────────────────────

    /// Create a new project. Returns the created ProjectInfo.
    pub fn create_project(
        &self,
        name: String,
        paths: Vec<String>,
        tags: Vec<String>,
        emoji: Option<String>,
        description: Option<String>,
    ) -> Result<ProjectInfo> {
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        let project = self.project_manager.create_project(
            name, paths, tags, emoji, description, ProjectSource::Manual,
        )?;
        Ok(ProjectInfo::from(&project))
    }

    /// Update a project. Only non-None fields are changed.
    pub fn update_project(
        &self,
        id: &str,
        name: Option<String>,
        paths: Option<Vec<String>>,
        tags: Option<Vec<String>>,
        emoji: Option<String>,
        description: Option<String>,
        memory_visible: Option<bool>,
    ) -> Result<ProjectInfo> {
        let project_id = Uuid::parse_str(id)
            .context("Invalid project ID")?;
        let paths = paths.map(|v| v.into_iter().map(PathBuf::from).collect());

        let mut project = self.project_manager.update_project(
            project_id, name, paths, tags, emoji, description,
        )?;

        // memory_visible은 ProjectManager::update_project에 파라미터가 없으므로
        // 직접 설정 후 저장
        if let Some(visible) = memory_visible {
            project.memory_visible = visible;
            project.updated_at = Utc::now();
            // ProjectManager에 save 위임 — 내부에서 db.save_project 호출
            self.project_manager.save_project(&project)?;
        }

        Ok(ProjectInfo::from(&project))
    }

    /// Remove a project.
    pub fn remove_project(&self, id: &str) -> Result<()> {
        let project_id = Uuid::parse_str(id)
            .context("Invalid project ID")?;
        self.project_manager.remove_project(project_id)
    }

    /// Link a memory to a project.
    pub fn link_memory(&self, project_id: &str, memory_id: &str) -> Result<()> {
        let pid = Uuid::parse_str(project_id)
            .context("Invalid project ID")?;
        self.project_manager.link_memory(pid, memory_id)
    }

    /// Unlink a memory from a project.
    pub fn unlink_memory(&self, project_id: &str, memory_id: &str) -> Result<()> {
        let pid = Uuid::parse_str(project_id)
            .context("Invalid project ID")?;
        self.project_manager.unlink_memory(pid, memory_id)
    }

    /// Get all memory IDs linked to a project.
    pub fn get_project_memory_ids(&self, project_id: &str) -> Result<Vec<String>> {
        let pid = Uuid::parse_str(project_id)
            .context("Invalid project ID")?;
        self.project_manager.get_project_memory_ids(pid)
    }
}
```

### 2.3 ProjectManager 수정

`update_project`에 `memory_visible` 파라미터가 없으므로, 별도 `save_project` 메서드를 추가합니다:

```rust
// project/manager.rs에 추가

/// Save (upsert) a project to SQLite directly.
/// Used when fields like `memory_visible` need updating outside update_project().
pub fn save_project(&self, project: &Project) -> Result<()> {
    self.db.save_project(project)?;

    // Refresh in-memory indices
    let mut projects = self.projects.write();
    let mut name_index = self.name_index.write();
    name_index.insert(project.name.clone(), project.id);
    projects.insert(project.id, project.clone());

    Ok(())
}
```

### 2.4 커널 수정 파일 요약

| 파일 | 변경 | 설명 |
|------|------|------|
| `kernel_handle/project_api.rs` | 수정 | `ProjectInfo` 필드 3개 추가, 메서드 6개 추가 |
| `project/manager.rs` | 수정 | `save_project()` 메서드 추가 |

---

## 3. Backend: project_routes.rs

### 3.1 엔드포인트

| Method | Path | 설명 | Handler |
|--------|------|------|---------|
| `GET` | `/api/projects` | 목록 | `handle_projects_list` |
| `POST` | `/api/projects` | 생성 | `handle_project_create` |
| `GET` | `/api/projects/{id}` | 상세 | `handle_project_get` |
| `PUT` | `/api/projects/{id}` | 수정 | `handle_project_update` |
| `DELETE` | `/api/projects/{id}` | 삭제 | `handle_project_delete` |
| `GET` | `/api/projects/{id}/memories` | 연결된 메모리 | `handle_project_memories` |
| `POST` | `/api/projects/{id}/memories` | 메모리 연결 | `handle_project_link_memory` |
| `DELETE` | `/api/projects/{id}/memories/{memoryId}` | 메모리 해제 | `handle_project_unlink_memory` |

### 3.2 Query/Request 타입

```rust
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

use crate::error::AppError;
use crate::routes::paginate;
use crate::server::AppState;

use oxios_kernel::ProjectInfo;

/// List query parameters (검색용, PageParams와 별도).
#[derive(Debug, Deserialize)]
pub(crate) struct ProjectListParams {
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// 검색어 (name, description, tags에서 검색).
    pub search: Option<String>,
}

fn default_page() -> usize { 1 }
fn default_limit() -> usize { 50 }

#[derive(Debug, Deserialize)]
pub(crate) struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub emoji: Option<String>,
    pub description: Option<String>,
    #[serde(default = "default_true")]
    pub memory_visible: bool,
}

fn default_true() -> bool { true }

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateProjectRequest {
    pub name: Option<String>,
    pub paths: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub emoji: Option<String>,
    pub description: Option<String>,
    pub memory_visible: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LinkMemoryRequest {
    pub memory_id: String,
}
```

### 3.3 핸들러

모든 핸들러는 `state.kernel.projects` (`Option<ProjectApi>`)를 통해 접근합니다:

```rust
/// 모든 핸들러의 공통 패턴:
///   state.kernel.projects.as_ref()
///     .ok_or_else(|| AppError::Internal("ProjectManager not initialized".into()))?
macro_rules! project_api {
    ($state:expr) => {
        $state.kernel.projects.as_ref()
            .ok_or_else(|| AppError::Internal("Projects not available".into()))?
    };
}

/// GET /api/projects
pub(crate) async fn handle_projects_list(
    state: State<Arc<AppState>>,
    Query(params): Query<ProjectListParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api = project_api!(state);
    let all = api.list_projects();

    let filtered: Vec<ProjectInfo> = match &params.search {
        Some(search) => {
            let lower = search.to_lowercase();
            all.into_iter()
                .filter(|p|
                    p.name.to_lowercase().contains(&lower)
                    || p.description.to_lowercase().contains(&lower)
                    || p.tags.iter().any(|t| t.to_lowercase().contains(&lower))
                )
                .collect()
        }
        None => all,
    };

    // 정렬: last_active_at 내림차순
    let mut sorted = filtered;
    sorted.sort_by(|a, b| b.last_active_at.cmp(&a.last_active_at));

    let total = sorted.len();
    let limit = params.limit.min(500);
    let offset = (params.page.saturating_sub(1)) * limit;
    let items: Vec<&ProjectInfo> = sorted.iter().skip(offset).take(limit).collect();

    Ok(Json(serde_json::json!({
        "items": items,
        "total": total,
        "page": params.page,
        "limit": limit,
    })))
}

/// GET /api/projects/{id}
pub(crate) async fn handle_project_get(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ProjectInfo>, AppError> {
    let api = project_api!(state);
    api.get_project(&id)
        .ok_or_else(|| AppError::NotFound("Project not found".into()))
        .map(Json)
}

/// POST /api/projects
pub(crate) async fn handle_project_create(
    state: State<Arc<AppState>>,
    Json(body): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectInfo>), AppError> {
    let api = project_api!(state);

    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("Project name is required".into()));
    }

    let project = api
        .create_project(body.name, body.paths, body.tags, body.emoji, body.description)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(project)))
}

/// PUT /api/projects/{id}
pub(crate) async fn handle_project_update(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateProjectRequest>,
) -> Result<Json<ProjectInfo>, AppError> {
    let api = project_api!(state);

    let project = api
        .update_project(
            &id,
            body.name,
            body.paths,
            body.tags,
            body.emoji,
            body.description,
            body.memory_visible,
        )
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    Ok(Json(project))
}

/// DELETE /api/projects/{id}
pub(crate) async fn handle_project_delete(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let api = project_api!(state);

    api.remove_project(&id)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/projects/{id}/memories
pub(crate) async fn handle_project_memories(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<crate::routes::PageParams>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api = project_api!(state);

    // 1. project 존재 확인 + memory ID 목록 조회
    let memory_ids = api
        .get_project_memory_ids(&id)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // 2. 각 memory_id에 대해 StateApi에서 MemoryEntry 로드
    //    workspace.rs의 handle_memory_get과 동일한 패턴
    let mut memories = Vec::new();
    for mid in &memory_ids {
        for category in ["memory/facts", "memory/episodes", "memory/knowledge", "memory/sessions"] {
            if let Ok(Some(entry)) = state
                .kernel
                .state
                .load::<oxios_kernel::memory::MemoryEntry>(category, mid)
                .await
            {
                memories.push(serde_json::json!({
                    "id": entry.id,
                    "content": entry.content,
                    "memory_type": entry.memory_type.label(),
                    "importance": entry.importance,
                    "tier": entry.tier.label(),
                    "tags": entry.tags,
                    "created_at": entry.created_at.to_rfc3339(),
                }));
                break; // 찾았으면 다음 memory_id로
            }
        }
    }

    Ok(Json(paginate(&memories, &params)))
}

/// POST /api/projects/{id}/memories
pub(crate) async fn handle_project_link_memory(
    state: State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<LinkMemoryRequest>,
) -> Result<StatusCode, AppError> {
    let api = project_api!(state);

    api.link_memory(&id, &body.memory_id)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

/// DELETE /api/projects/{id}/memories/{memoryId}
pub(crate) async fn handle_project_unlink_memory(
    state: State<Arc<AppState>>,
    Path((project_id, memory_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let api = project_api!(state);

    api.unlink_memory(&project_id, &memory_id)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}
```

### 3.4 mod.rs 등록

```rust
// 1. 모듈 선언 (기존 mod 블록에 추가)
mod project_routes;

// 2. re-export (pub(crate) use 블록에 추가)
pub(crate) use project_routes::{
    handle_projects_list, handle_project_get,
    handle_project_create, handle_project_update, handle_project_delete,
    handle_project_memories, handle_project_link_memory, handle_project_unlink_memory,
};

// 3. build_routes() 내 "Projects" 주석 아래에 추가
        // Projects
        .route("/api/projects", get(handle_projects_list))
        .route("/api/projects", post(handle_project_create))
        .route("/api/projects/{id}", get(handle_project_get))
        .route("/api/projects/{id}", put(handle_project_update))
        .route("/api/projects/{id}", delete(handle_project_delete))
        .route("/api/projects/{id}/memories", get(handle_project_memories))
        .route("/api/projects/{id}/memories", post(handle_project_link_memory))
        .route("/api/projects/{id}/memories/{memoryId}", delete(handle_project_unlink_memory))
```

### 3.5 컴파일 검증

```bash
cargo build -p oxios-web
```

---

## 4. Frontend: /projects/ CRUD

### 4.1 목록 페이지 (`routes/projects/index.tsx`)

```
┌────────────────────────────────────────────────────────────────┐
│  Projects                                      [+ New Project]  │
├────────────────────────────────────────────────────────────────┤
│  [🔍 검색...]                                     [↻]          │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│  ┌─── 🔧 oxios ───────────────────────────────────────────┐   │
│  │  /Volumes/MERCURY/PROJECTS/oxios                        │   │
│  │  Tags: rust, kernel, async                              │   │
│  │  Last active: 2h ago  |  manual                        │   │
│  │  [View →]  [Edit]  [Delete]                            │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                                │
│  ┌─── 📝 pi-docs ─────────────────────────────────────────┐   │
│  │  No paths  |  Tags: docs, writing                      │   │
│  │  Last active: 1d ago  |  auto_detected                 │   │
│  │  [View →]  [Edit]  [Delete]                            │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                                │
│  ┌─── 🎮 game1 ───────────────────────────────────────────┐   │
│  │  /Users/won/games/rust-game                            │   │
│  │  Tags: game, rust, graphics                            │   │
│  │  Last active: 3d ago  |  manual                        │   │
│  │  [View →]  [Edit]  [Delete]                            │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

### 4.2 상세 페이지 (`routes/projects/$projectId.tsx`)

```
┌────────────────────────────────────────────────────────────────┐
│  ← Back to Projects                                            │
├────────────────────────────────────────────────────────────────┤
│  🔧 oxios                                    [Edit]  [Delete]  │
│  Oxios Agent Operating System                                   │
├──────────┬──────────┬──────────────────────────────────────────┤
│ Details  │ Memories │ Activity                                  │
├──────────┴──────────┴──────────────────────────────────────────┤
│                                                                │
│  ┌─── Details 탭 ──────────────────────────────────────────┐   │
│  │  Paths:                                                │   │
│  │    📁 /Volumes/MERCURY/PROJECTS/oxios                  │   │
│  │  Tags: rust | kernel | async                           │   │
│  │  Source: manual                                        │   │
│  │  Memory visible: ✅                                    │   │
│  │  Created: 2026-05-28 10:00                             │   │
│  │  Last active: 2026-05-30 14:22                         │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                                │
│  ┌─── Memories 탭 ────────────────────────────────────────┐   │
│  │  🔴 "Rust async patterns for agent scheduling"        │   │
│  │  🟡 "Ouroboros protocol understanding"                 │   │
│  │  🔵 "Memory tier system design"                        │   │
│  │  [+ Link Memory]                                       │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                                │
│  ┌─── Activity 탭 ────────────────────────────────────────┐   │
│  │  (Phase 3: 세션 히스토리, 메모리 추가/변경 이력)        │   │
│  │  Phase 4에서는 placeholder                             │   │
│  └────────────────────────────────────────────────────────┘   │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

### 4.3 생성 다이얼로그 (`create-project-dialog.tsx`)

```
┌─────────────────────────────────────────────────────────────┐
│  New Project                                           [X]  │
├─────────────────────────────────────────────────────────────┤
│  Name:        [oxios________________________________]       │
│  Emoji:       [🔧]  (클릭 시 이모지 피커)                   │
│  Description: [Oxios Agent Operating System____________]    │
│  Tags:        [rust, kernel]  [+ Add]                       │
│                                                              │
│  Paths:                                               [+]   │
│    📁 /Volumes/MERCURY/PROJECTS/oxios                  [✕]   │
│    (빈 값 허용 — 코드가 아닌 프로젝트)                       │
│                                                              │
│  ☑ Memory visible (cross-project 접근 허용)                  │
│                                                              │
│          [Cancel]  [Create]                                  │
└─────────────────────────────────────────────────────────────┘
```

### 4.4 편집 다이얼로그 (`edit-project-dialog.tsx`)

생성과 동일 + 현재 값 pre-fill. `memory_visible` 토글 포함.

### 4.5 삭제 확인 (`delete-project-dialog.tsx`)

```
┌─────────────────────────────────────────────────────────────┐
│  Delete "oxios"?                                             │
├─────────────────────────────────────────────────────────────┤
│  이 프로젝트를 제거합니다:                                   │
│  • Memories는 삭제되지 않습니다                               │
│  • 경로의 파일은 삭제되지 않습니다                            │
│  • 이 작업은 되돌릴 수 없습니다                               │
│                                                              │
│          [Cancel]  [Delete]                                  │
└─────────────────────────────────────────────────────────────┘
```

---

## 5. Session ↔ Project 연동

### 5.1 chat.tsx — 수정 불필요

`chat.tsx`에 이미 `ProjectSessionSidebar`가 구현되어 있습니다:
- `/api/projects` 호출로 프로젝트 목록 표시
- project 선택 → `setActiveProject(projectId)` → chat에 `project_id` 전송
- `<Link to="/projects">` "Manage Projects" 링크 있음

**이 Phase에서 할 일:** project CRUD routes만 구현하면 sidebar가 자동으로 동작.
chat.tsx는 수정하지 않습니다.

### 5.2 AI Detection Badge — chat.tsx 입력창 위에 추가

`chat.tsx`의 `ChatInput` 위에 detection badge를 배치합니다.
Phase 2에서 `detect()`가 실제 동작하기 전까지는 보이지 않습니다.

```tsx
// chat.tsx 수정 — 메시지 영역과 ChatInput 사이에 추가
{detectedProject && !activeProjectId && (
  <DetectionBadge
    project={detectedProject}
    onApply={() => setActiveProject(detectedProject.id)}
    onDismiss={() => clearDetectedProject()}
  />
)}
```

`stores/chat.ts`에 `detectedProject` 상태 추가:
```typescript
detectedProject: Project | null,
setDetectedProject: (project: Project | null) => void,
```

**Phase 2에서:** 백엔드가 응답에 `detected_project_id`를 포함 → 프론트엔드가 store 업데이트 → badge 표시.

### 5.3 Session 상세 — project 연결 UI

`routes/sessions/$sessionId.tsx`에 project 표시/변경 추가:

```
┌────────────────────────────────────────────────────────────────┐
│  ← Back to Sessions                                            │
├────────────────────────────────────────────────────────────────┤
│  Session #abc...def                                            │
│  Created: 2026-05-30 10:00  |  Messages: 12                    │
│                                                                │
│  Project: 🔧 oxios  [Change ▼] [Remove]                       │
│                                                                │
│  ...(기존 세션 상세)                                           │
└────────────────────────────────────────────────────────────────┘
```

**구현:** `Select` 컴포넌트로 프로젝트 목록 드롭다운. 선택 시 세션 metadata에 `project_id` 업데이트.

---

## 6. 데이터 흐름

### 6.1 Project CRUD

```
Frontend                    Backend                         Kernel
   │                           │                               │
   │  GET /api/projects        │                               │
   │───────────────────────────►                               │
   │                           │  kernel.projects.list()        │
   │                           │───────────────────────────────►│
   │                           │◄───────────────────────────────│
   │                           │  Vec<ProjectInfo>              │
   │◄──────────────────────────│                               │
   │  { items, total }         │                               │
```

### 6.2 Project ↔ Memory Linking

```
Frontend                    Backend                         Kernel
   │                           │                               │
   │  POST /api/projects/:id/memories                         │
   │  { memory_id }            │                               │
   │───────────────────────────►                               │
   │                           │  kernel.projects.link_memory() │
   │                           │───────────────────────────────►│
   │                           │◄───────────────────────────────│
   │  204                      │                               │
   │◄──────────────────────────│                               │
```

### 6.3 메모리 조회 (project → memory 상세)

```
Frontend                    Backend                         Kernel
   │                           │                               │
   │  GET /api/projects/:id/memories                          │
   │───────────────────────────►                               │
   │                           │  1. projects.get_memory_ids()  │
   │                           │───────────────────────────────►│
   │                           │◄───────────────────────────────│
   │                           │  [id1, id2, id3]              │
   │                           │                                │
   │                           │  2. state.load::<MemoryEntry>()│
   │                           │     for each id, 4 categories  │
   │                           │───────────────────────────────►│
   │                           │◄───────────────────────────────│
   │  { items: [...] }         │                               │
   │◄──────────────────────────│                               │
```

---

## 7. 파일 목록

### 커널 (Rust) — Phase 4를 위해 수정 필요

| 파일 | 작업 | 설명 |
|------|------|------|
| `kernel_handle/project_api.rs` | **수정** | `ProjectInfo` 필드 3개 추가, 메서드 6개 추가 |
| `project/manager.rs` | **수정** | `save_project()` 메서드 추가 |

### Backend (Rust) — 신규

| 파일 | 작업 | 설명 |
|------|------|------|
| `surface/oxios-web/src/routes/project_routes.rs` | **신규** | 핸들러 8개 |
| `surface/oxios-web/src/routes/mod.rs` | **수정** | `mod project_routes;` + import + 8개 라우트 |

### Frontend (React/TS) — 신규

| 파일 | 설명 |
|------|------|
| `web/src/hooks/use-projects.ts` | TanStack Query 훅 (list, get, create, update, delete, linkMemory, unlinkMemory) |
| `web/src/components/project/project-card.tsx` | 목록 카드 |
| `web/src/components/project/create-project-dialog.tsx` | 생성 다이얼로그 |
| `web/src/components/project/edit-project-dialog.tsx` | 편집 다이얼로그 |
| `web/src/components/project/delete-project-dialog.tsx` | 삭제 확인 |
| `web/src/components/project/project-memory-list.tsx` | 연결된 메모리 목록 |
| `web/src/components/project/ai-detection-badge.tsx` | AI detection 미리보기 badge |
| `web/src/routes/projects/index.tsx` | 목록 페이지 |
| `web/src/routes/projects/$projectId.tsx` | 상세 페이지 |

### Frontend (React/TS) — 수정

| 파일 | 설명 |
|------|------|
| `web/src/routes/sessions/$sessionId.tsx` | project 연결 UI 추가 (Select dropdown) |
| `web/src/routes/chat.tsx` | detection badge만 추가 (sidebar는 이미 구현됨) |
| `web/src/stores/chat.ts` | `detectedProject` 상태 추가 |
| `web/src/components/layout/sidebar.tsx` | Projects nav 항목 추가 |

### i18n

| 파일 | 변경 |
|------|------|
| `web/src/public/locales/en/common.json` | projects 키 ~15개 |
| `web/src/public/locales/ko/common.json` | projects 키 ~15개 |

---

## 8. 구현 순서

### Step 1: 커널 수정

1. `project/manager.rs` — `save_project()` 추가
2. `kernel_handle/project_api.rs` — `ProjectInfo` 확장 + 6개 메서드 추가
3. `cargo build -p oxios-kernel` 확인

### Step 2: Backend CRUD

4. `surface/oxios-web/src/routes/project_routes.rs` 생성
5. `surface/oxios-web/src/routes/mod.rs` — `mod project_routes;` + import + 라우트 등록
6. `cargo build -p oxios-web` 확인

### Step 3: Frontend 기반

7. `hooks/use-projects.ts` — TanStack Query 훅
8. `components/project/project-card.tsx`
9. `components/project/create-project-dialog.tsx`
10. `components/project/edit-project-dialog.tsx`
11. `components/project/delete-project-dialog.tsx`

### Step 4: Frontend 페이지

12. `routes/projects/index.tsx` — 목록
13. `routes/projects/$projectId.tsx` — 상세 (Details/Memories 탭)
14. `components/project/project-memory-list.tsx`

### Step 5: Session + Chat 연동

15. `routes/sessions/$sessionId.tsx` — project 연결 UI
16. `components/project/ai-detection-badge.tsx` — stub
17. `routes/chat.tsx` — detection badge 추가
18. `stores/chat.ts` — detectedProject 상태

### Step 6: 마무리

19. `components/layout/sidebar.tsx` — Projects 항목 추가
20. i18n keys 추가
21. `bun run build` + `cargo test --workspace` 확인

---

## 9. 사이드바 내비게이션 변경

현재:
```
Agents
  ├── Agents
  ├── Agent Groups
  ├── Seeds
  ├── Personas
  └── Skills

Storage
  ├── Knowledge
  ├── Memory
  └── Workspace
```

변경 후:
```
Agents
  ├── Agents
  ├── Agent Groups
  ├── Seeds
  ├── Personas
  └── Skills

Projects                    ← 신규 독립 그룹
  └── Projects

Storage
  ├── Knowledge
  ├── Memory
  └── Workspace
```

sidebar.tsx에 추가:
```tsx
{
  labelKey: 'common.projects',
  items: [
    { labelKey: 'common.projects', href: '/projects', icon: <FolderKanban className="h-4 w-4" /> },
  ],
},
```

---

## 10. 리뷰 이슈 해결 추적

| # | 심각도 | 이슈 | 해결 |
|---|--------|------|------|
| 1 | 🔴 | `state.kernel.project_manager()` 미존재 | `state.kernel.projects` 사용 |
| 2 | 🔴 | `ProjectApi`에 CRUD 미존재 | §2에서 6개 메서드 추가 |
| 3 | 🔴 | `PageParams`에 `search` 없음 | `ProjectListParams` 별도 정의 |
| 4 | 🔴 | `state.workspace.memory_store` 미존재 | `state.kernel.state.load` 패턴 사용 |
| 5 | 🟡 | `update_project`에 `memory_visible` 없음 | `save_project()` 별도 추가 |
| 6 | 🟡 | unused `existing` 변수 | 제거 |
| 7 | 🟡 | Path에 PageParams 혼합 | `Path(id)` + `Query(params)` 분리 |
| 8 | 🟠 | chat.tsx 중복 구현 | 수정 불필요로 변경 |
| 9 | 🟠 | `ProjectInfo` vs `ProjectResponse` 이중 | `ProjectInfo` 확장으로 통일 |
| 10 | 🔵 | `mod project_routes;` 누락 | §3.4에 명시 |
| 11 | 🔵 | `touch()` 미연결 | Phase 4에서는 세션 로드 시 `touch()` 호출하는 경로를 project_routes에 추가하지 않음 — Orchestrator가 이미 project touch를 수행 |

---

## 11. 완료 후 상태

```
┌─────────────────────────────────────────────────────────────┐
│           RFC-011 구현 상태                                  │
├─────────────────────────────────────────────────────────────┤
│  Phase 1 (Core)       ████████████████████  ✅ 완료         │
│  Phase 2 (AI Detect)  ░░░░░░░░░░░░░░░░░░░  📋 미구현     │
│  Phase 3 (Memories)   ██████████████████░░  📋 미구현     │
│  Phase 4 (Web UI)     ████████████████████  ✅ 이 설계     │
├─────────────────────────────────────────────────────────────┤
│  Phase 4 이후 다음 단계:                                     │
│  • Phase 2: detect() 실제 구현 → badge가 동작 시작         │
│  • Phase 3: project_memory junction 기반 메모리 조회        │
│  • Activity 탭: 세션 히스토리, 메모리 변경 이력             │
└─────────────────────────────────────────────────────────────┘
```
