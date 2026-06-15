# Settings UI — Adaptive 3-Zone Redesign

> **상태**: 구현 완료 (v3 — 설계 명세를 코드에 충실히 구현: §5 3-티어 브레이크포인트 200/240/280px + 40/60→320→360px, §6 디자인 토큰 3개 추가. 이제 코드가 설계를 따름)
> **날짜**: 2026-06-15
> **범위**: `surface/oxios-web/web/src/routes/settings.tsx`, `src/components/settings/*`, `src/index.css`
> **검증**: tsc · biome · 단위 200/200 · e2e(settings) 11/11 · build · build-storybook

> **주의**: 이 문서의 설계 명세와 코드는 1:1로 일치한다. v1/v2에서 "문서를 코드에 맞췄던" 항목(브레이크포인트, 토큰)은 v3에서 코드를 설계에 맞췄다.

## 1. 동기 (Motivation)

기존 `/settings` 화면이 안고 있던 문제들:

1. **사이드바 폭 부족** (`w-52` = 208px) — 아이콘 + 텍스트가 답답하고, 그룹 헤더가 `text-2xs`(10px)로 거의 안 보임
2. **필드 행이 정의 리스트 형식** — 라벨/설명/배지를 왼쪽에 몰아넣고 컨트롤은 오른쪽 고정 폭(`w-40 sm:w-56`) → 설명 잘림, select 좁음
3. **Legacy/New 섹션 렌더링 분기** — 같은 화면인데 카드 스타일이 미세하게 다르고, `RestartBadge`는 New 섹션에만 표시됨
4. **저장 바가 하단 sticky** — 콘텐츠가 길면 저장하려면 다시 위로 스크롤해야 함
5. **Memory만 4개 서브카드, 다른 섹션은 단일 카드** — 시각적 일관성 부족 *(→ v1에서는 Memory 예외 유지, §4.2 참고)*
6. **검색/필터 없음** — 14개 섹션 중 찾는 게 힘듦
7. **네비게이션 정의가 중복** — `routes/settings.tsx`의 로컬 `navGroups`(JSX 아이콘 포함)와 `field-defs.SETTINGS_GROUPS`가 별도 존재

## 2. 설계 목표 (Goals)

| 목표 | 달성 여부 |
|------|------------|
| **3-zone adaptive layout** (rail + section tabs + content) | ✅ (§5 — rail md부터 3-티어) |
| **반응형 field row** (3-티어: 40/60 → 320 → 360) | ✅ (§5) |
| **Legacy/New 시각 통일** (둘 다 `SectionCard` + `FieldRow` 사용) | ✅ (Memory 제외, §4.2) |
| **Floating Save Dock** (변경 있을 때만 노출) | ✅ |
| **헤더 저장 상태 pill** (saved/unsaved/saving/error) | ✅ |
| **Rail 검색** (그룹/섹션 필터) | ✅ |
| **네비게이션 메타데이터 일원화** + 일관성 테스트 | ✅ (§8 — `advanced` orphan 제거, 테스트 추가) |
| **키보드 단축키** (⌘K / ⌘S / j-k / gg-G) | ✅ (§7.6) |
| **딥링크 복구** (orphan `?section=` 폴백) | ✅ (§7.5) |
| **저장 알림 단일화** (toast + 헤더 pill, 인라인 배너 제거) | ✅ (§7.4) |
| **접근성 보강** (badge aria-label, rail 자동 스크롤) | ✅ (§7.7) |
| **Storybook 시각 회귀 스토리** | ✅ (5개 컴포넌트, §12) |

