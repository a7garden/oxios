# Web UI 설정 ↔ 백엔드 데몬 설정 연결 감사

**날짜:** 2026-06-03
**감사 범위:** `surface/oxios-web/web/src/routes/settings.tsx` ↔ `crates/oxios-kernel/src/config.rs` ↔ `surface/oxios-web/src/routes/system.rs::handle_config_put`
**결론:** **P0 안전 결함 1건 + P1 커버리지 갭 다수.** 실 데몬에서 라이브 검증 완료.

---

## 요약

| # | 발견 | 등급 | 상태 |
|---|------|------|------|
| F-1 | WebUI "Save"가 폼에 없는 섹션을 기본값으로 리셋 | **P0 안전** | ✅ 수정 (커밋 b451c2c) |
| F-2 | `exec.allowed_commands` / `exec.allowlist_mode`가 폼에 없음 | **P0 안전** | F-1에 의해 증폭 |
| F-3 | `security` 섹션 5개 필드 누락 (`allowed_tools`, `cors_origins`, …) | P1 | F-1에 의해 증폭 |
| F-4 | 14개 섹션 통째로 폼에 없음 (memory/audit/budget/otel/…) | P1 | F-1에 의해 증폭 |
| F-5 | `logging.level` 등 부분 필드 누락 | P2 | F-1에 의해 증폭 |
| F-6 | `mcp` / `cron` / `persona` 런타임 UI와 설정 UI 중복 | P2 | 설계 결정 필요 |
| F-7 | Web config 변경이 커널 런타임에 반영되지 않음 | **P0 아키텍처** | 라이브 검증 완료 |

---

## F-1. (P0) "Save"가 폼 외 섹션을 기본값으로 덮어씀

### 데이터 흐름
```
settings.tsx::handleSave (line 533)
  └─ fieldDefs의 9개 섹션만 모아서 PUT
       (kernel, exec, security, scheduler, orchestrator, context, gateway, session, logging)

system.rs::handle_config_put (line 773)
  └─ body 전체를 OxiosConfig으로 deserialize
       (#[serde(default)] 때문에 빠진 필드는 default 값으로 채움)
  └─ toml::to_string_pretty(&updated) → 디스크 덮어쓰기
  └─ *state.config.write() = updated
```

`fieldDefs`에 없는 15개 섹션(`engine`, `daemon`, `persona`, `memory`, `cron`, `mcp`, `git`, `audit`, `budget`, `resource_monitor`, `otel`, `channels`, `surfaces`, `browser`, `marketplace`)은 PUT 본문에서 누락되고, 백엔드는 그 누락을 "리셋"으로 해석한다. **merge/patch 로직은 어디에도 없다.** (`config.rs` 전체 grep: 0 hit)

### 라이브 검증 (실 데몬, `~/.oxios/config.toml`)

| 시점 | `exec.allowed_commands` | `engine.default_model` | `engine.api_key_set` | `channels.enabled` | `git` 섹션 |
|------|--------------------------|------------------------|----------------------|--------------------|-----------|
| Save **전** | 19개 (`ls, cat, git, …`) | `"minimax/MiniMax-M2.7"` | `true` | `["web"]` | 존재 |
| Save **후** | **`[]`** | **`""`** | **`false`** | **`[]`** | **삭제됨** |

복원: `cp /tmp/oxios-config-backup.toml ~/.oxios/config.toml`

### 영향
1. **즉각적 보안 회귀.** `exec.allowlist_mode = "enforced"`(기본값)에서 `allowed_commands = []`이 되면 **어떤 명령도 실행 불가**. 모든 에이전트 즉시 실패.
2. **자기기 죽이기.** `channels.enabled = []`이 되면 현재 사용 중인 web 채널까지 비활성화됨.
3. **모델 인증 정보 손실.** API key는 별도 저장소(`~/.oxi/auth.json`)에서 폴백되지만 `default_model`은 빈 문자열이 되어 다음 요청부터 400 에러.

### 권장 수정
**두 가지 옵션** (둘 다 필요):

1. **프론트 측 수정 — `settings.tsx::handleSave`**
   - GET으로 받은 현재 설정을 spread한 뒤 변경된 섹션만 덮어쓰기
   - 예: `const updated = { ...currentConfig, kernel: { ...currentConfig.kernel, ...formValues.kernel } }`

