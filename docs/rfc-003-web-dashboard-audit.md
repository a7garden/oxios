# RFC-003: Web Dashboard 감사 및 개선

> **상태**: Draft
> **작성일**: 2025-05-17
> **범위**: `channels/oxios-web/` 전체 (backend routes + frontend WASM)

---

## 1. 감사 요약

프론트엔드 25개 소스파일, 백엔드 13개 라우트 모듈, CSS, 미들웨어를 전수 조사했다.
총 **21개** 이슈를 발견했고, 심각도에 따라 4단계로 분류했다.

| 심각도 | 개수 | 의미 |
|--------|------|------|
| 🔴 Critical | 3 | 기능이 완전히 동작하지 않거나 잘못된 데이터 표시 |
| 🟠 High | 6 | 중요한 UX 결함 또는 데이터 유실 |
| 🟡 Medium | 7 | 개선이 필요한 설계/품질 문제 |
| 🔵 Low | 5 | 정리, 관례, 중복 제거 |

---

## 2. 이슈 상세 및 해결 방안

---

### 🔴 C-1. `rate_remaining`이 항상 `rate_limit_per_minute`와 동일

**파일**: `src/routes/infra.rs` — `handle_scheduler_stats`

**현상**:
```rust
rate_remaining: stats.rate_limit_per_minute,  // BUG: 실제 remaining이 아님
```

`rate_remaining`이 실제 남은 토큰 수가 아니라 limit 값을 그대로 복사.
스케줄러 UI의 "Rate Limit" 프로그레스 바가 항상 100%로 표시됨.

**해결**:
```rust
// KernelHandle의 scheduler_stats()가 rate_remaining을 제공해야 함.
// SchedulerStats 구조체에 실제 remaining 필드가 있는지 확인 후 수정.
rate_remaining: stats.rate_remaining,
```

커널에 `scheduler.rate_limit_remaining()` 메서드가 이미 존재함.
`SchedulerStats`에 `rate_remaining: u32` 필드를 추가하고 `stats()`에서 채우면 됨.

---

### 🔴 C-2. Events 뷰 `event_type` 파싱이 JSON의 키 이름을 반환

**파일**: `frontend/src/views/events.rs` L53-58

**현상**:
```rust
v.as_object().and_then(|obj| obj.keys().next().cloned())
// 항상 "type"을 반환 (JSON의 첫 번째 키)
```

백엔드가 `{"type": "agent_created", "agent_id": "..."}` 형태로 보내는데,
프론트엔드가 `"type"`이라는 **키 이름**을 가져옴. 화면에 모든 이벤트가 "type"으로 표시됨.

**해결**:
```rust
v.as_object()
 .and_then(|obj| obj.get("type"))
 .and_then(|v| v.as_str())
 .map(String::from)
 .unwrap_or_else(|| "unknown".to_string())
```

---

### 🔴 C-3. `fetch_json`이 HTTP 에러 상태를 무시

**파일**: `frontend/src/api/mod.rs` — `fetch_json`, `post_json`, `put_json`, `delete_json`

**현상**:
```rust
.send().await.map_err(...)?
.json::<T>().await.map_err(...)  // 4xx/5xx여도 JSON 파싱 시도
```

`gloo-net`은 HTTP 에러 응답도 `Ok(Response)`로 반환함.
401, 404, 500 응답 본문을 `T`로 디코드하려 시도 → 의미 없는 디코드 에러.

**해결**: 공통 응답 체크 함수 추가:
```rust
fn check_status(response: gloo_net::http::Response, method: &str, path: &str) -> Result<(), String> {
    let status = response.status();
    if status >= 400 {
        // 에러 본문을 읽어서 메시지에 포함
        Err(format!("{method} {path}: HTTP {status}"))
    } else {
        Ok(())
    }
}
```

모든 fetch 함수에서 `.json()` 호출 전에 상태 체크 삽입.
`post_action`/`delete_action`도 동일하게 수정.

---

### 🟠 H-1. Programs `enabled`/`has_skill_content`가 항상 `false`

**파일**: `src/routes/resources.rs` L26-28

**원인**: `kernel_handle/extension_api.rs`의 `list_programs()`가 `.map(|p| p.meta)`로 `enabled`/`skill_content`를 버리고 `Vec<ProgramMeta>`만 반환.
커널 `ProgramManager::list_programs()`는 `Vec<Program>` (enabled 포함)을 반환하지만, facade에서 필드 손실.

**해결**: `extension_api.rs`의 `list_programs()`가 `Vec<Program>`을 반환하도록 수정:

---

### 🟠 H-2. `/api/config`가 API 키를 평문으로 노출

