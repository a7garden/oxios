# Loop 11: Cron Scheduler — 시간 기반 자율 에이전트 실행

> **목표:** 사용자 개입 없이 정해진 시간에 에이전트가 자동 실행되는 cron 시스템
> **의존:** `cron` crate (표현식 파서), 기존 `AgentScheduler` (실행 큐)

---

## 1. 문제 정의

현재 Oxios는 **사용자가 메시지를 보낼 때만** 에이전트가 실행됨:

```
사용자 → POST /api/chat → Orchestrator → Ouroboros → 결과
```

필요한 것:

```
cron schedule → CronScheduler → Orchestrator → Ouroboros → 결과 → 대시보드/알림
```

사용 예:
- "매일 오전 9시에 뉴스 요약"
- "매시간 백업"
- "매주 월요일 코드 리뷰"
- "15분마다 모니터링 체크"

---

## 2. 아키텍처

```
┌─────────────────────────────────────────────────────┐
│                    Kernel                            │
│                                                      │
│  ┌──────────────┐     ┌─────────────────────────┐   │
│  │ CronScheduler│────→│   AgentScheduler         │   │
│  │              │     │   (기존, 변경 없음)       │   │
│  │ tick(60s)    │     └─────────────────────────┘   │
│  │ ┌──────────┐ │                │                   │
│  │ │ CronJob  │ │                ▼                   │
│  │ │ ┌──────┐ │ │     ┌─────────────────────┐       │
│  │ │ │ cron │ │ │     │  Orchestrator        │       │
│  │ │ │ expr │ │ │     │  handle_message()    │       │
│  │ │ └──────┘ │ │     └─────────────────────┘       │
│  │ │ template │ │                                     │
│  │ └──────────┘ │     ┌─────────────────────┐       │
│  │              │────→│  StateStore          │       │
│  └──────────────┘     │  (jobs.json)         │       │
│       ▲               └─────────────────────┘       │
│       │                                             │
│  ┌────┴─────────┐                                   │
│  │ Config TOML  │                                   │
│  └──────────────┘                                   │
└─────────────────────────────────────────────────────┘
```

핵심: **CronScheduler는 AgentScheduler 위에 얹는 레이어.** 기존 스케줄러는 건드리지 않음.

---

## 3. 데이터 구조

### CronJob

```rust
/// 하나의 cron 작업 정의.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    /// 작업 ID.
    pub id: Uuid,
    /// 사람이 읽을 수 있는 이름.
    pub name: String,
    /// cron 표현식 (예: "0 9 * * *").
    pub schedule: String,
    /// 에이전트가 실행할 목표 설명.
    pub goal: String,
    /// 실행 제약 (선택).
    #[serde(default)]
    pub constraints: Vec<String>,
    /// 완료 조건 (선택).
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    /// 사용할 툴체인 (기본: "default").
    #[serde(default = "default_toolchain")]
    pub toolchain: String,
    /// 실행 우선순위.
    #[serde(default)]
    pub priority: Priority,
    /// 활성화 여부.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 마지막 실행 시각.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run: Option<DateTime<Utc>>,
    /// 다음 예정 실행 시각.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run: Option<DateTime<Utc>>,
    /// 총 실행 횟수.
    #[serde(default)]
    pub run_count: u64,
    /// 마지막 실행 결과 요약.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_result: Option<String>,
    /// 마지막 실행 성공 여부.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_success: Option<bool>,
}
```

### CronJobResult

```rust
/// cron 실행 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobResult {
    pub job_id: Uuid,
    pub job_name: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub success: bool,
    pub summary: String,
}
```

---

## 4. CronScheduler

```rust
pub struct CronScheduler {
    /// 등록된 cron jobs.
    jobs: Arc<RwLock<HashMap<Uuid, CronJob>>>,
    /// cron 표현식 파서 (캐시).
    schedules: Arc<Mutex<HashMap<Uuid, Schedule>>>,
    /// 기존 에이전트 스케줄러에 작업 제출용.
    agent_scheduler: Arc<AgentScheduler>,
    /// 상태 저장소.
    state_store: Arc<StateStore>,
    /// 실행 취소 토큰.
    cancel: Arc<AtomicBool>,
}
```