2. **백엔드 측 방어 — `system.rs::handle_config_put`**
   - body를 deserialize하기 전에 현재 in-memory config를 베이스로 merge
   - 또는 PATCH 시맨틱: `serde_json::Value`를 받아 deep merge 후 `OxiosConfig`으로 검증
   - 또는 필드별 sub-resource API로 분리 (`PUT /api/config/security` 등)

   **백엔드 방어가 더 안전** — 어떤 클라이언트가 호출하든 동일하게 보호됨.

---

## F-2. (P0) `exec.allowed_commands` / `exec.allowlist_mode` 폼 부재

`exec` 섹션의 6개 필드 중 4개만 노출:
- ✅ `default_mode`, `allow_shell_mode`, `default_timeout_secs`, `max_timeout_secs`
- ❌ `allowed_commands` ⚠️ **명시적 보안 설정**
- ❌ `allowlist_mode` ⚠️ **Enforced ↔ Permissive 토글**

이 필드들은 `default-config.toml`에서 가장 상세하게 다뤄지는 보안 설정이다:
```toml
allowed_commands = [
    "ls", "cat", "head", "tail", "wc",
    "grep", "rg", "find", "fd",
    "git", "cargo", "rustc",
    "python3", "node", "bun",
    "curl", "wget", "jq", "yq",
    "echo", "mkdir", "cp", "mv",
]
allowlist_mode = "enforced"
```

**이 필드가 폼에 없으므로 사용자는:**
- (a) 직접 `~/.oxios/config.toml`을 편집하거나
- (b) `oxios run` 명령형 흐름에서 `exec` 도구를 호출할 때 실패 → doctor 액션 아이템으로 안내

웹UI만으로는 안전하게 명령 실행 allowlist를 관리할 방법이 없다.

### 권장 추가 (F-1과 함께)
- 폼에 추가 + 별도 카드 `[exec] Allowlist` (Multi-select 또는 줄바꿈 textarea)
- "Reset to safe defaults" 버튼 (default-config.toml 값으로 복원)

---

## F-3. (P1) `security` 섹션 5개 필드 누락

`SecurityConfig`의 10개 필드 중 5개만 노출:
- ✅ `auth_enabled`, `network_access`, `can_fork`, `max_execution_time_secs`, `max_memory_mb`
- ❌ `allowed_tools` — 에이전트별 도구 화이트리스트 (기본: `read, write, edit, bash, grep, find, exec`)
- ❌ `max_audit_entries` — 감사 로그 보존
- ❌ `cors_origins` — 기본 `["http://localhost:4200"]` (프로덕션에서 도메인 추가 필요)
- ❌ `audit_log_path` — 감사 로그 파일 경로
- ❌ `rate_limit_per_minute` — API rate limit (기본 120)

`cors_origins`와 `auth_enabled`는 함께 동작 — 운영 환경에서 둘 다 켜야 함. 폼 분리로 사용자 혼란.

---

## F-4. (P1) 14개 섹션 통째로 폼 부재

