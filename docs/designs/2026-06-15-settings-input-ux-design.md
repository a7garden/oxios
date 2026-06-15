# Settings — Field Control UX 개선

> **날짜**: 2026-06-15
> **상태**: 설계 — 사용자 승인 대기
> **범위**: 백엔드 (`crates/oxios-kernel/`, `surface/oxios-web/src/routes/`) + 프론트엔드 (`surface/oxios-web/web/src/components/settings/*`, `routes/settings.tsx`)
> **선행 의존**: `2026-06-15-settings-redesign.md` (3-zone 레이아웃), `dependsOn` 의존성 시스템 (구현 완료)
> **사용자 결정 (2026-06-15)**: 툴 카탈로그 = **백엔드 API**, 슬라이더 범위 = **확장 15개**

## 1. 동기 (Motivation)

현재 `/settings` 화면의 필드 컨트롤은 두 가지 큰 UX 문제를 안고 있다:

### 1.1 자유 텍스트 입력이 도메인-특화 필드에 잘못 사용됨

| 필드 | 현재 | 문제 |
|------|------|------|
| `security.allowed_tools` | `csv` — `read, write, edit, bash`를 텍스트로 직접 입력 | 사용자가 정확한 툴명을 외워야 함. 오타 위험. 허용 가능 툴 파악 불가 |
| `security.cors_origins` | `csv` — URL을 쉼표 구분 텍스트로 입력 | URL 형식 검증 없음. 오타로 CORS 디버깅 시간 낭비 |

### 1.2 단순 숫자 입력이 슬라이더로 더 자연스러운 필드에 사용됨

예: `max_evolution_iterations`, `min_evaluation_score`, `default_timeout_secs`, `dream_interval_hours` 등 15개

기존 사용자 인용:
> "최대 진화 반복, 평가 실패 후 에이전트가 재시도할 수 있는 횟수 이런 것같은 몇몇 항목들은 슬라이더 같은걸로 편집이 가능한 경우가 있지. 그런 건 슬라이더로 편집 가능하게 해줘."
> "허용 도구 ... 이런것도 직접 쓰게 할게 아니라 전용 gui가 지원이 되어야지. cors 출처도 그렇고, 그리고 기타 등등."

## 2. 설계 목표 (Goals)

| 목표 | 측정 기준 |
|------|----------|
| **G1. 도메인-특화 입력 위젯** | `allowed_tools` → 백엔드 카탈로그 기반 멀티-셀렉트, `cors_origins` → URL 검증 태그 에디터 |
| **G2. 슬라이더 도입** | `range` 필드 타입 신설, **15개 필드** 변환 |
| **G3. 기존 패턴 일관성** | `FieldControl` switch case 추가, `ExecAllowlistEditor` 재사용/확장 |
| **G4. i18n 일관성** | 모든 라벨/placeholder/에러 메시지 i18n key로 분리 |
| **G5. 일관된 비활성화 처리** | 신규 위젯도 기존 `disabled` prop 받아 흐리게 표시 |
| **G6. 의존성 시스템 통합** | `dependsOn`이 모든 신규 컨트롤에 자동 적용 |
| **G7. 백엔드 API 신설** | `GET /api/tools/registry` — 런타임 툴 카탈로그 (oxi-sdk `ToolRegistry`) |

## 3. 범위 (Scope)

### 3.1 In-scope (이번 PR)

- **`allowed_tools` 필드**: `csv` → **백엔드 카탈로그 + 자유 입력 (하이브리드) 멀티-셀렉트**
- **`cors_origins` 필드**: `csv` → **URL 검증 태그 에디터**
- **`range` 필드 타입**: `FieldType` 추가 (이미 일부 진행됨)
- **15개 숫자 필드**: `number` → `range` 슬라이더
- **`ExecAllowlistEditor`**: 검증/제안 슬롯 추가하여 재사용
- **백엔드 API**: `GET /api/tools/registry` 신설

### 3.2 Out-of-scope (후속 작업)

- `number` 입력 자체 제거 (fallback용 유지)
- `range`의 단위 표시 (`초`, `회`, `MB` 등) — 다음 디자인 사이클
- 백엔드 툴 카탈로그 자동 발견 (MCP 동적 툴) — 다음 단계

## 4. 아키텍처

### 4.1 컴포넌트 재사용 전략

