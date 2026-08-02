# oxios — Unified Design System (Project Adaptation)

> **정규 문서:** `project-oxi/.github/DESIGN.md` (oxi 생태계 통합 디자인 시스템 v1.0).
> 이 파일은 oxios가 그 위에 갖는 **고유 표면 정체성 + 잔여 마이그레이션**만 다룬다. 공통 토큰·컴포넌트·철학은 정규 문서를 따른다.
>
> **버전:** v1.0 · **작성일:** 2026-07-31
> **참고:** `oxios/web/DESIGN.md`는 이 통합 시스템의 정규 스펙 사본(1151줄)이다. `oxios/web/src/index.css`가 이미 3-tier 구현을 완료한 상태다.

---

## oxios의 정체성

oxios는 에이전트 운영체제 대시보드다. 정보 밀도가 높고, 모든 에이전트 상태가 시각적 피드백을 요구한다. 공통 문법의 **상태색 메커니즘**과 **dense 대시보드 밀도**가 oxios만의 핵심 표면이다.

---

## 공통 시스템에서 변경 없는 것

아래는 정규 문서(§1–10)를 그대로 따른다 — oxios 고유값이 아님:

- OKLCH 3-tier 토큰 (✅ `web/src/index.css`에 이미 구현 완료)
- 중성 warm paper / cool ink 램프
- 6-hue 라벨 팔레트
- 상태색 (APCA 최적화) — oxios 대시보드 실측값이 정규 시스템의 권위 있는 출처
- SUIT(본문) + SUITE(헤드라인) + Geist Mono(코드)
- `.dark` 클래스 단일 트리거 (✅ 이미 올바름)
- `@theme inline` 노출 + 레거시 shadcn 별칭 동시 보존

---

## oxios 고유값 (공통 시스템 위에 유지)

### 1. 상태색 = 주 메커니즘

oxios에서 상태색은 장식이 아니라 **핵심 정보 채널**이다. 모든 에이전트 상태가 `text-status-{success|warning|error|info}` + 페어된 아이콘 + 라벨로 표면화된다. 색 단독 전달 금지 (접근성 §9.2).

> 정규 시스템의 상태색 OKLCH 값(Light/Dark)은 oxios 대시보드에서 실측·APCA 최적화된 값이다. 즉 oxios가 이 값들의 출처다.

### 2. 대시보드 밀도

- 기본 리듬: `gap-2` (8px) — 컴포넌트 내부
- 섹션 간격: `gap-4` (16px)
- 3-zone 레이아웃: 사이드바 + 메인 + 선택적 인스펙터 패널
- `sidebarPrimitives`를 Console / Knowledge / Chat 사이드바가 공유 (정규 §7.6)

### 3. 차트·메시지 토큰 (oxios 전용)

대시보드 데이터 시각화 전용 토큰. 공통 시스템에 속하지 않음 (검증: `web/src/index.css:291-299`):

```css
/* 차트 (5색) */
--chart-1: oklch(0.646 0.222 41.116);
--chart-2: oklch(0.6   0.118 184.704);
--chart-3: oklch(0.398 0.07  227.392);
--chart-4: oklch(0.828 0.189 84.429);
--chart-5: oklch(0.769 0.188 70.08);

/* 메시지 타입별 색 */
--message-task:      oklch(0.623 0.214 259.815);
--message-status:    oklch(0.707 0.022 261.325);
--message-result:    oklch(0.723 0.219 149.579);
--message-query:     oklch(0.627 0.265 303.9);
--message-handshake: oklch(0.769 0.188 70.08);
```

### 4. 설정 패널 토큰

```css
--surface-section:      /* 설정 섹션 표면 */
--modified-accent:      /* 수정됨 상태 악센트 */
--modified-row-bg:      /* 수정된 행 배경 */
```

---

## 잔여 마이그레이션 (oxios는 가장 앞서 있음)

3-tier 토큰은 완료. 남은 정리 작업:

### Step 1: `dark:` 리터럴 스윕
- `web/src/components/**`의 산재된 `dark:bg-*`, `dark:text-*`, `dark:border-*` → 시맨틱 유틸리티로 교체.
- lint 규칙 추가: `no-restricted-syntax`로 컴포넌트 파일의 `dark:` 금지 (`tokens/`, `design-system/`만 허용).
- **주의:** Geist 참조(10건/3파일)와 `dark:` 변형은 별개 문제. 혼동 금지.

### Step 2: 저장 키 통일
- 현재 `oxios-theme` → 정규 `oxi-theme`.
- 부트 시 1회 마이그레이션: `oxios-theme` 존재하면 `oxi-theme`으로 복사 후 기존 키 삭제.

### Step 3: 폰트 (단일 단계, 저위험)
- Geist 참조는 **10건/3파일**에만: `index.css`(×5), `tokens/index.ts`(×2), `editor-prefs.ts`(×3). **컴포넌트 `.tsx` 파일엔 0건.**
- `--font-sans` → `'SUIT Variable', system-ui, -apple-system, sans-serif`.
- `--font-mono` → `'Geist Mono'` 유지 (SUIT 모노 변형 없음).
- `--font-display`(SUITE)를 대시보드 히어어 영역(대시보드 타이틀, empty-state 히어어)에만 노출. 기존 헤딩에 소급 적용하지 않음 (visual diff 가치 낮음).
- `web/index.html`의 Geist Google Fonts `<link>` → jsDelivr SUIT import. Geist Mono `<link>`는 유지.

### Step 4: 에디터 폰트 프리셋 (`editor-prefs.ts`)
- `FONT_PRESETS`에 `'SUIT Variable'` 추가.
- **`'Serif'` 프리셋 제거** (`ui-serif, Georgia, …` — 정규 §3.1 세리프 금지에 위배). 세리프 읽기 폰트가 필요하면 콘텐츠 전용 선호로 스코프하고 디자인 시스템 밖임을 명시.
- `'Geist Sans'` deprecated 표시, v2 제거.

---

## 검증 체크리스트

- [ ] 컴포넌트 테스트 통과 (`dark:` 스윕 후)
- [ ] visual snapshot diff (상태색 의미 보존)
- [ ] Latin UI 변화 없음 (SUIT 로드 확인)
- [ ] 저장 키 마이그레이션 1회 부트 정상
- [ ] FONT_PRESETS에 세리프 옵션 잔존 없음
- [ ] 차트/메시지 토큰 표시 정상 (양 테마)

---

*공통 토큰 값·컴포넌트 스펙·철학은 `project-oxi/.github/DESIGN.md`를 따른다. oxios는 이미 정규 시스템의 가장 완성된 구현체다.*