**파일**: `src/routes/system.rs` — `handle_config_get`

**현상**: `GET /api/config`가 `OxiosConfig` 전체를 JSON으로 반환.
`security.api_key`, `security.auth_token` 등이 포함될 수 있음.

**해결**: 민감 필드를 마스킹하는 직렬화 적용:
```rust
// 방안 1: 전용 응답 구조체 사용
pub struct ConfigResponse {
    pub gateway: GatewayConfig,
    pub security: SecurityConfigSafe,  // api_key 제외
    // ...
}

// 방안 2: serde skip_serializing
#[serde(skip_serializing)]
pub api_key: Option<String>,
```

`OxiosConfig`의 보안 관련 필드에 `#[serde(skip_serializing)]` 추가가 가장 간단.

---

### 🟠 H-3. 대시보드 StatCard가 의미 없는 지표 표시

**파일**: `frontend/src/views/dashboard.rs`

**현상**:
| 카드 | 값 | 문제 |
|------|----|------|
| Uptime | `s.uptime` (string) | OK |
| Active Agents | `components.agents.active_count` | OK |
| Memory Entries | `components.memory.total_entries` | 메모리 카운트인데 "Workspaces" 라벨 |
| Status | `s.status` ("running") | 정적 문자열, 카드 낭비 |
| Version | `s.version` | OK |

"Memory Entries"와 "Status" 카드가 의미 없음.

**해결**: 5개 카드를 실제 유용한 지표로 교체:

```
┌──────────┐ ┌──────────────┐ ┌──────────────┐ ┌───────────────┐ ┌─────────┐
│  Uptime  │ │ Active Agents│ │Total Forked  │ │   Memory      │ │ Version │
│ 2h 34m 0s│ │      3       │ │    127       │ │  1,024 entries│ │  0.1.0  │
└──────────┘ └──────────────┘ └──────────────┘ └───────────────┘ └─────────┘
```

- Uptime → `s.uptime` (유지)
- Active Agents → `components.agents.active_count` (유지)
- Total Forked → `components.agents.total_forked` (새로)
- Memory Entries → `components.memory.total_entries` (라벨 수정)
- Version → `s.version` (유지)

---

### 🟠 H-4. Events 뷰에 EventSource 재연결 없음

**파일**: `frontend/src/views/events.rs`

**현상**: 연결 끊어지면 `connected = false`만 설정. 사용자가 수동으로 새로고침해야 함.

**해결**: 지수 백오프 재연결 로직 추가:
```
연결 끊김 → 1초 대기 → 재시도 → 실패 → 2초 → 4초 → 8초 → 최대 30초
```

구현: `spawn` 블록 안에 재시도 루프 추가:
```rust
let mut backoff = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

loop {
    match EventSource::new("/api/events") {
        Ok(mut es) => {
            connected.set(true);
            backoff = Duration::from_secs(1); // 리셋
            // ... 기존 메시지 루프 ...
            connected.set(false);
        }
        Err(_) => { connected.set(false); }
    }
    gloo_timers::future::TimeoutFuture::new(backoff.as_millis() as u32).await;
    backoff = (backoff * 2).min(MAX_BACKOFF);
}
```

---

### 🟠 H-5. Config 뷰가 읽기 전용

**파일**: `frontend/src/views/config.rs`

**현상**: `GET /api/config` 결과를 `<pre>`로만 표시.
`PUT /api/config` 백엔드가 있지만 프론트엔드에 편집 UI가 없음.

**해결**: TOML 편집 모드 추가:

```
┌─ Config ──────────────────────────── ─┐
│  [Edit] [Save]          [Refresh]     │
│ ┌────────────────────────────────────┐│
│ │ [gateway]                          ││
│ │ host = "127.0.0.1"                 ││
│ │ port = 3000                        ││
│ │ ...                                ││
│ └────────────────────────────────────┘│
└───────────────────────────────────────┘
```

구현:
- `ConfigView`에 `editing: Signal<bool>` 상태 추가
- 편집 모드에서 `<textarea>`로 TOML 표시
- Save → JSON 대신 TOML로 `PUT /api/config` 전송
- 백엔드 `handle_config_put`이 TOML도 받을 수 있도록 수정 필요

---

### 🟠 H-6. Sidebar에 16개 항목 — 섹션 그룹핑 필요

**파일**: `frontend/src/components/sidebar.rs`

**현상**: 모든 패널이 평면 나열. 스크롤이 길어지고 관련 기능을 찾기 어려움.

**해결**: 논리적 섹션으로 그룹화:

