# Web UI Overhaul Design

> 날짜: 2026-05-17
> 범위: Dioxus 프론트엔드 전면 보완

## 1. 변경 개요

### 치명적 수정 (P0)

| ID | 항목 | 파일 |
|----|------|------|
| C1 | 채팅 API 스키마 불일치 | `frontend/src/api/mod.rs` |
| C2 | 인증 헤더 미전송 | `frontend/src/api/mod.rs` |
| C3 | WebSocket 채팅 미연결 | `frontend/src/views/chat.rs` 신규 로직 |

### 신규 뷰 추가 (P1)

| ID | 뷰 | 백엔드 API | 패널 |
|----|----|-----------|------|
| V1 | 크론잡 관리 | 6개 CRUD | System 섹션 |
| V2 | 세션 관리 | 3개 (list/get/delete) | Monitor 섹션 |
| V3 | Git 버전관리 | 4개 | System 섹션 |
| V4 | 예산 관리 | 5개 | Agents 섹션 |
| V5 | 에이전트 그룹 | 2개 | Agents 섹션 |
| V6 | 리소스 모니터링 | 3개 | Monitor 섹션 |

### 기존 뷰 보강 (P2)

| ID | 뷰 | 보강 내용 |
|----|----|---------| 
| E1 | Protocol | 하드코딩 → 동적 phase 조회 |
| E2 | Security | 권한 관리, 감사 내보내기/검증 |
| E3 | Memory | 검색 UI, 생성 UI |
| E4 | Workspace | 파일 편집 + 저장 (PUT) |
| E5 | Personas | CRUD (생성/수정/삭제) |
| E6 | Skills | 생성 UI, 상세 조회 |
| E7 | Programs | 설치, 삭제, 상세 |

---

## 2. 아키텍처 설계

### 2.1 인증 모듈 (C2)

**위치**: `frontend/src/api/mod.rs` + `frontend/src/main.rs`

```
localStorage("oxios-api-key")
        ↓
api::AuthContext (GlobalSignal)
        ↓
모든 HTTP 헬퍼가 Authorization: Bearer {key} 삽입
```

**변경사항**:
- `GlobalSignal<Option<String>>` 로 API 키 관리
- 모든 fetch/post/put/delete 헬퍼에 `.header("Authorization", ...)` 추가
- `main.rs`에서 localStorage 읽어 초기화
- 사이드바 또는 설정 패널에 API 키 입력 UI 추가

### 2.2 채팅 스키마 수정 (C1)

**프론트엔드를 백엔드에 맞춤** (백엔드가 이미 여러 소비자에게 서비스 중이므로)

```rust
// Before (frontend)
pub struct ChatRequest {
    pub message: String,           // → content
    pub session_id: Option<String>, // → String (default "")
}

pub struct ChatResponse {
    pub response: String,          // → reply
    pub session_id: String,        // → Option<String>
}

// After (frontend)
pub struct ChatRequest {
    pub content: String,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub session_id: String,
}

pub struct ChatResponse {
    pub id: String,
    pub echo: String,
    pub reply: String,
    pub session_id: Option<String>,
    pub phase: Option<String>,
}
```

### 2.3 WebSocket 채팅 (C3)

**프로토콜**: `GET /api/chat/stream?token={api_key}` → WebSocketUpgrade

```
Browser ←WebSocket→ Axum → mpsc → Gateway → Kernel
                  ↑                    ↓
                  └── broadcast ←──────┘
```

**Dioxus 구현**:
- `web_sys::WebSocket` 사용 (gloo-net에는 WS 클라이언트 없음)
- `use_signal<Vec<MessageEntry>>` 에 수신 메시지 push
- 재연결 로직 (exponential backoff, Events 뷰 패턴 차용)
- REST POST는 폴백으로 유지 (WS 연결 실패 시)

### 2.4 Protocol 뷰 동적화 (E1)

현재: 5단계 항상 Interview→Seed completed, Execute active, 나머지 대기