### 핵심 메서드

```rust
impl CronScheduler {
    /// 새 CronScheduler 생성.
    pub fn new(
        agent_scheduler: Arc<AgentScheduler>,
        state_store: Arc<StateStore>,
    ) -> Self;

    /// cron 표현식으로 스케줄 파싱.
    fn parse_schedule(&self, expr: &str) -> Result<Schedule>;

    /// 다음 실행 시각 계산.
    fn next_fire_time(&self, schedule: &Schedule, after: &DateTime<Utc>) -> Option<DateTime<Utc>>;

    /// 메인 루프 시작 (60초 간격 tick).
    pub async fn start(&self);

    /// 매 tick마다: 만료된 job 찾기 → 실행 → 상태 업데이트.
    async fn tick(&self);

    /// 단일 job 실행: Orchestrator.handle_message() 호출.
    async fn execute_job(&self, job: &CronJob) -> Result<CronJobResult>;

    /// job 추가.
    pub async fn add_job(&self, job: CronJob) -> Result<Uuid>;

    /// job 제거.
    pub async fn remove_job(&self, id: Uuid) -> Result<()>;

    /// job 활성화/비활성화.
    pub async fn toggle_job(&self, id: Uuid, enabled: bool) -> Result<()>;

    /// 수동 즉시 실행 (schedule 무시).
    pub async fn trigger_job(&self, id: Uuid) -> Result<CronJobResult>;

    /// 모든 job 조회.
    pub async fn list_jobs(&self) -> Vec<CronJob>;

    /// 단일 job 조회.
    pub async fn get_job(&self, id: Uuid) -> Option<CronJob>;

    /// 상태를 StateStore에 영구 저장.
    async fn persist_jobs(&self);

    /// 시작 시 StateStore에서 job 복원.
    pub async fn restore_jobs(&self);

    /// 종료.
    pub fn stop(&self);
}
```

### tick() 로직

```
tick() (60초마다):
  now = Utc::now()
  for each job in jobs:
    if !job.enabled → skip
    if job.next_run ≤ now:
      spawn execute_job(job)
      job.last_run = now
      job.next_run = next_fire_time(schedule, now)
      job.run_count += 1
      persist_jobs()
    else:
      next_run 미리 계산 (최초 로드 시)
```

---

## 5. 설정 (config.toml)

```toml
[cron]
# cron 스케줄러 활성화
enabled = true
# tick 간격 (초)
tick_interval_secs = 60

# 작업 정의
[cron.jobs]
morning_report = { schedule = "0 9 * * *", goal = "Summarize latest tech news and create a brief report", priority = "low" }
hourly_backup = { schedule = "0 * * * *", goal = "Backup workspace state to ~/.oxios/backups", priority = "normal" }
weekly_review = { schedule = "0 10 * * 1", goal = "Review code changes from the past week", toolchain = "rust", priority = "normal" }
monitor = { schedule = "*/15 * * * *", goal = "Check system health and report anomalies", priority = "low" }
```