```
OXIOS                          [☀️] [◀]

── Core ──────────────
 💬 Chat
 📊 Dashboard
 ⚙️  Config

── Agents ─────────────
 🤖 Agents
 🧠 Personas
 📋 Scheduler

── Ouroboros ──────────
 🔄 Protocol
 🌱 Seeds

── System ─────────────
 📁 Workspace
 ⭐ Skills
 📦 Programs
 🔮 Memory
 🔧 Host Tools

── Security ───────────
 🛡️  Security
 ✅ Approvals

── Monitor ────────────
 📡 Events
```

구현:
- `NavItem`에 `section: Option<&'static str>` 추가
- `NAV_ITEMS` 배열에 섹션 정보 추가
- CSS에 `.nav-section`, `.nav-section-label` 클래스 추가
- 같은 섹션 아이템 사이에 구분선/라벨 렌더링

---

### 🟡 M-1. `post_action`/`delete_action`이 에러를 삼킴

**파일**: `frontend/src/api/mod.rs`

**현상**: Kill agent, approve/reject 등의 액션이 서버 에러여도 `Ok(())` 반환.
UI에서 성공한 것처럼 보이지만 실제로는 실패함.

**해결**:
```rust
pub async fn post_action(path: &str) -> Result<(), String> {
    let resp = gloo_net::http::Request::post(path)
        .send()
        .await
        .map_err(|e| format!("POST {path}: {e}"))?;
    if resp.status() >= 400 {
        return Err(format!("POST {path}: HTTP {}", resp.status()));
    }
    Ok(())
}
```

`delete_action`도 동일하게 수정.

---

### 🟡 M-2. 프론트엔드 API 레이어에 에러 바운더리 없음

**현상**: API 실패 시 각 뷰가 개별적으로 에러를 표시.
사용자가 다른 패널로 이동하면 에러를 놓침.

**해결**: 전역 에러 토스트 시스템 도입:

```rust
// api/mod.rs
pub fn last_api_error() -> Signal<Option<String>> { ... }
pub fn clear_api_error() { ... }

// 모든 fetch 함수에서 에러 발생 시 last_api_error 설정
```

```rust
// layout.rs — AppLayout에 토스트 오버레이 추가
if let Some(err) = api::last_api_error()() {
    rsx! {
        div { class: "toast-overlay",
            div { class: "toast toast-error",
                "{err}"
                button { onclick: |_| api::clear_api_error(), "×" }
            }
        }
    }
}
```

---

### 🟡 M-3. Events 뷰의 `started` 플래그가 리렌더 간 유지됨

**파일**: `frontend/src/views/events.rs`

**현상**:
```rust
let mut started = use_signal(|| false);
if !started() {
    started.set(true);
    spawn(async move { ... });
}
```

`use_signal`의 초기값은 `false`지만, 컴포넌트가 리렌더되어도 `started`는 `true`로 유지됨.
이건 Dioxus 0.7에서 의도된 동작이므로 OK. 다만 `started` 대신 `use_effect` 사용이 더 관용적.

---

### 🟡 M-4. Chat 뷰가 SSE 스트리밍 미사용

**파일**: `frontend/src/views/chat.rs`

**현상**: `POST /api/chat`만 사용. `/api/chat/stream` 엔드포인트가 있지만 활용 안 함.

**해결**: 2단계로 구현:
1. **Phase 1** (지금): 현재 구조 유지, 에러 처리만 개선
2. **Phase 2** (별도 RFC): SSE 스트리밍 채팅 구현
   - `EventSource`로 `/api/chat/stream` 연결
   - 토큰 단위 스트리밍 표시
   - 타이핑 인디케이터

현재 감사 범위에서는 Phase 1만 다룸.

---

### 🟡 M-5. Workspace breadcrumb 배치

**파일**: `frontend/src/views/workspace.rs`

**현상**: breadcrumb이 오른쪽 파일 뷰어 위쪽에 있음.
왼쪽 트리의 현재 위치를 직관적으로 보여주려면 트리 위쪽이 더 자연스러움.

**해결**:
```
┌─ Workspace ──────── [Refresh] ──────────────┐
│                                              │
│  ┌─ ~/seeds/evolution ─────┐  ┌──────────┐ │
│  │  [..]                    │  │ file.md  │ │
│  │  📁 gen-1               │  │ contents │ │
│  │  📁 gen-2               │  │          │ │
│  │  📄 summary.md          │  │          │ │
│  └──────────────────────────┘  └──────────┘ │
└──────────────────────────────────────────────┘
```

breadcrumb을 트리 패널 상단으로 이동.

---

### 🟡 M-6. Rate limiter가 전역 하나뿐

**파일**: `src/middleware.rs`