변경:
- `GET /api/seeds`에서 최근 시드 조회
- 각 시드의 상태(또는 `GET /api/seeds/{id}/evolution`)에서 phase 추출
- 활성 시드의 현재 phase에 따라 단계 표시 업데이트
- 진화 이력 타임라인 표시

### 2.5 사이드바 확장

**현재**: 16개 Panel → 6개 섹션

**변경**: 22개 Panel → 7개 섹션

```
Core:        Chat, Dashboard, Config
Agents:      Agents, Agent Groups, Personas, Budget
Ouroboros:   Protocol, Seeds
System:      Workspace, Skills, Programs, Cron Jobs, Git, Memory
Security:    Security, Approvals, Permissions
Monitor:     Events, Sessions, Resources, Host Tools
```

---

## 3. 파일별 변경 명세

### 3.1 `frontend/src/api/mod.rs`

**수정**:
- [C1] `ChatRequest`: `message` → `content`, `user_id` 추가, `session_id: String`
- [C1] `ChatResponse`: `response` → `reply`, `id`/`echo` 추가, `session_id: Option<String>`
- [C2] `AUTH_TOKEN: GlobalSignal<Option<String>>` 추가
- [C2] `auth_header()` 헬퍼 — 토큰 있으면 `Bearer {token}` 반환
- [C2] 모든 HTTP 함수에 `.header("Authorization", auth_header())` 삽입

**신규 타입**:
```rust
// Cron Jobs
pub struct CronJobSummary { pub id, name, schedule, goal, enabled, last_run, next_run }
pub struct CreateCronJobRequest { pub name, schedule, goal, constraints, acceptance_criteria, toolchain, priority }

// Sessions
pub struct SessionListItem { pub id, user_id, message_count, active_seed_id, created_at, updated_at }

// Git
pub struct GitLogEntry { pub hash, author, message, timestamp }
pub struct GitTag { pub name, hash }

// Budget
pub struct BudgetInfo { pub agent_id, tokens_remaining, calls_remaining, window_remaining_secs, is_exhausted }
pub struct SetBudgetRequest { pub token_budget, calls_budget, window_secs }

// Agent Groups
// (serde_json::Value 사용 — 백엔드가 동적 JSON 반환)

// Resources
// (serde_json::Value 사용 — 백엔드가 ResourceMonitor 스냅샷 직렬화)
pub struct OverloadStatus { pub overloaded: bool, pub threshold: ThresholdInfo }
```

**신규 함수**:
```rust
pub async fn post_json_empty_response<B: Serialize>(path: &str, body: &B) -> Result<(), String>
// JSON body POST but ignore response body (cron create/edit, budget set, etc.)
```

### 3.2 `frontend/src/main.rs`

**수정**:
- [C2] localStorage에서 `oxios-api-key` 읽어 `AUTH_TOKEN` 초기화

### 3.3 `frontend/src/views/chat.rs`

**수정**:
- [C1] `ChatRequest`/`ChatResponse` 새 스키마 적용
- [C3] WebSocket 연결 로직 추가
  - `web_sys::WebSocket::new("/api/chat/stream?token=...")`
  - `onmessage` → messages.push()
  - `onerror`/`onclose` → REST 폴백
  - `send()` → WS 전송 (연결 안 되어 있으면 REST POST)

### 3.4 `frontend/src/views/protocol.rs`

**수정**:
- [E1] 하드코딩 제거, 실제 시드 phase로 단계 표시
- 최근 시드 중 가장 최신 것의 phase를 기준으로 활성 단계 결정
- 진화 이력 (`GET /api/seeds/{id}/evolution`) 조회 → 타임라인

### 3.5 `frontend/src/views/security.rs`

**수정**:
- [E2] 상단에 탭: Audit Log | Permissions
- Permissions 탭: 에이전트 선택 → `GET /api/permissions/{agent}` → 권한 매트릭스 표시/편집
- 감사 로그 탭에 "Export" 버튼 (`POST /api/audit/export`)
- 감사 로그 탭에 "Verify Integrity" 버튼 (`GET /api/audit/verify`)

