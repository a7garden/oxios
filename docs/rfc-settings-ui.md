# RFC: Settings UI — Form-Based Configuration

> **Status**: Draft (v2 — 리뷰 반영)
> **Date**: 2025-05-19
> **Scope**: `channels/oxios-web/frontend/src/views/config.rs`, backend `system.rs`

---

## 1. 문제 (Problem)

현재 Web UI의 Config 뷰(`ConfigView`)는 전체 설정을 raw JSON 텍스트로 보여주고, 수정 시에도 JSON textarea를 직접 편집해야 한다.

**문제점:**
- 사용자가 TOML 스키마를 알아야 함
- 오타 하나로 전체 설정이 깨짐 (400 에러)
- `true`/`false` 같은 boolean을 직접 타이핑
- 숫자 범위를 검증할 수 없음 (예: port 0~65535)
- 어떤 설정이 있는지, 기본값이 뭔지 알기 어려움
- 위험한 설정(`allow_shell_mode`)의 경고가 없음

## 2. 제안 (Proposal)

Config 뷰를 **카테고리별 탭 + 폼 컴포넌트** 기반으로 재설계한다.
각 설정 필드에 적절한 UI 위젯(토글, 슬라이더, 셀렉트, 태그 리스트 등)을 제공하고,
전체 설정을 PUT으로 백엔드에 저장한다.

### 핵심 원칙

1. **No raw text editing** — 모든 필드는 전용 위젯으로 편집
2. **Logical grouping** — 관련 설정을 자연스럽게 그룹화하여 탭 수 최소화
3. **Inline validation** — 저장 전 필드 단위 검증
4. **Safety indicators** — 위험 설정에 명확한 경고
5. **Saved ≠ Applied** — hot-reload 불가 필드에 "restart required" 명시
6. **기존 JSON 편집 모드 유지** — 고급 사용자를 위해 "Advanced" 탭으로 이동

---

## 3. 아키텍처 제약

### 3.1 프론트엔드가 `OxiosConfig` 타입을 사용할 수 없음

WASM 프론트엔트(`oxios-frontend`)는 `oxios-kernel`에 의존하지 않는다.
현재 `serde_json::Value`로 설정을 다루며, 이 구조를 유지한다.

**해결**: 프론트엔드에 **미러 타입**을 정의한다.

```rust
// components/settings/config_types.rs
// OxiosConfig의 필드 레이아웃을 미러링하는 frontend 전용 타입.
// oxios-kernel의 실제 타입과 sync를 맞춰야 함.

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub kernel: KernelSnapshot,
    pub engine: EngineSnapshot,
    pub daemon: DaemonSnapshot,
    pub gateway: GatewaySnapshot,
    pub scheduler: SchedulerSnapshot,
    pub orchestrator: OrchestratorSnapshot,
    pub context: ContextSnapshot,
    pub security: SecuritySnapshot,
    pub persona: PersonaSnapshot,
    pub memory: MemorySnapshot,
    pub cron: CronSnapshot,
    pub mcp: McpSnapshot,
    pub git: GitSnapshot,
    pub audit: AuditSnapshot,
    pub budget: BudgetSnapshot,
    pub exec: ExecSnapshot,
    pub resource_monitor: ResourceMonitorSnapshot,
    pub otel: OtelSnapshot,
    pub channels: ChannelsSnapshot,
    pub browser: BrowserSnapshot,
}
```

- `serde_json::Value` → `ConfigSnapshot`으로 파싱하여 Signal에 보관
- Save 시 `ConfigSnapshot` → `serde_json::Value`로 직렬화하여 PUT
- 스키마 불일치는 `Deserialize` 에러로 감지

### 3.2 "Hot-reload"의 현실

백엔드 `handle_config_put`은 `state.config` RwLock만 업데이트한다:

```rust
// system.rs — 실제 코드
*state.config.write() = updated;
```

하지만 커널 컴포넌트(Scheduler, Orchestrator, SecurityManager 등)는 **빌드 타임에 config 값을 복사**하여 소유한다:

```rust
// kernel.rs — 실제 코드
let scheduler = Scheduler::new(
    config.scheduler.max_concurrent,    // ← 복사됨
    config.scheduler.rate_limit_per_minute,
    config.scheduler.zombie_timeout_secs,
);
```

즉, `state.config`가 바뀌어도 이미 생성된 커널 컴포넌트에는 반영되지 않는다.

**설계에 미치는 영향:**
- 설정 저장 → 디스크에 persist + `state.config` 업데이트는 성공
- 하지만 **실제 런타임 동작은 restart 전까지 변경되지 않음**
- UI는 이 차이를 솔직하게 표현해야 함

**대응 전략:**

