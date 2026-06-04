# RFC-T1-D: Settings UI Completion (F-1 ~ F-7)

> **날짜:** 2026-06-03
> **Tier:** 1 (안정성 + 사용성, **가장 먼저 부딪히는 문제**)
> **영역:** `surface/oxios-web/web/src/routes/settings.tsx` + 백엔드 `/api/config`
> **기반:** [production-audit/2026-06-03-webui-config-coverage.md](../../production-audit/2026-06-03-webui-config-coverage.md)
> **연계:** RFC-018 (config UX), production-audit F-1~F-7

---

## 1. 동기

[production-audit/2026-06-03-webui-config-coverage.md](../../production-audit/2026-06-03-webui-config-coverage.md)가 이미 **P0 결함 1건 + P1 갭 다수**를 식별. F-1 (폼 외 섹션 리셋)은 이미 백엔드에서 부분 수정됨. **F-2~F-7은 그대로.**

현재 사용자 시나리오:
1. Settings에서 "Save" 클릭
2. 성공 메시지 표시 → **실제로는 효과 없음** (F-7: 핫리로드 미지원)
3. 폼에 없는 13개 섹션은 **웹으로 관리 불가** → `~/.oxios/config.toml` 직접 편집
4. 보안 핵심 설정(`exec.allowed_commands`, `security.allowed_tools`) **편집 불가**

**목표:** 모든 config를 웹에서 안전하게 관리. 핫리로드 미지원 필드 명시. 변경 diff 미리보기.

---

## 2. 디자인

### 와이어프레임 — Settings 재구조화

```
┌─────────────────────────────────────────────────────────────────┐
│ Settings                                                         │
│                                                                  │
│  ┌────────────────┐  ┌─────────────────────────────────────────┐│
│  │ Sidebar (groups)│  │ Group: Memory                          ││
│  │                │  │ ────────────────                        ││
│  │ AI             │  │                                         ││
│  │ • Engine       │  │ Embedding provider [gguf ▾]            ││
│  │ • Routing      │  │ Database path   [/home/.oxios/mem.db]   ││
│  │                │  │                                         ││
│  │ System         │  │ Consolidation                           ││
│  │ • Kernel    ✓  │  │  Preset [balanced ▾]                    ││
│  │ • Daemon    ⚠  │  │  ☑ Auto-dream nightly                   ││
│  │ • Logging       │  │                                         ││
│  │ • Scheduler  ✓  │  │ Learning                                ││
│  │ • Resources  ⚠  │  │  SONA enabled      [✓]                  ││
│  │                │  │  Proactive recall  [✓]                  ││
│  │ Security       │  │                                         ││
│  │ • Auth      ⚠  │  │ ⚠ Some changes require daemon restart   ││
│  │ • Allowlist ⚠  │  │ [Reset to defaults]                     ││
│  │ • Audit     ⚠  │  │                                         ││
│  │ • Telemetry ⚠  │  └─────────────────────────────────────────┘│
│  │                │                                             │
│  │ Memory         │  상단 sticky: [💾 Save] [↶ Undo] [⏪ Reset]│
│  │ • Storage    ⚠  │                                             │
│  │ • Embedding  ⚠  │                                             │
│  │ • Learning   ⚠  │                                             │
│  │ • Dream      ⚠  │                                             │
│  │                │                                             │
│  │ Channels       │                                             │
│  │ • Web        ✓  │                                             │
│  │ • Telegram  ⚠  │                                             │
│  │                │                                             │
│  │ Advanced       │                                             │
│  │ • MCP        ⚠  │                                             │
│  │ • Browser    ⚠  │                                             │
│  │ • Marketplace ⚠  │                                             │
│  └────────────────┘                                             │
└─────────────────────────────────────────────────────────────────┘
```

**범례:** ✓ = 핫리로드 지원, ⚠ = 재시작 필요

### 좌측 사이드바 추가 (현재 1단 → 2단)

현재 `settings.tsx`는 단일 페이지 (SubNav 없음). `SettingsLayout`은 **라우트 분할용**이고 그룹핑은 아님. 좌측에 그룹 사이드바를 추가하여 **긴 폼을 논리적 묶음으로 분할**.

### 필드 추가 매트릭스 (F-2, F-3, F-4, F-5)