### 3.6 `frontend/src/views/memory.rs`

**수정**:
- [E3] 검색 바 추가 (`POST /api/memory/search`)
- 시맨틱 검색 토글 (`POST /api/memory/semantic`)
- 생성 버튼 → 모달 폼 (`POST /api/memory`)

### 3.7 `frontend/src/views/workspace.rs`

**수정**:
- [E4] 파일 열기 → 편집 모드 (textarea)
- 저장 버튼 → `PUT /api/workspace/file/{path}` (put_text)

### 3.8 `frontend/src/views/personas.rs`

**수정**:
- [E5] 생성 버튼 → 모달 폼 (`POST /api/personas`)
- 카드에 편집/삭제 버튼 (`PUT/DELETE /api/personas/{id}`)

### 3.9 `frontend/src/views/skills.rs`

**수정**:
- [E6] 생성 버튼 → 모달 폼 (`POST /api/skills`)
- 카드 클릭 → 상세 조회 (`GET /api/skills/{name}`)

### 3.10 `frontend/src/views/programs.rs`

**수정**:
- [E7] 설치 버튼 → 모달 폼 (`POST /api/programs`)
- 카드에 삭제 버튼 (`DELETE /api/programs/{name}`)

### 3.11 신규 뷰 파일

| 파일 | 설명 |
|------|------|
| `views/cron_jobs.rs` | [V1] 크론잡 목록 + 생성/편집/삭제/수동실행 |
| `views/sessions.rs` | [V2] 세션 목록 + 상세 + 삭제 |
| `views/git.rs` | [V3] 커밋 로그 + 태그 + 검증 + 복원 |
| `views/budget.rs` | [V4] 에이전트별 예산 설정/조회/예약/리셋 |
| `views/agent_groups.rs` | [V5] 에이전트 그룹 목록 + 상세 |
| `views/resources.rs` | [V6] CPU/메모리/부하 스냅샷 + 히스토리 + 과부하 상태 |

### 3.12 `frontend/src/components/sidebar.rs`

**수정**:
- Panel 열거형에 6개 추가: `CronJobs`, `Sessions`, `Git`, `Budget`, `AgentGroups`, `Resources`
- NAV_ITEMS에 추가
- Section에 Monitor 추가 (이미 있음)
- panel_icon()에 아이콘 매핑 추가

### 3.13 `frontend/src/components/icons.rs`

**신규 아이콘** (필요한 것만):
- `IconGit` (분기 아이콘)
- `IconDatabase` (세션/DB)
- `IconDollarSign` (예산)
- `IconLayers` (에이전트 그룹)
- `IconCpu` (리소스)

### 3.14 `frontend/src/views/mod.rs`

**추가**:
```rust
pub mod cron_jobs;
pub mod sessions;
pub mod git;
pub mod budget;
pub mod agent_groups;
pub mod resources;
```

### 3.15 `frontend/src/components/layout.rs`

**수정**:
- Panel 매치에 6개 신규 뷰 추가

### 3.16 `frontend/static/style.css`

**추가**:
- 탭 컴포넌트 스타일 (`.tab-bar`, `.tab-item`, `.tab-active`)
- 모달/다이얼로그 스타일 (`.modal-overlay`, `.modal`, `.modal-header`, `.modal-body`, `.modal-footer`)
- 폼 필드 스타일 (`.form-group`, `.form-label`, `.form-input`, `.form-select`, `.form-textarea`)
- 검색 바 스타일 (`.search-bar`)
- 리소스 게이지 스타일 (`.gauge`, `.gauge-fill`)
- 타임라인 스타일 (`.timeline`, `.timeline-item`)
- API 키 입력 섹션 (`.api-key-section`)

---

## 4. 구현 순서

### Phase 1: 치명적 수정 (1차)
1. `api/mod.rs` — 인증 헤더 + 채팅 스키마 수정
2. `views/chat.rs` — 새 스키마 + WebSocket
3. `main.rs` — API 키 초기화
4. 빌드 + 테스트