| 전략 | 설명 | 채택 |
|------|------|------|
| A. 전체 restart required | 모든 설정에 표시 | ✅ Phase 1 |
| B. 필드별 분류 | hot-reload 가능/불가 분류 | Phase 2 (필드 분석 후) |
| C. 커널 리팩토링 | `Arc<RwLock<Config>>` 참조 | Phase 3 (장기) |

Phase 1에서는 **전역 Save + "Restart to apply changes" 배너**를 표시한다.
필드별로 hot-reload 가능 여부를 정확히 분류하는 건 별도 작업이며,
현실적으로 대부분의 설정이 restart를 필요로 하므로 전역 표시가 솔직하다.

### 3.3 API Key의 양방향 문제

`EngineConfig.api_key`는 `#[serde(skip_serializing)]`이므로 GET 응답에서 **아예 누락**된다.
사용자가 "키가 설정되어 있는지" 알 수 없다.

**해결 (백엔드 수정 최소화):**

```rust
// handle_config_get — 수동 마스킹
let mut json = serde_json::to_value(&*config)?;
// api_key는 skip_serializing이므로 누락됨 → 별도 필드 추가
if config.engine.api_key.is_some() {
    json["engine"]["api_key_set"] = serde_json::Value::Bool(true);
} else {
    json["engine"]["api_key_set"] = serde_json::Value::Bool(false);
}
```

프론트엔드 PasswordInput 위젯은 `api_key_set: bool`로 상태를 표시하고,
PUT 시 새 키가 입력되면 `engine.api_key` 필드를 포함하여 전송,
빈 문자열이면 "변경 없음" 의미로 생략.

---

## 4. UI 레이아웃

```
┌───────────────────────────────────────────────────┐
│  ⚙ Settings                              [Save ▾] │
├───────────────────────────────────────────────────┤
│ [General] [Engine] [Exec & Security] [Agents]     │
│ [Memory & Context] [Integrations] [Monitoring]    │
│ [Advanced]                                         │
├───────────────────────────────────────────────────┤
│  ℹ️ Changes saved. Restart Oxios to apply.         │
├───────────────────────────────────────────────────┤
│                                                    │
│  ┌─ Kernel ────────────────────────────────────┐  │
│  │  Workspace  [~/.oxios/workspace      ]      │  │
│  │  Max Agents          [===●====] 10 / 64     │  │
│  │  Event Bus Capacity  [256        ]          │  │
│  └─────────────────────────────────────────────┘  │
│                                                    │
│  ┌─ Gateway ───────────────────────────────────┐  │
│  │  Host  [127.0.0.1]                          │  │
│  │  Port  [4200       ]                        │  │
│  └─────────────────────────────────────────────┘  │
│                                                    │
│  ┌─ Daemon ────────────────────────────────────┐  │
│  │  PID File  [~/.oxios/oxios.pid]             │  │
│  │  Log Dir   [~/.oxios/logs       ]           │  │
│  └─────────────────────────────────────────────┘  │
│                                                    │
│  [Reset to Defaults]                               │
│                                                    │
└───────────────────────────────────────────────────┘
```

**Save 버튼은 상단에 전역 1개만.** 탭 전환 시 dirty 상태를 배지로 표시.
저장 시 모든 탭의 변경사항을 포함하여 PUT.

---

## 5. 카테고리 & 필드 매핑 (전체 커버리지)

`OxiosConfig`의 **모든** 하위 구조체를 매핑. 8개 탭으로 그룹화.

### Tab 1: General (kernel + daemon + gateway + git)

| Field | Type | Widget | Range/Options |
|-------|------|--------|---------------|
| `kernel.workspace` | String | Text input (path) | — |
| `kernel.max_agents` | u32 | Slider | 1 – 64 |
| `kernel.event_bus_capacity` | u32 | Number input | 16 – 4096 |
| `gateway.host` | String | Text input | — |
| `gateway.port` | u16 | Number input | 1 – 65535 |
| `daemon.pid_file` | String | Text input | — |
| `daemon.log_dir` | String | Text input | — |
| `git.auto_commit` | bool | Toggle | — |

> `git.auto_commit`은 필드 1개뿐이라 General에 흡수.

### Tab 2: Engine

| Field | Type | Widget | Notes |
|-------|------|--------|-------|
| `engine.default_model` | String | Text input | `provider/model` 형식, placeholder 가이드 |
| `engine.api_key` | String | Password input | `api_key_set`으로 상태 표시, 실제 값은 마스킹 |

### Tab 3: Exec & Security (exec + security)

관련성이 높은 실행 정책과 보안 정책을 한 탭에. 섹션 카드로 구분.