### Config 구조체

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CronConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_tick_interval")]
    pub tick_interval_secs: u64,
    /// 인라인 job 정의 (name → { schedule, goal, ... }).
    #[serde(default)]
    pub jobs: HashMap<String, InlineCronJob>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InlineCronJob {
    pub schedule: String,
    pub goal: String,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(default = "default_toolchain")]
    pub toolchain: String,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default = "default_true")]
    pub enabled: bool,
}
```

---

## 6. API 엔드포인트

| Method | Path | 설명 |
|--------|------|------|
| `GET` | `/api/cron-jobs` | 모든 cron job 목록 |
| `POST` | `/api/cron-jobs` | 새 job 생성 |
| `GET` | `/api/cron-jobs/:id` | 단일 job 조회 |
| `DELETE` | `/api/cron-jobs/:id` | job 삭제 |
| `PATCH` | `/api/cron-jobs/:id` | job 수정 (enabled, schedule, goal 등) |
| `POST` | `/api/cron-jobs/:id/trigger` | 수동 즉시 실행 |

### POST /api/cron-jobs 요청 예

```json
{
  "name": "morning_report",
  "schedule": "0 9 * * *",
  "goal": "Summarize latest tech news",
  "priority": "low",
  "toolchain": "default"
}
```

### GET /api/cron-jobs 응답 예

```json
{
  "jobs": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "morning_report",
      "schedule": "0 9 * * *",
      "goal": "Summarize latest tech news",
      "enabled": true,
      "last_run": "2026-05-06T09:00:12Z",
      "next_run": "2026-05-07T09:00:00Z",
      "run_count": 42,
      "last_success": true,
      "last_result": "Generated 5-item tech news summary"
    }
  ]
}
```

---

## 7. 의존성

```toml
# Cargo.toml (oxios-kernel)
[dependencies]
cron = "0.16"  # cron 표현식 파서 (타이머 없음, 파싱만)
chrono = { version = "0.4", features = ["serde"] }
```

`cron` crate은 표현식 파싱 + 다음 실행 시각 계산만 함. 타이머는 `tokio::time::interval` 직접 사용.

---

## 8. 통합 지점

### kernel.rs (초기화)

```rust
// 기존 코드 뒤에 추가
let cron_scheduler = Arc::new(CronScheduler::new(
    agent_scheduler.clone(),
    state_store.clone(),
));
// config에서 job 로드
cron_scheduler.restore_jobs().await?;
// config의 인라인 job도 로드
for (name, inline) in &config.cron.jobs {
    let job = CronJob::from_inline(name, inline);
    cron_scheduler.add_job(job).await?;
}
// 백그라운드에서 시작
if config.cron.enabled {
    let cron_clone = cron_scheduler.clone();
    tokio::spawn(async move { cron_clone.start().await });
}
```

### Orchestrator 연결

CronScheduler는 `execute_job`에서 Orchestrator를 직접 호출:

```rust
async fn execute_job(&self, job: &CronJob) -> Result<CronJobResult> {
    let result = self.orchestrator
        .handle_message("cron", &job.goal, None)
        .await?;

    // job 상태 업데이트
    let mut jobs = self.jobs.write().await;
    if let Some(j) = jobs.get_mut(&job.id) {
        j.last_run = Some(Utc::now());
        j.last_result = Some(result.output.clone());
        j.last_success = Some(result.success);
        j.run_count += 1;
    }
    drop(jobs);
    self.persist_jobs().await;

    Ok(CronJobResult { ... })
}
```

Orchestrator를 Arc로 공유해야 함 → 기존 구조에서 `orchestrator`를 `Arc<Orchestrator>`로 래핑.

---

## 9. 파일 구조

```
crates/oxios-kernel/src/
├── cron.rs              # CronScheduler, CronJob, CronJobResult (신규)
├── scheduler.rs         # 기존 AgentScheduler (변경 없음)
├── config.rs            # CronConfig, InlineCronJob 추가
└── lib.rs               # pub mod cron; 추가

channels/oxios-web/src/routes/
├── cron_jobs.rs         # API 핸들러 (신규)
└── mod.rs               # 라우트 등록
```

---

## 10. 테스트 계획

| 테스트 | 대상 |
|--------|------|
| `test_parse_cron_expression` | "0 9 * * *" → Schedule 파싱 |
| `test_parse_invalid_expression` | 잘못된 표현식 → 에러 |
| `test_next_fire_time_daily` | 매일 9시 → 다음 실행 시각 계산 |
| `test_next_fire_time_every_15min` | "*/15 * * * *" → 15분 간격 |
| `test_add_remove_job` | job 추가/제거 |
| `test_toggle_job` | enabled/disabled 토글 |
| `test_disabled_job_skipped` | 비활성 job은 tick에서 스킵 |
| `test_trigger_job_manual` | 수동 즉시 실행 |
| `test_persist_and_restore` | StateStore 저장/복원 |
| `test_run_count_increments` | 실행 후 run_count 증가 |
| `test_cron_job_from_config` | TOML 인라인 job → CronJob 변환 |

---

## 11. 크기 추정

| 항목 | 라인 수 |
|------|---------|
| `cron.rs` | ~350 |
| `cron_jobs.rs` (API) | ~120 |
| config 확장 | ~50 |
| 테스트 | ~200 |
| **총계** | **~720** |

소요: 1일