### Phase 2: 신규 뷰 (2차)
5. `views/cron_jobs.rs` — 크론잡 CRUD
6. `views/sessions.rs` — 세션 관리
7. `views/git.rs` — Git 버전관리
8. `views/budget.rs` — 예산 관리
9. `views/agent_groups.rs` — 에이전트 그룹
10. `views/resources.rs` — 리소스 모니터링
11. sidebar + layout + icons 확장
12. views/mod.rs 모듈 등록

### Phase 3: 기존 뷰 보강 (3차)
13. `views/protocol.rs` — 동적화
14. `views/security.rs` — 권한 + 감사 보강
15. `views/memory.rs` — 검색 + 생성
16. `views/workspace.rs` — 파일 편집
17. `views/personas.rs` — CRUD
18. `views/skills.rs` — 생성 + 상세
19. `views/programs.rs` — 설치 + 삭제

### Phase 4: 스타일 + 마무리
20. `style.css` — 모달, 탭, 폼, 게이지 등 신규 스타일
21. 전체 빌드 + WASM 빌드
22. 통합 테스트

---

## 5. 공통 패턴

### 5.1 모달 폼 (생성/편집)

```rust
// 모든 생성 폼의 공통 패턴
#[component]
pub fn CreateModal(on_close: EventHandler<()>, on_created: EventHandler<()>) -> Element {
    let mut name = use_signal(String::new);
    let mut saving = use_signal(|| false);
    
    rsx! {
        div { class: "modal-overlay", onclick: move |_| on_close.call(()),
            div { class: "modal", onclick: move |e| e.stop_propagation(),
                div { class: "modal-header", h3 { "Create..." } }
                div { class: "modal-body",
                    div { class: "form-group",
                        label { class: "form-label", "Name" }
                        input { class: "form-input", r#type: "text", 
                                value: "{name}", oninput: move |e| name.set(e.value()) }
                    }
                }
                div { class: "modal-footer",
                    button { class: "btn", onclick: move |_| on_close.call(()), "Cancel" }
                    button { class: "btn btn-primary", disabled: saving(), onclick: move |_| { /* save */ },
                        if saving() { "Saving..." } else { "Create" }
                    }
                }
            }
        }
    }
}
```

### 5.2 리스트 + 액션

```rust
// 모든 리스트 뷰의 공통 패턴
#[component]
pub fn SomeListView() -> Element {
    let mut resource = use_resource(|| async move { api::fetch_paginated::<Type>("/api/...").await });
    let mut show_create = use_signal(|| false);
    
    rsx! {
        div { class: "panel-container",
            div { class: "panel-header",
                h2 { Icon { size: 20 } " Title" }
                div { class: "panel-actions",
                    button { class: "btn btn-sm btn-primary", onclick: move |_| show_create.set(true), "+ Create" }
                    button { class: "btn btn-sm", onclick: move |_| resource.restart(), "Refresh" }
                }
            }
            div { class: "panel-body",
                // ... items or loading/empty states
            }
        }
        if show_create() {
            CreateModal { on_close: move |_| show_create.set(false), on_created: move |_| { show_create.set(false); resource.restart(); } }
        }
    }
}
```

### 5.3 에러 피드백

모든 액션(생성/수정/삭제) 후 성공 시 `resource.restart()`로 갱신, 실패 시 글로벌 에러 토스트 자동 표시.

---

## 6. API 키 인증 흐름

```
최초 접속 (/)
    ↓
main.rs: localStorage.getItem("oxios-api-key")
    ↓ null
사이드바 하단에 "Set API Key" 버튼 표시
    ↓ 클릭
모달에서 API 키 입력 → localStorage 저장 → AUTH_TOKEN 업데이트
    ↓
이후 모든 API 호출에 Authorization: Bearer {key} 헤더 자동 포함
```

백엔드 `auth_enabled=false`인 경우 API 키 없이도 동작 (헤더가 없어도 401 안 남).