| Field | Type | Widget | Range/Options |
|-------|------|--------|---------------|
| **Exec** ||||
| `exec.default_mode` | Enum | Toggle button group | `Structured` / `Shell` |
| `exec.allow_shell_mode` | bool | Toggle + ⚠️ danger | "위험 — 에이전트가 임의 bash 실행 가능" |
| `exec.allowed_commands` | Vec\<String\> | Tag input | `git`, `gh`, `open` 등 |
| `exec.default_timeout_secs` | u64 | Slider | 10 – 600 |
| `exec.max_timeout_secs` | u64 | Number input | 30 – 3600 |
| **Security** ||||
| `security.auth_enabled` | bool | Toggle | — |
| `security.network_access` | bool | Toggle + ⚠️ warning | "에이전트 네트워크 접근 허용" |
| `security.can_fork` | bool | Toggle + ⚠️ warning | "에이전트 하위 에이전트 생성 허용" |
| `security.allowed_tools` | Vec\<String\> | Multi-checkbox | 도구 목록 |
| `security.max_execution_time_secs` | u64 | Slider | 30 – 3600 |
| `security.max_memory_mb` | u64 | Slider | 128 – 4096 |
| `security.cors_origins` | Vec\<String\> | Tag input | — |
| `security.rate_limit_per_minute` | u32 | Slider | 10 – 1000 |
| `security.max_audit_entries` | u32 | Number input | 100 – 1,000,000 |
| `security.audit_log_path` | Option\<String\> | Text input | 빈 값 = 비활성화 |

### Tab 4: Agents (scheduler + orchestrator + persona + budget)

에이전트 OS의 핵심 — 에이전트 관련 설정을 한 곳에.

| Field | Type | Widget | Range |
|-------|------|--------|-------|
| **Scheduler** ||||
| `scheduler.max_concurrent` | u32 | Slider | 1 – 32 |
| `scheduler.rate_limit_per_minute` | u32 | Slider | 1 – 600 |
| `scheduler.zombie_timeout_secs` | u64 | Slider | 30 – 1800 |
| **Orchestrator** ||||
| `orchestrator.max_evolution_iterations` | u32 | Number input | 1 – 10 |
| `orchestrator.min_evaluation_score` | f64 | Slider | 0.0 – 1.0 (step 0.05) |
| **Persona** ||||
| `persona.default_persona_id` | Option\<String\> | Text input | 기본값 "dev" |
| `persona.max_concurrent_personas` | u32 | Number input | 1 – 20 |
| **Budget** ||||
| `budget.enabled` | bool | Toggle | — |
| `budget.default_token_budget` | u64 | Number input + unlimited toggle | 0 = unlimited |
| `budget.default_calls_budget` | u64 | Number input + unlimited toggle | 0 = unlimited |
| `budget.default_window_secs` | u64 | Slider | 60 – 86400 |

> `budget.default_token_budget` 등은 0이 "무제한"을 의미.
> Number input 아래에 "0 = unlimited" 헬퍼를 표시하거나,
> 별도 "Unlimited" 토글 체크 시 입력 필드를 비활성화.

### Tab 5: Memory & Context (memory + context)

LLM 메모리/컨텍스트 관련 설정.

| Field | Type | Widget | Range |
|-------|------|--------|-------|
| **Memory** ||||
| `memory.enabled` | bool | Toggle | — |
| `memory.max_recall` | u32 | Number input | 1 – 100 |
| `memory.auto_summarize` | bool | Toggle | — |
| `memory.capture_compaction` | bool | Toggle | — |
| `memory.retention_days` | u32 | Number input | 0 (무제한) – 365 |
| `memory.cache_enabled` | bool | Toggle | — |
| `memory.cache_ttl_secs` | u64 | Slider | 60 – 86400 |
| `memory.cache_max_entries` | u32 | Number input | 100 – 100,000 |
| **Context** ||||
| `context.active_limit_tokens` | u32 | Number input | 1,000 – 1,000,000 |
| `context.cache_limit_entries` | u32 | Number input | 10 – 500 |

> `context.active_limit_tokens`은 모델별 컨텍스트 윈도우에 직접 영향.
> placeholder에 "GPT-4: 128000, Claude: 200000" 가이드 표시.

### Tab 6: Integrations (channels + mcp + cron + browser)

외부 연동 설정. MCP 서버 편집이 핵심.

| Field | Type | Widget | Notes |
|-------|------|--------|-------|
| **Channels** ||||
| `channels.enabled` | Vec\<String\> | Multi-checkbox | web, cli, telegram |
| `channels.telegram.bot_token_env` | String | Text input | 환경변수명 |
| `channels.telegram.allowed_users` | Vec\<i64\> | Tag input (numeric) | Telegram user ID |
| **MCP Servers** ||||
| `mcp.servers` | HashMap\<String, McpServerDef\> | MCP Server Cards | 아래 별도 설명 |
| **Cron** ||||
| `cron.enabled` | bool | Toggle | — |
| `cron.tick_interval_secs` | u64 | Slider | 10 – 600 |
| **Browser** ||||
| `browser.enabled` | bool | Toggle | — |

#### MCP Server Cards