| 섹션 | 핵심 필드 | 사용자 영향 |
|------|-----------|-------------|
| `engine` | (전용 EnginePanel + RoutingSection에서 대부분 처리) | OK |
| `daemon` | `pid_file`, `log_dir` | 로그 위치 변경 불가 |
| `persona` | `default_persona_id`, `max_concurrent_personas` | 기본 페르소나 변경 불가 |
| `memory` | `enabled`, `sqlite.path`, `embedding.provider`, `learning.*`, `consolidation.preset`, `bridge.*` | **메모리 시스템 설정 변경 불가** (예: MLX ↔ TF-IDF 전환, DB 경로 변경, dream preset) |
| `cron` | `enabled`, `tick_interval_secs`, inline jobs | cron 활성화/비활성화는 `/cron-jobs` UI에서 인라인 잡 편집 가능. 글로벌 토글은 부재 |
| `mcp` | 서버 정의 | `/mcp` UI는 토글/등록은 가능. config 정의 추가 시 직접 TOML 편집 |
| `git` | `auto_commit` | 자동 커밋 토글 불가 |
| `audit` | `enabled`, `max_entries` | 감사 끄기/크기 조정 불가 |
| `budget` | `enabled`, `default_token_budget`, `default_calls_budget`, `default_window_secs` | **비용 상한 설정 불가** — 별도 `/budget` 페이지는 런타임 한도 관리만 |
| `resource_monitor` | `cpu_threshold`, `memory_threshold`, `load_threshold`, … | 리소스 임계값 조정 불가 |
| `otel` | `enabled`, `endpoint`, `service_name`, `sampling_ratio` | OpenTelemetry 트레이싱 토글 불가 |
| `channels` | `enabled` (list), `telegram.bot_token_env`, `telegram.allowed_users`, `telegram.session.*` | **Telegram 채널 on/off 및 허용 사용자 관리 불가** |
| `surfaces` | `enabled` (list) | web surface 토글 불가 |
| `browser` | `enabled`, `engine.*` | 브라우저 통합 토글 불가 |
| `marketplace` | `base_url`, `enabled`, `skills_sh.*` | ClawHub / Skills.sh 마켓플레이스 토글 불가 |

### 우선순위
1. **`memory.consolidation.preset`** (conservative/balanced/aggressive) — RFC-008 핵심 설정, 자주 변경
2. **`memory.embedding.provider`** (gguf ↔ tfidf) — Apple Silicon에서만 의미 있는 옵션이지만 명확한 토글 필요
3. **`channels.enabled` / `telegram.allowed_users`** — 채널 운영의 기본
4. **`audit.enabled`** — 운영 디버깅에 필수
5. **`otel.enabled`** — 프로덕션 관측성

---

## F-5. (P2) 부분 필드 누락

| 섹션 | 누락 필드 | 기본값 |
|------|-----------|--------|
| `orchestrator` | `eval_cache_enabled` | `true` |
| `logging` | `level` | `None` (RUST_LOG 폴백) |

둘 다 폼에서 누락 시 F-1 경로로 기본값 리셋 위험.

---

## F-6. (P2) 런타임 UI와 설정 UI 중복

- `mcp.tsx` — MCP 서버 등록/토글 (config 기반)
- `cron-jobs.tsx` — cron 잡 CRUD
- `budget.tsx` — 런타임 예산 한도

이들은 `config`의 일부를 동적으로 편집한다. 폼의 정적 config 편집과 어떻게 통합할지 명시 필요:
- **옵션 A:** 폼이 모든 config를 편집, 런타임 UI는 액션 전용
- **옵션 B:** 런타임 UI가 config를 직접 편집 (현재 추정), 폼은 보충

코드 추적 결과 런타임 UI는 `mcp.servers`, `cron.jobs` 등을 직접 mutate하는 별도 API 사용. 폼은 정적 config만 다룸. **현재는 양립하나 사용자 멘탈 모델 혼란 가능.**

---

## F-7. (P0) Web config 변경이 커널 런타임에 반영되지 않음

### 현상
`handle_config_put`은 `state.config` (web layer)를 갱신하고 디스크에 기록하지만, **커널 서브시스템은 시작 시 복제한 config 스냅샷을 계속 사용**한다.

### 원인
```text
src/surface.rs:61
  config: Arc::new(RwLock::new(config.clone()))  ← web 전용 RwLock

crates/oxios-kernel/src/kernel_handle/engine_api.rs:362
  config: Arc<RwLock<OxiosConfig>>              ← EngineApi 전용 RwLock

crates/oxios-kernel/src/kernel_handle/exec_api.rs:11
  config: Arc<ExecConfig>                        ← ExecApi (RwLock 없음!)
```

각 API 퍼사드가 **시작 시 복제**한 독립 config 사본을 보유. Web의 PUT 변경은 web layer만 갱신.

### 라이브 검증 (실 데몬)
```
1. PUT /api/config {"gateway": {"port": 8000}}
2. GET /api/config → port: 8000 ✓ (web layer 갱신 확인)
3. 디스크 config.toml → port = 8000 ✓
4. curl http://127.0.0.1:8000/ → Connection refused ✗
5. curl http://127.0.0.1:4200/ → 200 OK (구 포트로 계속 리스닝)
```