**현상**: 모든 API 엔드포인트가 동일한 토큰 버킷 공유.
채팅 API 호출이 잦으면 다른 모든 API가 막힘.

**해결**: 엔드포인트 그룹별 rate limiter 분리:
```rust
pub struct RateLimiters {
    pub chat: RateLimiter,      // 20/min (높음)
    pub general: RateLimiter,   // 60/min (기본)
    pub write: RateLimiter,     // 30/min (리소스 변경)
}
```

라우트 빌더에서 엔드포인트별로 다른 limiter 적용.

---

### 🟡 M-7. 대시보드가 `_scheduler` 리소스를 사용 안 함

**파일**: `frontend/src/views/dashboard.rs`

**현상**:
```rust
let _scheduler = use_resource(|| async move {
    api::fetch_json::<SchedulerStatsResponse>("/api/scheduler/stats").await
});
```

`_scheduler`의 결과를 렌더링에 사용하지 않음. 불필요한 API 호출.

**해결**: 제거하거나 stat card에 활용 (Queued/Running 카드 추가).

---

### 🔵 L-1. Dead types 정리

**파일**: `frontend/src/api/mod.rs`

**현상**: 12개의 레거시 타입이 `#[allow(dead_code)]`로 방치됨:
- `AgentInfo`, `SeedInfo`, `MemoryEntry`, `AuditEntry`, `ApprovalInfo`
- `ProgramInfo`, `HostToolStatus`, `PersonaInfo`, `ConfigResponse`
- `TreeEntryOld`, `SseEvent`, `McpServerInfo`

**해결**: 모두 삭제. 나중에 필요하면 그때 추가.

---

### 🔵 L-2. `placeholder.rs` 미사용

**파일**: `frontend/src/views/placeholder.rs`

**현상**: `PlaceholderView`가 어느 뷰에서도 사용되지 않음.
`mod.rs`에 `pub mod placeholder;`만 있음.

**해결**: `placeholder.rs`와 `mod.rs`의 `pub mod placeholder;` 삭제.

---

### 🔵 L-3. 중복 CSS 파일

**현상**: `channels/oxios-web/static/style.css`와
`channels/oxios-web/frontend/static/style.css`가 완전히 동일함.

**해결**: `frontend/static/style.css`만 유지.
`channels/oxios-web/static/`의 `style.css`를 심볼릭 링크로 변경하거나 제거.
빌드 스크립트가 하나의 소스만 참조하도록 수정.

---

### 🔵 L-4. 미사용 CSS 클래스

**현상**: 정의되었지만 HTML에서 사용되지 않는 클래스:
- `.card`, `.card-header`, `.card-grid`
- `.approval-card`, `.approval-actions`, `.approval-status`
- `.seed-card`, `.seed-name`, `.seed-spec`
- `.memory-entry`, `.memory-key`, `.memory-value`
- `.tab-bar`, `.tab-item`
- `.progress-bar`, `.progress-fill`
- `.tool-grid`, `.tool-item`, `.tool-status`

**해결**: 사용하지 않는 클래스는 향후 사용 가능성을 고려해 유지하되,
주석으로 분류 정리. 실제 사용하는 클래스만 남기는 것도 옵션.

---

### 🔵 L-5. `fetch_text`가 파일 뷰어에만 쓰임

**현상**: `fetch_text`가 `workspace.rs`의 파일 내용 조회에만 사용됨.
파일이 바이너리면 깨진 텍스트가 표시됨.

**해결**: 바이너리 파일 감지 시 "Binary file" 메시지 표시:
```rust
match api::fetch_text(&format!("/api/workspace/file/{full}")).await {
    Ok(c) => {
        // null 바이트가 있으면 바이너리로 판단
        if c.contains('\0') {
            file_content.set("Binary file (cannot display)".to_string());
        } else {
            file_content.set(c);
        }
    }
    Err(e) => file_content.set(format!("Error: {e}")),
}
```

---

## 3. 구현 순서

동일 파일 수정을 그룹화하여 최소 커밋 수로 구현.

### Phase 1: Critical 수정 (API 레이어 + 백엔드 버그)

| # | 작업 | 파일 | 변경 크기 |
|---|------|------|-----------|
| 1 | HTTP 상태 체크 추가 | `frontend/src/api/mod.rs` | M |
| 2 | Events event_type 파싱 수정 | `frontend/src/views/events.rs` | S |
| 3 | rate_remaining 버그 수정 | `src/routes/infra.rs` | S |

### Phase 2: High 수정 (데이터 정확성 + UX)