`McpServerDef`는 `command`, `args: Vec<String>`, `env: HashMap<String, String>`, `enabled: bool`을
가지므로 TagInput으로 표현 불가. **별도 편집 카드**가 필요.

```
┌─ MCP Servers ─────────────────────────────────────┐
│                                                     │
│  ┌─ filesystem ─────────── [✓ enabled] [🗑] ─────┐ │
│  │  Command  [npx                              ]  │ │
│  │  Args     [-y] [@modelcontextprotocol/...] [+] │ │
│  │  Env      [NODE_ENV=production]          [+]  │ │
│  └────────────────────────────────────────────────┘ │
│                                                     │
│  ┌─ github ────────────── [✓ enabled] [🗑] ──────┐ │
│  │  Command  [npx                              ]  │ │
│  │  Args     [-y] [@modelcontextprotocol/...] [+] │ │
│  │  Env      [GITHUB_TOKEN=***]              [+]  │ │
│  └────────────────────────────────────────────────┘ │
│                                                     │
│  [+ Add MCP Server]                                 │
└─────────────────────────────────────────────────────┘
```

인터랙션:
- 서버 이름: 카드 제목, 클릭하면 편집 (또는 항상 펼쳐진 상태)
- `args`: TagInput 변형 (문자열 리스트)
- `env`: Key=Value 편집용 TagInput (키/밸류 페어)
- `enabled`: 카드 우상단 토글
- 삭제: 휴지통 버튼 → 확인 모달
- 추가: "Add Server" → 이름 입력 모달 → 빈 카드 생성

### Tab 7: Monitoring (otel + audit + resource_monitor)

| Field | Type | Widget | Notes |
|-------|------|--------|-------|
| **OpenTelemetry** ||||
| `otel.enabled` | bool | Toggle | — |
| `otel.endpoint` | String | Text input | — |
| `otel.service_name` | String | Text input | — |
| `otel.sampling_ratio` | f64 | Slider | 0.0 – 1.0 |
| **Audit Trail** ||||
| `audit.enabled` | bool | Toggle | — |
| `audit.max_entries` | u32 | Number input | 100 – 1,000,000 |
| **Resource Monitor** ||||
| `resource_monitor.interval_secs` | u64 | Slider | 10 – 300 |
| `resource_monitor.cpu_threshold` | f32 | Slider | 50 – 100% |
| `resource_monitor.memory_threshold` | f32 | Slider | 50 – 100% |
| `resource_monitor.load_threshold` | f32 | Number input | 1.0 – 64.0 |
| `resource_monitor.history_max` | u32 | Number input | 10 – 500 |

### Tab 8: Advanced (기존 JSON 편집기)

기존의 JSON textarea 편집기를 유지하되 "Advanced" 탭으로 격하.
저장 전 백엔드 validation이 필수.

---

## 6. UI 위젯 컴포넌트 설계

### 6.1 Toggle (On/Off Switch)

```
 ───────●     (OFF)          ●───────  (ON)
 회색 배경                   accent 색상
```

- Boolean 필드 전용
- **div + onClick**으로 구현 (CSS ::after 의사요소는 Dioxus에서 이벤트 불가)
- 위험 설정(`allow_shell_mode`, `network_access`, `can_fork`)은 빨간색 배경 + "⚠️ DANGEROUS" 라벨

```rust
#[component]
fn SettingsToggle(
    /// 필드 레이블
    label: &'static str,
    /// 현재값
    value: bool,
    /// 값 변경 콜백
    onchange: EventHandler<bool>,
    /// 도움말 텍스트 (선택)
    description: Option<&'static str>,
    /// 위험 설정 여부
    dangerous: Option<bool>,
) -> Element;
```

### 6.2 Slider (Range)

```
 Max Agents  [====●============] 10 / 64
```

- `min`, `max`, `step` 속성
- 현재값을 오른쪽에 "{value} / {max}" 형식으로 표시
- 정수/소수 자동 판별 (step 기준)
- **범위가 넓은 필드**(event_bus_capacity 등)는 Number input 사용

```rust
#[component]
fn SettingsSlider(
    label: &'static str,
    value: f64,
    onchange: EventHandler<f64>,
    min: f64,
    max: f64,
    step: Option<f64>,           // default: 1.0
    unit: Option<&'static str>,  // "sec", "MB", "%"
    show_max: Option<bool>,      // "{value} / {max}" 표시
    description: Option<&'static str>,
) -> Element;
```

### 6.3 Number Input

```
 Port:  [4200]  (1 – 65535)
```

- `min`, `max` 경계를 입력란 아래에 표시
- 범위 벗어나면 red outline + 에러 메시지
- 범위가 넓은 정수 필드에 사용 (event_bus_capacity, port, max_entries 등)

```rust
#[component]
fn SettingsNumberInput(
    label: &'static str,
    value: f64,
    onchange: EventHandler<f64>,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,           // default: 1.0
    unit: Option<&'static str>,
    description: Option<&'static str>,
) -> Element;
```