```
                  ┌──────────────────────────────┐
                  │  ExecAllowlistEditor (기존)  │
                  │  (태그 입력 + chip 렌더링)    │
                  └──────────────┬───────────────┘
                                 │ props 확장
                ┌────────────────┼────────────────────┐
                │                │                    │
        ┌───────▼──────┐  ┌──────▼─────┐  ┌──────────▼──────────┐
        │ 자유 명령 입력 │  │ URL 검증   │  │ 카탈로그 멀티-셀렉트 │
        │ (allowed_    │  │ (cors_     │  │ (allowed_tools)     │
        │  commands)   │  │  origins)  │  │                     │
        └──────────────┘  └────────────┘  └─────────────────────┘
                tags            tags              tags + suggestions
```

**설계 결정**:
- `ExecAllowlistEditor`에 `validate?: (v: string) => string | null` prop 추가 → CORS 검증용
- `ExecAllowlistEditor`에 `suggestions?: { value: string; labelKey: string; categoryKey: string }[]` prop 추가 → 툴 카탈로그용
- 두 prop 모두 optional → 기존 `allowed_commands` 사용처는 변경 없음
- 새 `FieldType` 추가하지 않음: `'tags'`는 그대로 유지하고 동작은 `suggestions`/`validate` 유무로 분기

### 4.2 `range` 슬라이더 통합

`FieldType`에 `'range'` 추가, `min`/`max`/`step`은 `SettingsFieldDef`에 optional (range는 필수).

**표시 형식 결정**:
- `step >= 1` → 정수 표시 (`3`, `120`, `600`)
- `step < 1` → 소수점 2자리 (`0.85`, `0.05`)
- `step` 미지정 → 기본 `1`
- `min` 미지정 → 기본 `0`

### 4.3 데이터 흐름

**Slider**:
```
사용자 입력
    │
    ▼
FieldControl 'range' case
    │ String(value) → Number (parse)
    │ Slider.onValueChange([n]) → onChange(String(n))
    ▼
formValues[section][field.key] = "3"  (string, 다른 타입과 일관)
    │
    ▼ 저장 시
buildPayload()
    │ field.type === 'range' → Number(raw) → setNestedValue
    ▼
JSON PATCH to backend  (number 그대로)
```

**Tool picker**:
```
GET /api/tools/registry
    │ ToolMeta[]
    ▼
useToolCatalog() (React Query, staleTime 5min)
    │
    ▼
AllowedToolsPicker
    │ suggestions prop = ToolMeta[] 변환
    ▼
ExecAllowlistEditor.suggestions
```

## 5. 파일 변경 계획

### 5.1 신규 생성 (백엔드)

| 파일 | 역할 |
|------|------|
| `crates/oxios-kernel/src/tools/registry.rs` | `ToolMeta` 구조 + `known_tools()` 정적 매핑 (이름→설명, 카테고리) |
| `crates/oxios-kernel/src/kernel_handle/security_api.rs` (또는 신규 `tools_api.rs`) | `list_available_tools() -> Vec<ToolMeta>` 메서드 |
| `surface/oxios-web/src/routes/tools.rs` (또는 기존 `security.rs`에 추가) | `GET /api/tools/registry` 핸들러 |
| `surface/oxios-web/src/main.rs` (또는 `mod.rs`) | 신규 라우트 모듈 등록 |

### 5.2 신규 생성 (프론트엔드)

| 파일 | 역할 |
|------|------|
| `components/settings/allowed-tools-picker.tsx` | `ExecAllowlistEditor` 래퍼 + 백엔드 카탈로그 fetch + 자유 입력 |
| `components/settings/cors-origins-editor.tsx` | `ExecAllowlistEditor` 래퍼 + URL 검증 |
| `lib/cors-validator.ts` | CORS origin URL 검증 순수 함수 |
| `hooks/use-tool-catalog.ts` | `GET /api/tools/registry` React Query 훅 |

### 5.3 수정

| 파일 | 변경 |
|------|------|
| `components/ui/slider.tsx` | 기존 — 사용 (변경 없음, 이미 생성됨) |
| `components/settings/exec-allowlist-editor.tsx` | `validate?`, `suggestions?` props 추가 |
| `components/settings/field-defs.ts` | `min/max/step` 속성 (이미 일부 추가됨), 9개 필드 `type: 'range'`로 변경, `allowed_tools`를 tags+suggestions로, `cors_origins`를 tags+validate로 |
| `components/settings/field-row.tsx` | `FieldControl` switch에 `'range'` case 추가, `'tags'`에 `validate`/`suggestions` 전달 |
| `routes/settings.tsx` | legacy 필드 6개도 `type: 'range'`로 변경, `buildPayload`에 `'range'` 처리 추가 |
| `i18n/locales/*/settings.json` | 신규 키 추가 (아래 §7) |