| 섹션 | 추가 필드 | 우선순위 |
|------|-----------|----------|
| `exec` | `allowed_commands` (Multi-textarea), `allowlist_mode` (toggle) | P0 |
| `security` | `allowed_tools`, `cors_origins`, `audit_log_path`, `rate_limit_per_minute`, `max_audit_entries` | P1 |
| `memory` | `enabled`, `sqlite.path`, `embedding.provider`, `learning.sona_enabled`, `consolidation.preset`, `bridge.*` | P1 |
| `channels.telegram` | `bot_token_env`, `allowed_users`, `session.*` | P1 |
| `audit` | `enabled`, `max_entries` | P1 |
| `otel` | `enabled`, `endpoint`, `service_name`, `sampling_ratio` | P2 |
| `daemon` | `pid_file`, `log_dir` | P2 |
| `persona` | `default_persona_id`, `max_concurrent_personas` | P2 |
| `cron` | `enabled` (글로벌 토글) | P2 |
| `resource_monitor` | `cpu_threshold`, `memory_threshold`, `load_threshold`, `*_action` | P2 |
| `logging` | `level` (debug/info/warn/error) | P2 |
| `orchestrator` | `eval_cache_enabled` | P2 |
| `marketplace` | `base_url`, `enabled`, `skills_sh.*` | P3 |

### 핫리로드 경고 배지 (F-7)

각 필드 옆에 메타데이터:
```ts
type FieldDef = {
  key: string
  hotReload: boolean      // false면 "재시작 필요" 빨간 점
  restartScope: 'kernel' | 'gateway' | 'logging' | 'memory'
  ...
}
```

배지 클릭 → tooltip:
> ⚠ Changes to this field require daemon restart to take effect.
> Kernel subsystems will use the new value, but Gateway (port), Logging, and Memory will use cached snapshots until restart.

### 변경 Diff 미리보기

Save 클릭 시 → 모달:
```
┌─────────────────────────────────────────────────────────┐
│  Confirm changes                                         │
│                                                          │
│  Section      Field           Old → New                  │
│  ─────────────────────────────────────────────          │
│  exec       allowed_commands  19 → 22                    │
│              + "fd" "rg" "jq"                             │
│  security   cors_origins      [a] → [a, b, c]            │
│  memory     consolidation     conservative → balanced    │
│             .preset                                       │
│                                                          │
│  ⚠ 1 change requires daemon restart:                    │
│    • memory.embedding.provider (gguf → tfidf)            │
│                                                          │
│  [Cancel]                            [Save & restart ▶]  │
└─────────────────────────────────────────────────────────┘
```

### Undo

좌측 사이드바에 "Last saved: 2m ago · [Undo]" 링크 → 백엔드는 history 보관 (선택). 아니면 클라이언트에서 `previousConfig` 스냅샷.

---

## 3. 구현 계획

### 파일 변경

| 파일 | 변경 |
|------|------|
| `routes/settings.tsx` | **대폭 재작성** — 그룹 사이드바 + 핫리로드 배지 + Diff 모달 |
| `components/layout/settings-layout.tsx` | 변경: 그룹 네비게이션 추가 |
| `components/settings/field-defs.ts` | **신규** — 모든 필드 정의 (핫리로드 메타 포함) |
| `components/settings/diff-preview.tsx` | **신규** — Save 직전 변경 사항 미리보기 |
| `components/settings/restart-badge.tsx` | **신규** |
| `components/settings/exec-allowlist-editor.tsx` | **신규** — Multi-line tag input |
| `components/settings/memory-section.tsx` | **신규** — Memory 그룹의 4개 서브섹션 |
| `components/settings/channels-section.tsx` | **신규** |
| `hooks/use-config.ts` | **신규** — GET /api/config + history |
| `lib/config-schema.ts` | **신규** — 런타임 스키마 (백엔드 응답을 zod로 검증, 필드 노출 여부) |
| `i18n/locales/{en,ko}.json` | 추가: 80+ 새 키 |

### 백엔드 변경

#### A. F-1 강화 — 진짜 PATCH 시맨틱

이미 `b451c2c`로 부분 수정됨. 더 강화하려면:
- `PUT /api/config` → 전체 replace (현재)
- `PATCH /api/config` → partial merge (신규)
- 또는 `PUT /api/config/section/:name` → 섹션별 업데이트

**권장:** `PATCH /api/config`를 추가하고 프론트는 PATCH만 사용.

#### B. `/api/config/schema` — 런타임 스키마 노출

```json
{
  "sections": {
    "memory": {
      "fields": {
        "enabled": { "type": "bool", "default": true, "hot_reload": true },
        "sqlite.path": { "type": "path", "default": "...", "hot_reload": false }
      }
    }
  }
}
```

프론트는 이 스키마로 동적 폼 생성 가능. 하지만 **수동 정의가 더 빠르고 안전** → 이번 RFC에서는 `field-defs.ts`로 수동 정의, 향후 자동화.

#### C. 핫리로드 결과 검증