### 6.4 Tag Input (String List)

```
 Allowed Commands:  [git] [gh] [open] [+ add]
```

- 태그 chip + X 버튼으로 개별 삭제
- 하단 input에서 Enter로 추가
- `allowed_commands`, `cors_origins`, `args` 등에 사용

```rust
#[component]
fn SettingsTagInput(
    label: &'static str,
    values: Vec<String>,
    onchange: EventHandler<Vec<String>>,
    placeholder: Option<&'static str>,
    description: Option<&'static str>,
) -> Element;
```

### 6.5 Key-Value Tag Input (MCP env 전용)

```
 Environment:  [NODE_ENV=production ×] [API_KEY=*** ×] [+ add]
```

- TagInput의 변형: `=`로 key/value 구분
- MCP 서버의 `env: HashMap<String, String>` 편집용

```rust
#[component]
fn SettingsKeyValueInput(
    label: &'static str,
    values: HashMap<String, String>,
    onchange: EventHandler<HashMap<String, String>>,
    placeholder: Option<&'static str>,
    description: Option<&'static str>,
) -> Element;
```

### 6.6 Toggle Button Group (Enum Select)

```
 Execution Mode:  [Structured ■]  [Shell]
```

- Enum variant 중 하나 선택
- `exec.default_mode`에 사용

```rust
#[derive(Clone, PartialEq)]
struct SelectOption {
    value: String,
    label: String,
}

#[component]
fn SettingsSelectGroup(
    label: &'static str,
    options: Vec<SelectOption>,
    selected: String,
    onchange: EventHandler<String>,
    description: Option<&'static str>,
) -> Element;
```

### 6.7 Multi-Checkbox (Set Select)

```
 Channels:  ☑ Web   ☐ CLI   ☐ Telegram
```

- `Vec<String>` 필드에서 옵션 목록이 정해진 경우

```rust
#[component]
fn SettingsMultiCheckbox(
    label: &'static str,
    options: Vec<SelectOption>,
    selected: Vec<String>,
    onchange: EventHandler<Vec<String>>,
    description: Option<&'static str>,
) -> Element;
```

### 6.8 Password Input

```
 API Key:  [••••••••••••]  👁  ✓ Set
```

- 기본 마스킹, eye 아이콘으로 토글
- `api_key_set: bool` 값이 true면 "✓ Set" 표시 (실제 값은 백엔드에서 주지 않음)
- PUT 시 빈 문자열이면 "변경 없음"으로 해석 (필드 생략)

```rust
#[component]
fn SettingsPasswordInput(
    label: &'static str,
    /// true = 키가 설정됨, false = 미설정
    is_set: bool,
    /// 사용자가 입력한 새 값 (빈 문자열 = 변경 없음)
    onchange: EventHandler<String>,
    description: Option<&'static str>,
) -> Element;
```

### 6.9 Section Card

```rust
#[component]
fn SectionCard(
    title: &'static str,
    description: Option<&'static str>,
    children: Element,
) -> Element;
```

---

## 7. 데이터 흐름

### 7.1 전체 흐름

```
Frontend                              Backend
   │                                     │
   │  GET /api/config                    │
   │ ──────────────────────────────────► │
   │   { full config JSON }              │
   │   + engine.api_key_set: bool        │ ← handle_config_get에서 수동 추가
   │ ◄────────────────────────────────── │
   │                                     │
   │  serde_json::Value                  │
   │    → ConfigSnapshot 파싱            │
   │    → Signal<ConfigSnapshot> 보관    │
   │                                     │
   │  (각 탭별 폼 위젯에 바인딩)          │
   │  (dirty 상태를 탭 배지로 표시)       │
   │                                     │
   │  [Save] 클릭 시                      │
   │  ConfigSnapshot                     │
   │    → serde_json::Value 직렬화       │
   │    → PUT /api/config                │
   │ ──────────────────────────────────► │
   │   { full modified config }          │
   │        200 OK + validate            │
   │ ◄────────────────────────────────── │
   │                                     │
   │  "✓ Saved. Restart to apply."       │
   └─────────────────────────────────────┘
```

### 7.2 상태 관리

```rust
// views/config/mod.rs

/// 전역 설정 편집 상태.
/// ConfigSnapshot을 Signal로 보관하고, 원본과 비교하여 dirty 판별.
struct SettingsState {
    /// GET으로 받아온 원본 (변경 비교용)
    original: Signal<ConfigSnapshot>,
    /// 편집 중인 설정
    editing: Signal<ConfigSnapshot>,
    /// 전체 dirty 여부
    dirty: Signal<bool>,
    /// 저장 결과 메시지
    save_message: Signal<Option<SaveMessage>>,
}

struct SaveMessage {
    success: bool,
    text: String,
}
```