### 커널 핫리로드 지원 현황
| 서브시스템 | 핫리로드 | 경로 |
|-----------|---------|------|
| EngineApi | ✅ | `/api/engine/*` → `rebuild_and_swap()` |
| ExecApi | ❌ | `Arc<ExecConfig>` (불변) |
| SecurityApi | ❌ | AuthManager만 내부 상태 관리 |
| InfraApi | ❌ | Scheduler/ResourceMonitor 고정 |
| PersonaApi | ❌ | PersonaManager 내부 상태 |
| SessionConfig | ❌ | 이벤트 라우트에서 config.read() 사용 |

### 영향
- WebUI에서 `exec.allowed_commands`, `security.auth_enabled`, `scheduler.max_concurrent` 등을 변경해도 **데몬 재시작 전까지 아무 효과 없음**
- 사용자에게 "Settings saved successfully" 메시지가 표시되지만 실제로는 효과가 없어 혼란 유발

### 권장 수정
1. **단기 (방어):** 핫리로드 미지원 섹션 변경 시 UI에 "재시작 필요" 경고 표시
2. **중기 (아키텍처):** `AppState.config`를 커널과 공유 (단일 `Arc<RwLock<OxiosConfig>>` 참조)
3. **장기 (서브시스템):** ExecApi, Scheduler 등이 `RwLock<ExecConfig>`를 읽도록 전환

---

## 권장 작업 순서

1. **완료 (F-1 백엔드 방어):** `handle_config_put`에서 PATCH 시맨틱 적용 (커밋 `b451c2c`)
2. **즉시 (F-7):** 핫리로드 미지원 섹션에 "재시작 필요" UI 경고 추가
3. **단기 (F-2, F-3):** exec.allowlist, security 나머지 5필드, logging.level, orchestrator.eval_cache_enabled를 폼에 추가.
4. **중기 (F-4):** memory/channels/audit/otel/daemon 섹션을 별도 settings 페이지 또는 "Advanced" 그룹으로 추가.
5. **중기 (F-7 아키텍처):** `AppState.config`를 커널과 공유 (단일 `Arc<RwLock<OxiosConfig>>` 참조)
6. **장기 (F-6):** 런타임 UI와 폼의 책임 경계 문서화.

---

## 부록: 비교 표

### 백엔드 OxiosConfig 섹션 vs WebUI 노출

| 섹션 | 백엔드 필드 수 | WebUI 필드 | 상태 |
|------|---------------|------------|------|
| `kernel` | 3 | 3 | ✅ 완전 |
| `engine` | 7 | 4 (전용패널) + 3 (Routing) + 1 (ApiKey) + 1 (ProviderOptions) | ✅ 완전 (전용패널) |
| `daemon` | 2 | 0 | ❌ |
| `gateway` | 2 | 2 | ✅ 완전 |
| `scheduler` | 3 | 3 | ✅ 완전 |
| `orchestrator` | 3 | 2 | ⚠️ 1 누락 |
| `context` | 2 | 2 | ✅ 완전 |
| `security` | 10 | 5 | ⚠️ 5 누락 |
| `persona` | 2 | 0 | ❌ |
| `memory` | 21+ | 0 | ❌ |
| `cron` | 3+ | 0 | ❌ (런타임 별도) |
| `mcp` | 동적 | 0 | ❌ (런타임 별도) |
| `git` | 1 | 0 | ❌ |
| `audit` | 2 | 0 | ❌ |
| `budget` | 4 | 0 | ❌ (런타임 별도) |
| `exec` | 6 | 4 | ⚠️ 2 누락 (보안 중요) |
| `resource_monitor` | 5 | 0 | ❌ |
| `otel` | 4 | 0 | ❌ |
| `logging` | 2 | 1 | ⚠️ 1 누락 |
| `channels` | 동적 | 0 | ❌ |
| `surfaces` | 1 | 0 | ❌ |
| `browser` | 2+ | 0 | ❌ |
| `session` | 3 | 3 | ✅ 완전 |
| `marketplace` | 4 | 0 | ❌ |

**통계:** 25개 섹션 중 완전 노출 7개, 부분 노출 5개 (모두 안전 필드 누락), 미노출 13개.