## 6. 카탈로그 정의

### 6.1 백엔드 툴 카탈로그 API

**엔드포인트**: `GET /api/tools/registry`

**응답 형식**:
```json
{
  "tools": [
    {
      "name": "exec",
      "description": "통합 워크스페이스/호스트 명령 실행 도구",
      "category": "exec"
    },
    {
      "name": "memory_read",
      "description": "에이전트 메모리에서 항목 읽기",
      "category": "memory"
    }
  ]
}
```

**백엔드 구현 경로**:

1. `crates/oxios-kernel/src/tools/registry.rs` 신규 모듈
   - `pub fn known_tools() -> &'static [ToolMeta]` — 정적 메타데이터 매핑
   - 메타데이터는 `oxi_sdk::AgentTool::name()` 값과 1:1 매칭

2. `KernelHandle`에 `pub fn list_available_tools(&self) -> Vec<ToolMeta>` 추가
   - 또는 `InfraApi`에 추가 (재사용성 고려)

3. `surface/oxios-web/src/routes/tools.rs` (또는 기존 `security.rs`에 추가)
   - `GET /api/tools/registry` 핸들러 → 200 JSON

**ToolMeta 구조 (Rust)**:
```rust
#[derive(Serialize, Clone)]
pub struct ToolMeta {
    pub name: String,        // AgentTool::name() 와 일치
    pub description: String, // 한국어 (i18n은 프론트에서 처리)
    pub category: String,    // "fs" | "exec" | "memory" | "comms" | "system" | "a2a"
}
```

**프론트엔드 인터페이스**:
```typescript
// hooks/use-tool-catalog.ts
interface ToolMeta {
  name: string
  description: string
  category: 'fs' | 'exec' | 'memory' | 'comms' | 'system' | 'a2a'
}

function useToolCatalog() {
  return useQuery({
    queryKey: ['tools', 'registry'],
    queryFn: () => api.get<{ tools: ToolMeta[] }>('/api/tools/registry'),
    staleTime: 5 * 60 * 1000,  // 5분 캐시
  })
}
```

**카탈로그 외 값 처리** (e.g. `mcp__github__create_issue`):
- 백엔드 응답에 `mcp.*` 툴도 포함 가능 (MCP 서버 동적 등록 시)
- 자유 텍스트 입력은 그대로 가능 (`suggestions`은 optional)
- "고급" 모드 토글로 카탈로그 외 입력 노출

### 6.2 CORS URL 검증 규칙

```typescript
// lib/cors-validator.ts
export function validateCorsOrigin(value: string): string | null {
  const trimmed = value.trim()
  if (!trimmed) return 'cors.errors.empty'
  try {
    const u = new URL(trimmed)
    if (u.protocol !== 'http:' && u.protocol !== 'https:') {
      return 'cors.errors.invalidProtocol'
    }
    if (u.pathname !== '/' || u.search || u.hash) {
      return 'cors.errors.pathNotAllowed'
    }
    return null  // 유효
  } catch {
    return 'cors.errors.invalidUrl'
  }
}
```

허용 형식: `http://localhost:4200`, `https://app.example.com`
비허용: `localhost:4200` (프로토콜 누락), `https://app.example.com/api` (path), `*` (와일드카드)

## 7. i18n 키 추가

```json
{
  "settings": {
    "security": {
      "allowedToolsDescription": "에이전트가 사용할 수 있는 도구. 비워두면 모두 허용.",
      "allowedToolsAdvanced": "고급: 카탈로그에 없는 도구 직접 입력",
      "corsOriginsDescription": "CORS를 허용할 출처. 정확한 origin만 (path/query/hash 불가).",
      "corsErrorsEmpty": "출처를 입력하세요",
      "corsErrorsInvalidUrl": "올바른 URL이 아닙니다 (예: http://localhost:3000)",
      "corsErrorsInvalidProtocol": "http:// 또는 https:// 만 허용",
      "corsErrorsPathNotAllowed": "origin만 입력 (path, query, hash 불가)"
    }
  },
  "tools": {
    "exec": "Exec (명령 실행)",
    "read": "Read (파일 읽기)",
    "write": "Write (파일 쓰기)",
    "edit": "Edit (파일 편집)",
    "bash": "Bash (셸 명령)",
    "browse": "Browse (웹 브라우저)",
    "webSearch": "Web Search (웹 검색)",
    "memoryRead": "Memory Read",
    "memoryWrite": "Memory Write",
    "memorySearch": "Memory Search",
    "knowledge": "Knowledge",
    "cron": "Cron (스케줄 작업)",
    "calendar": "Calendar (일정)",
    "sendEmail": "Send Email",
    "budget": "Budget (예산)",
    "resource": "Resource (리소스)",
    "security": "Security (보안)",
    "persona": "Persona (페르소나)",
    "project": "Project (프로젝트)",
    "kernelAgent": "Kernel Agent",
    "marketplace": "Marketplace",
    "a2aDelegate": "A2A Delegate",
    "a2aSend": "A2A Send",
    "a2aQuery": "A2A Query"
  },
  "categories": {
    "fs": "파일 시스템",
    "exec": "실행",
    "memory": "메모리",
    "comms": "통신",
    "system": "시스템",
    "a2a": "에이전트 간"
  }
}
```