**Save 전략**: 상단 전역 Save 버튼 1개.
- Dirty 상태가 있으면 Save 버튼이 accent 색상으로 하이라이트
- 탭 라벨에 dirty dot 표시 ("General ●")
- 탭 전환 시 "저장하시겠습니까?" 확인은 하지 않음 (자동 보존)
- Save 성공 시 원본을 현재값으로 업데이트 → dirty 해제

---

## 8. 백엔드 변경사항

### 8.1 Phase 1: 최소 변경

```
GET  /api/config       → 전체 설정 반환 (JSON) + api_key_set 필드 추가
PUT  /api/config       → 전체 설정 업데이트 (기존 로직 그대로)
```

**변경점 1개**: `handle_config_get`에 `api_key_set` 필드 추가.

```rust
pub(crate) async fn handle_config_get(
    state: State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let config = state.config.read();
    let mut json = serde_json::to_value(&*config)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // api_key는 skip_serializing이므로 누락됨 → 설정 여부만 추가
    json["engine"]["api_key_set"] = serde_json::Value::Bool(
        config.engine.api_key.is_some()
    );

    Ok(Json(json))
}
```

**PUT은 변경 없음.** 프론트엔드가 `api_key` 필드에 빈 문자열을 보내면
`EngineConfig` deserialization에서 `None`으로 처리 (기존 동작).

### 8.2 Phase 2 (선택)

```
GET  /api/config/schema    → 필드별 타입, 범위, 기본값, 설명 반환
PATCH /api/config/:section → 섹션별 부분 업데이트
```

### 8.3 Phase 3: 런타임 핫리로드 (장기)

커널 컴포넌트가 `Arc<RwLock<SectionConfig>>`를 참조하도록 리팩토링.
설정 변경 시 해당 컴포넌트가 즉시 반영.
이건 커널 아키텍처 변경이므로 별도 RFC에서 다룸.

---

## 9. 프론트엔드 파일 구조

```
channels/oxios-web/frontend/src/
├── views/
│   └── settings/                     ← 기존 config.rs 대체
│       ├── mod.rs                    ← SettingsView (탭 컨테이너 + 전역 Save)
│       ├── general_tab.rs            ← Kernel + Daemon + Gateway + Git
│       ├── engine_tab.rs             ← Engine 설정
│       ├── exec_security_tab.rs      ← Exec + Security
│       ├── agents_tab.rs             ← Scheduler + Orchestrator + Persona + Budget
│       ├── memory_context_tab.rs     ← Memory + Context
│       ├── integrations_tab.rs       ← Channels + MCP + Cron + Browser
│       ├── monitoring_tab.rs         ← OTel + Audit + ResourceMonitor
│       ├── advanced_tab.rs           ← 기존 JSON 편집기
│       └── config_types.rs           ← ConfigSnapshot 미러 타입
├── components/
│   └── settings/                     ← 재사용 위젯
│       ├── mod.rs                    ← pub mod + SelectOption 타입
│       ├── toggle.rs                 ← On/Off 스위치 (div onClick)
│       ├── slider.rs                 ← Range 슬라이더
│       ├── number_input.rs           ← 숫자 입력 (min/max)
│       ├── tag_input.rs              ← 태그 리스트 편집
│       ├── kv_input.rs               ← Key=Value 편집 (MCP env)
│       ├── select_group.rs           ← Toggle button group (enum)
│       ├── multi_checkbox.rs         ← Multi-checkbox (set)
│       ├── password_input.rs         ← 비밀번호 입력
│       ├── section_card.rs           ← 섹션 카드 래퍼
│       └── mcp_server_card.rs        ← MCP 서버 편집 카드
```

기존 `views/config.rs`는 `views/settings/advanced_tab.rs`로 이동하고,
`views/mod.rs`에서 `pub mod settings;`로 교체.

---

## 10. CSS 추가사항

### 10.1 Toggle Switch (div 기반, ::after 불가)

CSS ::after 의사요소는 Dioxus RSX에서 이벤트 핸들링이 불가하므로
실제 DOM 요소로 구현:

```html
<!-- RSX 구조 -->
<div class="settings-toggle active" onclick={...}>
    <div class="settings-toggle-thumb" />
</div>
```

```css
.settings-toggle {
    position: relative;
    width: 44px;
    height: 24px;
    background: var(--bg-3);
    border-radius: 12px;
    cursor: pointer;
    transition: background 0.2s;
    flex-shrink: 0;
}

.settings-toggle.active {
    background: var(--accent);
}

.settings-toggle.dangerous.active {
    background: var(--danger);
}

.settings-toggle-thumb {
    position: absolute;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: white;
    top: 3px;
    left: 3px;
    transition: transform 0.2s;
}

.settings-toggle.active .settings-toggle-thumb {
    transform: translateX(20px);
}
```