| # | 작업 | 파일 | 변경 크기 |
|---|------|------|-----------|
| 4 | Programs enabled/has_skill_content 수정 | `src/routes/resources.rs` | S |
| 5 | /api/config 민감 필드 마스킹 | `src/routes/system.rs` 또는 커널 | S |
| 6 | 대시보드 stat cards 교체 | `frontend/src/views/dashboard.rs` | S |
| 7 | Events 재연결 로직 | `frontend/src/views/events.rs` | M |
| 8 | Config 편집 모드 | `frontend/src/views/config.rs` | M |
| 9 | Sidebar 섹션 그룹핑 | `frontend/src/components/sidebar.rs` + CSS | M |

### Phase 3: Medium 수정 (품질 개선)

| # | 작업 | 파일 | 변경 크기 |
|---|------|------|-----------|
| 10 | post_action/delete_action 에러 처리 | `frontend/src/api/mod.rs` | S |
| 11 | 전역 에러 토스트 | `api/mod.rs` + `layout.rs` + CSS | M |
| 12 | Workspace breadcrumb 이동 | `frontend/src/views/workspace.rs` | S |
| 13 | 미사용 _scheduler 제거 | `frontend/src/views/dashboard.rs` | S |
| 14 | 바이너리 파일 감지 | `frontend/src/views/workspace.rs` | S |

### Phase 4: Low 정리

| # | 작업 | 파일 | 변경 크기 |
|---|------|------|-----------|
| 15 | Dead types 삭제 | `frontend/src/api/mod.rs` | S |
| 16 | placeholder.rs 제거 | `frontend/src/views/` | S |
| 17 | 중복 CSS 해소 | `channels/oxios-web/static/` | S |
| 18 | Rate limiter 분리 | `src/middleware.rs` + `plugin.rs` | M |
| 19 | CSS 정리 (선택) | `style.css` | S |

---

## 4. 파일별 변경 요약

```
channels/oxios-web/
├── src/
│   ├── routes/
│   │   ├── infra.rs            # C-1: rate_remaining 수정
│   │   ├── resources.rs        # H-1: programs enabled 수정
│   │   └── system.rs           # H-2: config 민감 필드 마스킹
│   └── middleware.rs            # M-6: rate limiter 분리 (Phase 4)
│
├── frontend/src/
│   ├── api/mod.rs              # C-3: HTTP 상태 체크, M-1: 에러 처리, L-1: dead types
│   ├── components/
│   │   ├── sidebar.rs          # H-6: 섹션 그룹핑
│   │   └── layout.rs           # M-2: 에러 토스트 오버레이
│   ├── views/
│   │   ├── dashboard.rs        # H-3: stat cards, M-7: _scheduler 제거
│   │   ├── events.rs           # C-2: event_type, H-4: 재연결
│   │   ├── config.rs           # H-5: 편집 모드
│   │   ├── workspace.rs        # M-5: breadcrumb, L-5: 바이너리 감지
│   │   └── placeholder.rs      # L-2: 삭제
│   └── views/mod.rs            # L-2: mod placeholder 제거
│
└── static/
    └── style.css               # H-6: 섹션 CSS, M-2: 토스트 CSS
                                # L-3: 중복 해소, L-4: 미사용 클래스 정리
```

---

## 5. 위험 및 고려사항

1. **rate_remaining**: 커널 `SchedulerStats`에 필드가 없으면 커널 수정 필요.
   있으면 infra.rs 1줄 수정으로 끝남. 사전 확인 필요.

2. **Config 편집**: 백엔드 `handle_config_put`이 현재 JSON만 받음.
   TOML 편집 UI를 원하면 백엔드에서 TOML 파싱도 지원해야 함.
   대안: 프론트엔드에서 JSON으로 편집 (현재 백엔드 호환).

3. **Rate limiter 분리**: `AppState` 구조 변경 필요.
   기존 `rate_limiter: RateLimiter`를 `rate_limiters: RateLimiters`로 변경.

4. **재연결 로직**: Events 뷰의 `started` 패턴이 `spawn`과 결합되어 있어
   재시도 루프를 추가하려면 전체 구조를 재작성해야 함.

---

## 6. 검증 방법

각 Phase 완료 후:

```bash
# Frontend 컴파일
cd channels/oxios-web/frontend
cargo check --target wasm32-unknown-unknown

# Backend 컴파일 (커널 에러 해결 후)
cargo check -p oxios-web

# 수동 테스트
# 1. 대시보드 — stat cards가 올바른 값 표시
# 2. Events — event_type이 "type"이 아닌 실제 이벤트명
# 3. Programs — enabled/disabled 상태가 실제와 일치
# 4. Config — API 키가 마스킹되어 표시
# 5. Kill agent — 실패 시 에러 메시지 표시
```