## 8. 필드 변환 매트릭스 (확장 17개)

| # | 섹션 | 필드 | 변경 전 | 변경 후 | min | max | step | 비고 |
|---|------|------|---------|---------|-----|-----|------|------|
| 1 | orchestrator | `max_evolution_iterations` | `number` | **`range`** | 1 | 10 | 1 | 핵심 |
| 2 | orchestrator | `min_evaluation_score` | `number` | **`range`** | 0 | 1 | 0.05 | 핵심 |
| 3 | kernel | `max_agents` | `number` | **`range`** | 1 | 50 | 1 | |
| 4 | scheduler | `max_concurrent` | `number` | **`range`** | 1 | 20 | 1 | |
| 5 | exec | `default_timeout_secs` | `number` | **`range`** | 10 | 600 | 10 | |
| 6 | exec | `max_timeout_secs` | `number` | **`range`** | 30 | 3600 | 30 | |
| 7 | memory | `dream_interval_hours` | `number` | **`range`** | 1 | 168 | 1 | 1h-7d |
| 8 | session | `max_sessions` | `number` | **`range`** | 10 | 500 | 10 | |
| 9 | security | `rate_limit_per_minute` | `number` | **`range`** | 10 | 300 | 10 | |
| 10 | security | `max_execution_time_secs` | `number` | **`range`** | 30 | 3600 | 30 | 확장 |
| 11 | security | `max_memory_mb` | `number` | **`range`** | 64 | 4096 | 64 | 확장 |
| 12 | session | `ttl_hours` | `number` | **`range`** | 1 | 720 | 24 | 확장 |
| 13 | scheduler | `zombie_timeout_secs` | `number` | **`range`** | 30 | 900 | 30 | 확장 |
| 14 | context | `cache_limit_entries` | `number` | **`range`** | 5 | 200 | 5 | 확장 |
| 15 | telegram | `session.rotation_hours` | `number` | **`range`** | 1 | 48 | 1 | 확장 |
| 16 | security | `allowed_tools` | `csv` | **`tags` + suggestions** | — | — | — | 백엔드 카탈로그 |
| 17 | security | `cors_origins` | `csv` | **`tags` + validate** | — | — | — | URL 검증 |

**유지 (`number`)**:
- `event_bus_capacity` (2의 거듭제곱이 일반적)
- `port` (구체적 포트)
- `max_audit_entries`, `audit.max_entries` (100~100000 — 슬라이더 정밀도 부족)
- `context.active_limit_tokens` (1000~500000)
- `telegram.session.max_messages` (0=무제한, 정확도 필요)

## 9. `range` FieldControl UI 스케치

```
[ min: 1 ]                                                [ max: 10 ]
   ├────●─────────────────────────────────────────────────┤
                              3
   현재 값: 3회 (실시간 업데이트, 슬라이더 드래그 시)
```

- 라벨과 값은 별도 표시 (기존 `FieldRow` 레이아웃 유지)
- 슬라이더 폭은 컨테이너 width에 맞춰 100% (반응형)
- 키보드 ←→ 이동 (Radix 기본), Home/End로 min/max 점프
- 탭 가능 (`tabIndex=0`)

## 10. 변경하지 않는 것 (Non-Goals)

- `SettingsSectionDef.groupId` 변경 없음
- `hotReload` / `restartScope` 의미 변경 없음
- `dependsOn` 시스템 자체는 변경 없음 (이미 작동)
- 백엔드 검증 로직 (`csv`/`tags` 모두 `string[]` 저장)
- `number` 입력 타입 자체 (fallback/추후 사용 위해 유지)