### 10.2 Slider

```css
.settings-slider {
    -webkit-appearance: none;
    width: 100%;
    height: 6px;
    background: var(--bg-3);
    border-radius: 3px;
    outline: none;
}

.settings-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--accent);
    cursor: pointer;
}
```

### 10.3 Tag Chip

```css
.tag-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    font-size: 12px;
    font-family: var(--font-mono);
    color: var(--text-1);
}

.tag-chip-remove {
    background: none;
    border: none;
    color: var(--text-3);
    cursor: pointer;
    padding: 0 2px;
    line-height: 1;
    font-size: 14px;
}
```

### 10.4 설정 필드 행

```css
.settings-field {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 0;
    border-bottom: 1px solid var(--border);
    gap: 16px;
}

.settings-field:last-child {
    border-bottom: none;
}

.settings-field-label {
    flex: 0 0 200px;
    min-width: 0;
}

.settings-field-label .label {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-0);
}

.settings-field-label .description {
    font-size: 11px;
    color: var(--text-3);
    margin-top: 2px;
}

.settings-field-control {
    flex: 1;
    max-width: 400px;
    display: flex;
    align-items: center;
    gap: 8px;
}
```

### 10.5 MCP Server Card

```css
.mcp-server-card {
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 16px;
    margin-bottom: 12px;
}

.mcp-server-card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
}

.mcp-server-card-name {
    font-weight: 600;
    font-size: 14px;
    color: var(--text-0);
    font-family: var(--font-mono);
}
```

### 10.6 전역 Save 바 + Restart 배너

```css
.settings-save-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 16px;
    background: var(--bg-1);
    border-bottom: 1px solid var(--border);
}

.settings-save-bar.dirty .btn-save {
    background: var(--accent);
    color: white;
}

.restart-banner {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    background: color-mix(in srgb, var(--warning) 10%, transparent);
    border-bottom: 1px solid var(--warning);
    font-size: 13px;
    color: var(--warning);
}
```

### 10.7 탭 dirty 배지

```css
.tab-dirty::after {
    content: '';
    width: 6px;
    height: 6px;
    background: var(--accent);
    border-radius: 50%;
    display: inline-block;
    margin-left: 4px;
    vertical-align: middle;
}
```

### 10.8 모바일 대응

탭 바는 수평 스크롤:

```css
.settings-tabs {
    display: flex;
    overflow-x: auto;
    white-space: nowrap;
    gap: 4px;
    padding: 0 16px;
    border-bottom: 1px solid var(--border);
    -webkit-overflow-scrolling: touch;
}

@media (max-width: 768px) {
    .settings-field {
        flex-direction: column;
        align-items: stretch;
    }
    .settings-field-label {
        flex: 0 0 auto;
    }
    .settings-field-control {
        max-width: 100%;
    }
}
```

---

## 11. Validation 전략

### 프론트엔드 (즉각적)

- **Number**: min/max 범위 확인, 입력 시 실시간
- **Required**: 빈 문자열 금지 (workspace path 등)
- **Cross-field**: `exec.default_timeout_secs <= exec.max_timeout_secs`
- **Format**: port 범위 (1–65535), IP 형식
- **크로스 필드 검증은 Save 시 일괄 수행**

### 백엔드 (최종 검증)

- `OxiosConfig::validate()` 기존 로직 그대로 사용
- 에러 응답을 필드별로 파싱하여 해당 탭에 표시

### 표시 방식

```
 ┌─────────────────────────────────────┐
 │ Port:  [99999]                      │
 │        ↑ Must be 1–65535            │
 └─────────────────────────────────────┘
```

백엔드 에러 응답 형식 (`"Invalid config: ..."` 문자열)을 파싱하여
가능하면 관련 필드에 매핑. 파싱 실패 시 Save 버튼 아래에 전체 에러 표시.

---

## 12. 안전 (Safety) 기능

### 12.1 위험 설정 경고

위험한 설정은 빨간색 배경 + 경고 아이콘:

```
 ┌─ ⚠️ DANGEROUS ────────────────────────────┐
 │ Shell Mode  [●───────] OFF                 │
 │                                             │
 │ "Enabling shell mode allows agents to       │
 │  execute arbitrary bash commands on the     │
 │  host. This is a major security risk."      │
 └─────────────────────────────────────────────┘
```

대상 필드:
- `exec.allow_shell_mode` — "에이전트가 임의 bash 명령 실행 가능"
- `security.network_access` — "에이전트가 외부 네트워크에 접근 가능"
- `security.can_fork` — "에이전트가 하위 에이전트를 무제한 생성 가능"

### 12.2 Restart 배너

Save 성공 후 상단에 지속 배너 표시:

```
 ℹ️ Configuration saved. Restart Oxios to apply changes.
```

이 배너는 다음 Save까지 유지되며, 새로고침하면 사라짐.