## 3. 새 레이아웃

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Header (title + subtitle)                            [Saved · 14:23]   │
├──────────────┬──────────────────────────────────────────────────────────┤
│  RAIL (lg+)  │  SECTION BAR (sticky, 현재 그룹의 섹션들)               │
│              │  ┌─[Kernel]─[Execution]─[Scheduler]─…────────────────┐  │
│  [🔍 검색]    │  └──────────────────────────────────────────────────┘  │
│              │                                                          │
│  AI          │  ┌─ SectionCard ────────────────────────────────────┐   │
│   ● Engine   │  │ [icon] Title ●   6 fields [✓3 live] [⚠3 restart] │   │
│              │  │ Description                          [Reset]     │   │
│  System      │  │ ───────────────────────────────────────────────│   │
│   ● Kernel*  │  │ Label / desc / badge            [control]       │   │
│   ● Exec     │  │ ───────────────────────────────────────────────│   │
│   …(7 more)  │  │ …                                                │   │
│              │  └──────────────────────────────────────────────────┘   │
│  Security    │                                                          │
│   ● Security │                                                          │
│   ● Audit    │                                                          │
│              │                                          ┌─ SaveDock ──┐  │
│  Memory      │                                          │ 3 changes   │  │
│   ● Memory   │                                          │ [Discard]   │  │
│              │                                          │ [Review]    │  │
│  Channels    │                                          └─────────────┘  │
│   ● Telegram │   (fixed bottom-right, fade-in on change)                 │
├──────────────┴──────────────────────────────────────────────────────────┤
│  * = 미저장 변경 있음 (rail dot / 카드 좌상단 dot / 헤더 pill 연동)      │
└─────────────────────────────────────────────────────────────────────────┘
```

> 구현되지 않은 `advanced` 그룹(`resource_monitor`, `otel`, `daemon`, `persona`, `cron`, `mcp`, `browser`, `marketplace` 8개)은 이번에 `SETTINGS_GROUPS`에서 **제거**했다(§8 참고). 해당 섹션에 필드 정의와 렌더러가 생기면 그때 재추가.

## 4. 컴포넌트 구조

### 4.1 신규 컴포넌트

| 컴포넌트 | 파일 | 역할 |
|----------|------|------|
| `SettingsShell` | `components/settings/settings-shell.tsx` | 3-zone 컨테이너. rail + section tabs + children 조립. 검색 state, 모바일 `Dialog` 드로어 포함. |
| `SettingsRail` | `components/settings/settings-rail.tsx` | 좌측 그룹+섹션 트리, 검색 입력(fuzzy `includes` 매칭), 미저장 dot/badge |
| `SectionTabs` | `components/settings/section-tabs.tsx` | 상단 sticky `role="tablist"`. 현재 그룹 섹션이 2개 이상일 때만 노출 |
| `SectionCard` | `components/settings/section-card.tsx` | 통일 섹션 컨테이너. 헤더(아이콘/제목/수정 dot/필드 수/live/restart 카운트/Reset) + `FieldRow` body |
| `FieldRow` | `components/settings/field-row.tsx` (재작성) | 반응형 row. `RestartBadge` + 수정 accent bar |
| `SaveDock` | `components/settings/save-dock.tsx` | 우하단 floating save bar (presentation 전용) |
| `SettingsHeader` | `components/settings/settings-header.tsx` | 페이지 헤더 + `SaveStatusPill` |
| `SectionIcon` | `components/settings/section-icons.tsx` | `SectionIconKey` → Lucide 아이콘 매핑 |

### 4.2 제거/예외

- `components/layout/settings-layout.tsx` — 더 이상 import되지 않음. 디스크엔 남아 있음(호환용, 제거 가능). `SettingsShell`이 역할 흡수.
- `routes/settings.tsx`의 `renderNewSection` 분기 → `SectionCard`로 일원화 (`LegacySectionCard`가 legacy 필드를 `SettingsFieldDef`로 adapt하여 동일 `FieldRow` 재사용).
- **Memory 섹션은 예외**: `memory-section.tsx`가 여전히 4개 서브카드(Storage / Embedding / Learning / Dream)를 렌더링한다. 도메인 복잡도(서브섹션별 아이콘/그룹핑) 때문에 **의도적으로** 통일 패턴에서 제외했다. 다른 모든 섹션은 `SectionCard` + `FieldRow`로 통일.
- `Engine`/`Update` 섹션은 전용 컴포넌트(`EnginePanel`, `SystemUpdateCard`, `SystemToolsPanel`)를 그대로 사용(`SECTION_META.custom = true`).

## 5. 반응형 브레이크포인트 (설계 명세 = 코드 구현)

> Tailwind 기본 스케일: `md=768px`, `lg=1024px`, `xl=1280px`.

| 화면 폭 | Rail | Section tabs | Field row |
|---------|------|--------------|-----------|
| < 768px (`md` 미만) | `Dialog` 드로어 (폰) | 항상(2개 이상 시) | **스택** 1-컬럼, 컨트롤 풀폭 |
| 768–1023px (`md`~`lg` 미만) | **고정 표시, 200px** | 항상 | 2-컬럼, **유동 비율 40/60** (라벨 2fr·컨트롤 3fr) |
| 1024–1279px (`lg`~`xl` 미만) | **고정 표시, 240px** | 항상 | 2-컬럼, 라벨 유동·컨트롤 **320px** |
| ≥ 1280px (`xl`) | **고정 표시, 280px** | 항상 | 2-컬럼, 라벨 유동·컨트롤 **360px** |

**근거 클래스** (코드가 설계를 그대로 구현):
- Rail: `hidden md:block w-[200px] lg:w-[240px] xl:w-[280px]` (`settings-shell.tsx`)
- FieldRow: `flex flex-col` → `md:grid md:grid-cols-[2fr_3fr] md:gap-x-6` → `lg:grid-cols-[minmax(0,1fr)_320px] lg:gap-x-8` → `xl:grid-cols-[minmax(0,1fr)_360px]` (`field-row.tsx`)

> v2 도입: rail은 태블릿(md)부터 고정 표시되며 200→240→280px 3-티어로 확장. field row는 md에서 유동 40/60 비율(태블릿 폭에서 컨트롤이 너무 좁아지지 않도록), lg/xl에서 고정폭 컨트롤로 안정화. 단위 테스트 `settings-breakpoints.test.tsx`가 클래스 불변조건을 검증.

## 6. 시각 디테일 (Refined Industrial)

- **타이포**: Geist Sans (본문), Geist Mono (`font-mono` — number/password/csv/numbers 컨트롤, key path)
- **간격**: 8px 베이스. `SectionCard` padding `px-5 py-4`, header `border-b`, body `CardContent px-5 py-4`
- **색상**: 기존 `index.css` semantic 토큰 + **신규 디자인 토큰 3개 추가** (`index.css` `:root`/`.dark` + `@theme inline` 등록):
  - `--surface-section` = `color-mix(in oklch, var(--muted) 30%, var(--background))` — SectionCard 단일 톤 배경 (header·body 동일). muted/30을 background 위에 합성한 고체 색.
  - `--modified-accent` = `var(--primary)` — 수정된 필드 좌측 2px accent bar / 카드 ring
  - `--modified-row-bg` = `color-mix(in oklch, var(--primary) 3%, transparent)` — 수정된 row 배경 (primary 3%)
  - `color-mix` 식은 `:root`/`.dark` 양쪽에 동일하게 정의되어 테마 변수 재참조로 자동 전환. Lightning CSS가 `@supports(color:color-mix)` 폴백 포함.
  - 적용처: `SectionCard` → `bg-surface-section` + `ring-modified-accent/30`; `FieldRow` → `bg-modified-row-bg` + bar `bg-modified-accent`
- **기타 강조 (기존 토큰 재사용)**: row hover `hover:bg-muted/30`; rail active `bg-primary/10 text-primary` + left bar `bg-primary`
- **Radius**: 카드 `rounded-xl` (`Card` 기본), row `rounded-lg`, 컨트롤 `rounded-md`, pill `rounded-full`
- **Border**: 카드 `border`, row 구분선 `divide-y divide-border/40`
- **그림자**: 카드 `shadow-sm`, SaveDock `shadow-xl backdrop-blur bg-card/90`
- **애니메이션** (`index.css`에 실제 정의됨):
  - `@keyframes fade-in-up` + `.animate-fade-in-up` — `SaveDock` 입장에 사용
  - `.animate-stagger > *` — 자식 순차 등장(40ms 간격, 최대 12개 후 누적 480ms). `SettingsShell` children 컨테이너가 사용

## 7. 동작

### 7.1 Search

- Rail 상단 `<Input type="search">` (placeholder: `settings.searchPlaceholder`)
- 입력 시 `filterGroups()`가 그룹/아이템 라벨을 소문자 `includes` 매칭으로 필터
- 매칭 0개 → "No settings match" 빈 상태(`SearchX` 아이콘)
- 활성 섹션 변경 시 검색어 자동 초기화(`SettingsShell`의 `useEffect`)
- **단축키 없음**: ⌘K / Ctrl+K는 미구현 (§11)

### 7.2 Navigation

- Rail 클릭 또는 Section tab 클릭 → `onNavigate(id)` → `setActiveSection` + `?section=<id>` URL 갱신(`history.replaceState`)
- 활성 rail 항목: `aria-current="page"` + 좌측 accent bar
- Section tab: `role="tablist"` / `role="tab"` / `aria-selected`. 활성 탭이 viewport 밖이면 `scrollIntoView`
- **단축키 없음**: j/k/gg/G 미구현 (§11)

### 7.3 Unsaved indicator propagation

- `formValues` + `diffConfigs` → 섹션별 `unsavedBySection: Record<id, number>` 파생
- `SaveDock`: 전역 diff 길이로 `visible` 결정
- `SectionCard`: 자기 섹션 `unsavedCount > 0` → 좌상단 dot + `ring-1 ring-primary/30`
- `SettingsRail`: 각 항목 `data-modified="true"` + dot(비활성 시) 또는 badge(숫자)
- `SettingsHeader`: 상태 pill (`saved` / `unsaved` / `saving` / `error`)

### 7.4 Save flow

- `SaveDock` "Review" 또는 페이지 하단 fallback "Review" 버튼 → `DiffPreview` 모달 → `handleConfirmSave`
- "Discard" → `setFormValues({})` + `refetch()`
- 저장 성공: **sonner `toast.success`** + 헤더 pill "Saved · HH:MM" (`lastSavedAt.toLocaleTimeString`). hot_reload 응답에 따라 `toast` 메시지 분기(`savedApplied` / `savedWithRestart`).
- 저장 실패: `toast.error` + 헤더 `error` pill (`saveMutation.isError`)
- **인라인 `saveNotice` 배너는 제거**했다. 저장 피드백은 toast(일시적 액션) + 헤더 pill(지속적 상태) 2원으로 충분했고, 배너는 중복이었다. `saveNotice` state 자체가 삭제됨.

### 7.5 렌더링 전략 + 딥링크 복구

- 활성 섹션 **1개만** 렌더링 (`renderActiveSection`). 비활성 섹션은 DOM에서 제거 → 메모리 절약, 상태 단순화(스크롤 위치는 섹션 전환 시 유지되지 않음).
- 분기 순서: `engine` → `update` → `!meta` (null 반환) → `memory` → `channels.telegram` → `NEW_SECTIONS` 매칭 → legacy `legacyFieldsBySection` 매칭 → `null`
- **딥링크 복구**: `safeActiveSection = getSectionMeta(activeSection) ? activeSection : SECTION_META[0]?.id ?? 'engine'`. URL `?section=persona` 같은 orphan 진입 시 첫 섹션(Engine)으로 폴백하여 빈 화면을 막는다. e2e로 검증(`unknown ?section= deep-link falls back`).

### 7.6 키보드 단축키

`SettingsShell`의 글로벌 `keydown` 리스너(`window`)가 처리:

| 단축키 | 동작 | 입력 중 동작 |
|--------|------|-------------|
| `⌘K` / `Ctrl+K` | 검색 입력 포커스 + 전체 선택 | 항상 동작 (modifier 조합) |
| `⌘S` / `Ctrl+S` | Review(저장) 플로우 열기 (`onReview`) | 항상 동작, 브라우저 저장 차단 |
| `j` | 다음 섹션 | 입력 중 무시 |
| `k` | 이전 섹션 | 입력 중 무시 |
| `g g` | 첫 섹션 (700ms 내 두 번 `g`) | 입력 중 무시 |
| `G` | 마지막 섹션 | 입력 중 무시 |

- 단일키(j/k/g/G)는 `target`이 `INPUT`/`TEXTAREA`/`SELECT`/`contentEditable`이면 무시하여 텍스트 입력을 방해하지 않는다.
- ⌘S는 `preventDefault`로 브라우저 기본 "페이지 저장"을 막는다.
- e2e로 검증(`⌘K focuses the search input and j/k navigates sections`).

### 7.7 접근성

- Rail 항목: `aria-current="page"`, 활성 항목 자동 `scrollIntoView`(`SettingsRail`의 `useEffect`, j/k 네비 후에도 보이게)
- SectionTabs: `role="tablist"` / `role="tab"` / `aria-selected` + 활성 탭 자동 스크롤
- 모바일 `Dialog` (Radix 포커스 트랩)
- SaveDock `<section aria-label="Unsaved settings">`
- 검색 Input `aria-label`, clear 버튼 `aria-label`
- SectionCard 메타데이터 badge: 아이콘은 `aria-hidden`, badge 본체에 `aria-label="{count} {desc}"` 부여하여 스크린리더가 "3 Applied immediately…"로 읽도록 함

## 8. 네비게이션 메타데이터 구조

**진실의 원천은 두 배열의 조인**이다:

1. `SETTINGS_GROUPS: SettingsGroup[]` (`field-defs.ts:441`) — 5개 그룹(ai/system/security/memory/channels), 각 `sectionKeys: string[]` 보유
2. `SECTION_META: SectionMeta[]` (`field-defs.ts:539`) — 14개 섹션(ai 1·system 9·security 2·memory 1·channels 1), 각 `groupId` / `labelKey` / `descriptionKey` / `iconKey` / `custom` 보유

`routes/settings.tsx`에서 둘을 조인하여 shell groups/sections를 구성한다.

**`advanced` orphan — 해결됨**:
구 버전에는 `SETTINGS_GROUPS`에 `advanced` 그룹(8개 섹션)이 정의됐지만 `SECTION_META`엔 없어 `.filter()`에서 통째로 숨겨졌었다. 이번에 **`advanced` 그룹을 `SETTINGS_GROUPS`에서 제거**했다 — 해당 섹션들은 필드 정의와 렌더러가 없어 UI에 내보낼 수 없기 때문. `SettingsGroup.id`와 `SectionMeta.groupId`의 union 타입에서도 `'advanced'`를 제거했다. 관련 섹션이 구현되면 그때 `SECTION_META` 항목 + 렌더러와 함께 재추가.

**일관성 안전망**:
`src/__tests__/settings/settings-consistency.test.ts`(7개 테스트)가 다음 불변 조건을 검증한다:
- `SETTINGS_GROUPS.sectionKeys` ⊆ `SECTION_META` ids
- `SETTINGS_GROUPS.sectionKeys` ⊆ renderable set (렌더러 있는 섹션만 nav에)
- `SECTION_META.groupId` ∈ `SETTINGS_GROUPS` ids
- `SECTION_META` ids ⊆ renderable set
- 모든 `SECTION_META` 항목이 `labelKey`/`descriptionKey`/`iconKey` 보유
- id 중복 없음 (양쪽 배열)

이 테스트는 향후 새 섹션 추가 시 메타데이터/렌더러/nav 어느 한 곳이 빠지면 실패하여 orphan 회귀를 잡는다.

## 9. i18n 추가 키 (실제 `en.json` / `ko.json` 기준)

`settings.*` 네임스페이스에 13개 키 추가됨 (ko/en 동기화됨):

| 키 | en 값 | 비고 |
|----|-------|------|
| `searchPlaceholder` | `Search settings…` | Rail 검색 |
| `noMatches` | `No settings match "{{query}}"` | 빈 상태 |
| `modified` | `Modified` | dot `aria-label` |
| `savedAt` | `Saved · {{time}}` | 헤더 pill, `time` = `HH:MM` |
| `reviewChanges` | `Review` | SaveDock / fallback 버튼 |
| `discardChanges` | `Discard` | SaveDock 버튼 |
| `applyLive` | `live` | SectionCard badge (짧은 형) |
| `applyLiveDesc` | `Applied immediately — no daemon restart required.` | badge tooltip |
| `restartNeededCount` | `restart` | SectionCard badge (짧은 형) |
| `restartNeededDesc` | `Requires a daemon restart to take effect.` | badge tooltip |
| `change_one` | `change` | SaveDock 단수 |
| `change_other` | `changes` | SaveDock 복수 |
| `saveDockLabel` | `Unsaved settings` | SaveDock `aria-label` |

> ⚠️ 이전 문서가 나열했던 `restartNeeded`, `hotReloadCount`, `shortcutHint` 키는 **실제로 존재하지 않는다** (실제 키는 `restartNeededCount`, `applyLive`이며 `shortcutHint`는 단축키 미구현으로 추가 안 됨).

기존 키는 변경 없음.

## 10. 호환성 / 마이그레이션

- `routes/settings.tsx`의 **form state, `buildPayload`, `diffConfigs`, `handleConfirmSave`** 로직은 그대로 유지됨
- **`field-defs.ts`는 변경되었다** (`+169줄`): `SectionMeta` 타입, `SECTION_META` 배열(14개 섹션), `SectionIconKey` 타입, `getSectionMeta()` 헬퍼, `SectionIcon` 매핑용 메타데이터 추가. 기존 `SETTINGS_GROUPS` / `NEW_SECTIONS` / `findFieldDef`는 변경 없음
- `SettingsLayout`을 import하는 외부 코드는 없음 (`grep` 단일 사용처였던 settings.tsx만 사용). deprecated, 제거 가능
- i18n 키 추가만 — ko/en 양쪽 동기화

## 11. 남은 한계 / 후속 작업

> 리뷰에서 짚은 항목들은 모두 이번에 해결됐다. 아래는 **남아있는** 작은 한계들.

### 후속 작업 (저위험)

| 항목 | 설명 |
|------|------|
| **`SettingsLayout` 파일 제거** | 더 이상 import되지 않음(`components/layout/settings-layout.tsx`). 디스크에 남아 있음. 확인 후 삭제 가능. |
| **SectionTabs 화살표 키 네비** | `role="tablist"`이지만 좌/우 화살표로 탭 전환은 미구현(Roxy roving tabindex). 현재는 클릭/j-k로만 이동. |
| **`advanced` 섹션 구현** | 8개 섹션이 실제로 필요해지면 `SECTION_META` 항목 + 필드 정의 + 렌더러 추가 후 `SETTINGS_GROUPS`에 재등록. 일관성 테스트가 가이드. |

### 해결된 항목 (참고용 기록)

| 항목 | 해결 |
|------|------|
| `advanced` orphan | `SETTINGS_GROUPS`에서 제거 + 일관성 테스트 추가 (§8) |
| 이중 저장 알림 | 인라인 `saveNotice` 배너 제거, toast + 헤더 pill로 단일화 (§7.4) |
| 딥링크 빈 화면 | `safeActiveSection` 폴백 (§7.5) |
| 키보드 단축키 부재 | ⌘K/⌘S/j-k/gg-G 구현 (§7.6) |
| badge aria-label 부재 | badge `aria-label` + 아이콘 `aria-hidden` (§7.7) |
| rail 활성 항목 미스크롤 | `SettingsRail` 자동 `scrollIntoView` (§7.7) |
| Storybook 스토리 부재 | 5개 컴포넌트 스토리 추가 (§12) |

## 12. 테스트

모두 통과: `tsc` · `biome` · 단위 200/200 · e2e(settings) 11/11 · `build` · `build-storybook`.

### 단위 (vitest, 29 files / 200 tests)
- 신규 `src/__tests__/settings/settings-consistency.test.ts` (7 tests) — 네비게이션 메타데이터 불변 조건 (§8)
- 신규 `src/__tests__/settings/settings-breakpoints.test.tsx` (2 tests) — §5 브레이크포인트 클래스 불변조건 (rail md부터 3-티어, 모바일 드로어 md 미만)

### e2e (`e2e/settings.spec.ts`, 11 tests)
- 그룹 라벨 5개 노출
- 헤더 saved status pill
- embedding provider restart badge
- exec allowlist editor + live/restart badge
- memory 4 서브카드
- save dock이 필드 수정 후 노출
- save flow → diff preview
- telegram PATCH body shape (P0-1)
- exec PATCH가 `memory.embedding.provider` 보존 (F-1, 클라이언트 불변 조건)
- **⌘K 검색 포커스 + j/k 네비게이션** (신규)
- **orphan `?section=` 딥링크 폴백** (신규)
- *테스트 viewport: 1440×900 (`beforeEach`) — rail이 `lg+` 설계라 1280×720에서는 마지막 rail 항목(Telegram)이 fold 아래에 깔림.*

### Storybook 시각 회귀 (5개 스토리 파일)
- `.storybook/i18n-mock.tsx` — `I18nextProvider` + `en.json` 리소스 주입 헬퍼 (`i18nDecorator`)
- `save-dock.stories.tsx` — Default / MixedChanges / RestartRequired / Applying / ManyChanges / Hidden
- `section-card.stories.tsx` — Default / WithIcon / Modified / Minimal / RestartHeavy
- `field-row.stories.tsx` — Toggle / Select / NumberField / Text / Modified / AllVariants
- `settings-rail.stories.tsx` — Default / FirstActive / MemoryActive / SearchFiltered / NoMatches
- `section-tabs.stories.tsx` — Default / FirstActive / LastActive / SmallGroup / Overflow
- `bun run build-storybook` 통과