## 11. 테스트 계획

| 테스트 | 위치 |
|--------|------|
| `validateCorsOrigin` 단위 테스트 | `__tests__/lib/cors-validator.test.ts` (신규) |
| `ExecAllowlistEditor.validate` prop 동작 | `__tests__/settings/exec-allowlist-editor.test.tsx` (신규) |
| `ExecAllowlistEditor.suggestions` prop 동작 | 동상 |
| `FieldControl 'range'` case 동작 | `__tests__/settings/field-row-range.test.tsx` (신규) |
| `useToolCatalog` 훅 | `__tests__/hooks/use-tool-catalog.test.tsx` (신규) |
| `GET /api/tools/registry` 백엔드 | `crates/oxios-kernel/src/tools/registry.rs` 단위 테스트 |
| 기존 consistency 테스트 | 그대로 통과해야 함 |

## 12. 구현 순서

### Phase 1: 백엔드 툴 카탈로그 API

1. `crates/oxios-kernel/src/tools/registry.rs` — `ToolMeta` 구조 + `known_tools()` 정적 함수
2. `KernelHandle` (또는 `InfraApi`)에 `list_available_tools()` 추가
3. `surface/oxios-web/src/routes/tools.rs` — `GET /api/tools/registry` 핸들러
4. 백엔드 라우트 등록 (`mod.rs` / `main.rs`)
5. `cargo test` 통과

### Phase 2: 프론트엔드 기본 인프라

6. `lib/cors-validator.ts` — `validateCorsOrigin()` 순수 함수
7. `hooks/use-tool-catalog.ts` — React Query 훅
8. `ExecAllowlistEditor`에 `validate?`, `suggestions?` props 추가
9. `field-row.tsx` — `FieldControl`에 `'range'` case 추가
10. `field-row.tsx` — `tags` case에서 validate/suggestions 통과
11. `settings.tsx` `buildPayload` — `'range'` 처리 추가

### Phase 3: 필드 선언 변경

12. `field-defs.ts` — `allowed_tools`를 `tags`+suggestions로
13. `field-defs.ts` — `cors_origins`를 `tags`+validate로
14. `field-defs.ts` — NEW_SECTIONS 내 9개 필드를 `range`로 (min/max/step 추가)
15. `settings.tsx` — legacy `legacyFieldDefs` 내 6개 필드를 `range`로

### Phase 4: 컴포넌트

16. `components/settings/allowed-tools-picker.tsx` — 카탈로그 + 자유 입력
17. `components/settings/cors-origins-editor.tsx` — URL 검증 에디터
18. `field-row.tsx` `tags` case에서 picker/editor 디스패치

### Phase 5: i18n & 테스트

19. `i18n/locales/ko/settings.json` + `en/settings.json` — 툴 이름, CORS 에러, 신규 라벨
20. 테스트: `cors-validator.test.ts`, `exec-allowlist-editor.test.tsx`, `use-tool-catalog.test.tsx`
21. 빌드 + e2e (settings) 검증

### Phase 6: 문서화

22. `docs/designs/2026-06-15-settings-input-ux-design.md` 보관 (이미 저장됨)
23. CHANGELOG 항목 추가

## 13. 위험 분석

| 위험 | 완화 |
|------|------|
| 기존 `csv` 형식 사용자가 많음 (마이그레이션) | 백엔드 호환: `tags` case는 기존과 동일하게 `string[]` 저장 |
| `ExecAllowlistEditor` API 변경으로 회귀 | 두 prop 모두 optional — 기존 `allowed_commands` 사용처 변경 없음 |
| 슬라이더 정밀도 부족 (큰 범위) | 범위별로 slider 적합성 사전 평가 (§8에서 보류 항목 명시) |
| `min_evaluation_score` 등 float 표시 | `step < 1` 자동 감지 → `toFixed(2)` |
| i18n 누락 | 한국어/영어 모두 추가, e2e 테스트로 확인 |
| 백엔드 API 지연/실패 | `useToolCatalog`는 React Query로 캐시; 실패 시 자유 입력만 가능 (graceful degradation) |
| MCP 동적 툴 | 백엔드 응답에 포함; 프론트는 카탈로그에 없는 값 자유 입력 허용 |
| 백엔드 배포 의존성 | 백엔드 + 프론트 동시 배포 필요; 백엔드 API가 없어도 프론트는 빌드 가능 (TS 타입은 stub 사용) |