Phase 1에서는 모든 설정 변경 후 표시. Phase 2에서는 필드별로 hot-reload
가능 여부를 분석하여 선택적 표시.

### 12.3 Reset to Defaults

각 섹션 카드 우상단에 "↺ Reset" 버튼.
프론트엔드에 하드코딩된 기본값(`ConfigSnapshot::default()`)으로 해당 섹션만 초기화.
원본과 비교하여 dirty 표시.

---

## 13. 구현 Phase

### Phase 1: 기본 구조 + 핵심 위젯

1. `config_types.rs` — `ConfigSnapshot` 미러 타입 + Default 구현
2. `components/settings/` — 핵심 위젯 라이브러리
   - Toggle, Slider, NumberInput, TagInput, SectionCard, PasswordInput
3. `views/settings/mod.rs` — SettingsView (탭 컨테이너 + 전역 Save + dirty 배지)
4. General 탭 구현
5. Engine 탭 구현 (PasswordInput + api_key_set 처리)
6. Exec & Security 탭 구현 (위험 설정 경고 포함)
7. 기존 `config.rs` → `advanced_tab.rs` 이동
8. 백엔드: `handle_config_get`에 `api_key_set` 필드 추가
9. CSS 스타일 추가
10. `views/mod.rs` 라우팅 업데이트

### Phase 2: 전체 탭 + 고급 기능

11. Agents 탭 (Scheduler + Orchestrator + Persona + Budget)
12. Memory & Context 탭
13. Integrations 탭 (Channels + MCP Server Cards + Cron + Browser)
14. Monitoring 탭 (OTel + Audit + ResourceMonitor)
15. SelectGroup, MultiCheckbox, KeyValueInput 위젯
16. MCP Server Card 편집 컴포넌트
17. Reset to Defaults 기능
18. 백엔드 validation 에러 → 필드 매핑

### Phase 3: 백엔드 개선 (선택)

19. `GET /api/config/schema` — 필드별 타입, 범위, 기본값, 설명 반환
20. `PATCH /api/config/:section` — 섹션별 부분 업데이트
21. 런타임 핫리로드 지원 (커널 리팩토링, 별도 RFC)

---

## 14. 기술 고려사항

### Dioxus WASM

- `<input type="range">`는 WASM에서 정상 동작
- CSS custom properties (`var(--accent)`) 지원
- `wasm_bindgen`으로 clipboard 접근 가능 (기존 코드에 이미 구현)
- CSS `::after` 의사요소는 이벤트 핸들링 불가 → 실제 DOM 요소 사용
- 탭 수평 스크롤: `-webkit-overflow-scrolling: touch` + flex overflow-x

### 프론트엔드 타입 안전성

- `ConfigSnapshot` 미러 타입은 `oxios-kernel` config.rs와 수동 동기화 필요
- 백엔드 스키마 변경 시 프론트엔트도 업데이트해야 함
- Phase 3의 `/api/config/schema`로 자동화 가능

### Signal 기반 상태

- `use_resource`로 초기 로드
- `Signal<ConfigSnapshot>`로 편집 상태 추적
- Save 시 Signal 값을 JSON으로 직렬화하여 PUT
- Dirty 판별: `original` Signal과 `editing` Signal의 필드 비교

### 성능

- 전체 설정은 보통 < 5KB — 매번 전체 PUT해도 부하 없음
- 탭 전환 시 네트워크 요청 없음 (초기 1회 로드 후 메모리)
- Debounce는 불필요 (Save 버튼 클릭 시에만 전송)
- MCP 서버가 많아도 수십 개 수준 — 카드 렌더링 부하 없음

---

## 15. 요약

| 항목 | 기존 | 제안 |
|------|------|------|
| 편집 방식 | JSON textarea | 8개 탭 카테고리 폼 위젯 |
| 검증 | 저장 후 백엔드 에러 | 필드 단위 즉각 검증 + 백엔드 최종 검증 |
| 위험 설정 | 표시 없음 | 빨간색 경고 배지 + 설명 |
| Hot-reload | 착각 (실제는 restart 필요) | "Restart to apply" 솔직 표시 |
| 기본값 복원 | 수동 | 섹션별 Reset 버튼 |
| API Key | JSON에 노출/누락 | 마스킹 + `api_key_set` 상태 표시 |
| API | GET/PUT 전체 | Phase 1 동일, Phase 2에서 PATCH |
| MCP 편집 | JSON 직접 편집 | 전용 카드 UI |
| 고급 편집 | 기본 화면 | Advanced 탭으로 유지 |
| 커버리지 | N/A (raw JSON) | OxiosConfig 전체 필드 커버 |
| 탭 수 | N/A | 8개 (관련 설정 자연스럽게 그룹화) |
| Save | 전체 PUT | 전역 Save 버튼 1개 + 탭 dirty 배지 |