PATCH 후 어떤 필드가 실제로 적용됐는지 응답:
```json
{
  "applied_immediately": ["exec.allowed_commands", "security.cors_origins"],
  "requires_restart": ["memory.embedding.provider", "otel.endpoint"]
}
```

### 단계별 작업

### Step 1: 백엔드 PATCH + schema 응답 (4시간)
- `surface/oxios-web/src/routes/system.rs::handle_config_patch` 신규
- `handle_config_get` 응답에 `hot_reload` 메타 포함
- 단위 테스트 (merge 로직)

### Step 2: 필드 정의 + 핫리로드 배지 (3시간)
- `field-defs.ts` — 모든 필드 정의
- `restart-badge.tsx`
- 기존 `fieldDefs` 코드 마이그레이션

### Step 3: Exec Allowlist Editor (2시간)
- Multi-line tag input (enter로 추가, X로 제거)
- "Reset to safe defaults" 버튼
- 각 명령어에 위험도 자동 평가 (TODO: 백엔드 정책 의존)

### Step 4: Memory 섹션 신규 (4시간)
- 4개 서브섹션 (Storage, Embedding, Learning, Dream)
- `consolidation.preset` select
- `embedding.provider` toggle (UI에서 gguf ↔ tfidf)

### Step 5: Channels / Audit / OTEL / Resources (6시간)
- 텔레그램 봇 토큰 (env var reference), 허용 유저 (Multi-textarea)
- Audit 토글 + max_entries
- OTEL endpoint, service_name, sampling_ratio (0.0~1.0 slider)
- Resource 임계치 + 액션 (log/shed/notify)

### Step 6: Diff Preview + Undo (3시간)
- `diff-preview.tsx` 모달
- 변경 사항 diff 계산 (단순 key-value 비교, deep)
- Undo: Save 직전 `previousConfig` 보관

### Step 7: 사이드바 그룹화 (2시간)
- 좌측 그룹 네비게이션 (현재 라우트 분할 X, 단일 페이지 내 앵커)

### Step 8: i18n (2시간)
- 80+ 키 영/한 번역
- `settings.*.description` 패턴

### Step 9: 테스트 + 다듬기 (3시간)
- 단위: `field-defs` 검증
- E2E: Settings 진입 → 그룹 이동 → 필드 변경 → Diff → Save → 적용 확인
- `cargo test --workspace` 깨지지 않도록

**총: ~29시간 (4일)**

---

## 4. 위험 / 주의

| 위험 | 대응 |
|------|------|
| 폼 필드 누락 시 다시 F-1 | 백엔드 PATCH로 강제, "Save = partial" 보장 |
| Restart 후에도 안 바뀌는 필드 | "Save & restart" 버튼 + 데몬 자동 재시작 옵션 (systemd user?) |
| 80+ i18n 키 부담 | AI-assisted 번역 (GPT-4로 1차 → 검수) |
| field-defs 동기화 누락 | `lib/config-schema.ts` zod 검증 + lint 규칙 (선택) |
| 폼이 너무 길어짐 | 그룹 사이드바 + "Save" sticky bar |
| Long page 스크롤 성능 | `react-hook-form` 도입 고려, 현재는 useState |

---

## 5. 의존성

```json
{
  "dependencies": {},
  "devDependencies": {}
}
```

신규 dep 없음. **순수 컴포넌트 + 기존 API 확장**.

---

## 6. 완료 기준

- [ ] F-1 완전 해결: PATCH 시맨틱, 폼 외 섹션 보존
- [ ] F-2: `exec.allowed_commands` / `allowlist_mode` 편집 가능
- [ ] F-3: `security` 5개 누락 필드 추가
- [ ] F-4: `memory` / `channels.telegram` / `audit` 추가
- [ ] F-5: `logging.level`, `orchestrator.eval_cache_enabled` 추가
- [ ] F-7: 핫리로드 미지원 필드에 "재시작 필요" 배지
- [ ] Diff Preview 모달
- [ ] Undo (이전 설정으로 되돌리기)
- [ ] 그룹 사이드바 네비게이션
- [ ] i18n (EN/KO 100%)
- [ ] E2E 테스트 1개
- [ ] `cargo test` 깨지지 않음

---

## 7. 이 RFC를 먼저 해야 하는 이유

다른 Tier 1 RFC(A2A, Memory Map, Dashboard)는 **시각적 임팩트** 중심이지만:
- 이 RFC는 **사용자가 부딪히는 첫 문제**
- F-7 (재시작 안 됨) + F-1 (데이터 손실)은 **신뢰 파괴**
- 다른 RFC를 시작하기 전에 **기반 안정화**가 먼저
- 작업량 4일, 다른 RFC(2~2.5일)보다 약간 더 김
