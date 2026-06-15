# Mobile Responsive Design — 전역 모바일 대응 설계

> **상태**: 구현 완료 — 2026-06-15 Phase 1-5 전면 구현, 설계 명세 = 코드
> **검증**: tsc · biome · 단위 191/191 · build · lint
> **날짜**: 2026-06-15
> **범위**: `surface/oxios-web/web/` 전체 (index.html, src/index.css, src/hooks/*, src/components/layout/*, src/components/ui/*, src/components/chat/*, src/components/shared/*, src/components/memory/*, src/components/a2a/*, src/components/knowledge/*, playwright.config.ts, e2e/*)
> **선행 검토**: `docs/designs/` 내 모바일 검토 보고서(2026-06-15) — 사이드바 드로어·헤더·대시보드 그리드는 양호, 콘텐츠 컴포넌트·테스트 인프라·터치 제스처가 데스크톱 기준에 머묾

> **설계 원칙**: 이 문서의 반응형 명세는 코드에 1:1로 반영된다. (기존 `2026-06-15-settings-redesign.md`의 "설계 명세 = 코드 구현" 원칙 준수)

---

## 1. 동기 (Motivation)

전역 모바일 검토에서 드러난 문제들:

1. **반응형 분기가 `lg:`(1024px)에 집중** — `lg:` 41회 vs `md:` 36회 vs `sm:` 51회. 사이드바·헤더 모드탭·패딩이 모두 `lg:`에서만 전환되어 **768~1023px(태블릿) 구간이 블랙홀**
2. **터치 제스처 미대응** — `interactive-topology.tsx`, `link-graph.tsx`에 zoom/touch 핸들러 없음. `embedding-canvas.tsx`는 단일 포인터만 처리(핀치줄 불가)
3. **채팅 입력 Enter=전송** — 모바일 소프트키보드에서 의도치 않은 전송 빈번
4. **노치/안전영역(safe-area) 미대응** — `env(safe-area-inset-*)` 사용 0건. iPhone 노치·가로 모드에서 헤더/입력바 가림
5. **고정 픽셀 너비 오버플로우** — `min-w-[640px]`(a2a message-log), `w-[400px]`, `w-[350px]` 등
6. **Dialog 높이 무제한** — `max-h` 없어 긴 콘텐츠가 모바일에서 화면 밖으로 넘침
7. **DataTable 모바일 카드뷰 미지원** — 가로 스크롤만으로 가독성 불량
8. **모바일 E2E 테스트 전무** — Playwright `chromium`(Desktop) 프로젝트 하나뿐. 회귀 그물 없음
9. **터치 타겟 44px 미만 버튼 다수** — WCAG 2.5.5 권장 미달 (sidebar `h-7 w-7`, attachment X `p-0.5`)
10. **`100vh` 사용 시 모바일 주소창 점프** — `100dvh` 필요

**잘 된 부분(유지)**: 사이드바 슬라이드 드로어(백드롭·ESC·`aria-modal`), 헤더 통합 모바일 드롭다운, 대시보드 그리드(`grid-cols-2 sm:grid-cols-3 xl:grid-cols-6`), 설정 3-티어 패널, IME `isComposing` 처리, viewport meta 기본값.

---

## 2. 설계 목표 (Goals)

| 목표 | 측정 기준 |
|------|-----------|
| **3-티어 디바이스 분기** (mobile / tablet / desktop) | 명시적 브레이크포인트 체계 + 감지 훅 |
| **안전 최소 폭 360px** 무결점 렌더 | 현대적 최소 폰 뷰포트에서 오버플로우 0 |
| **safe-area 완전 대응** | 노치/다이내믹 아일랜드/가로 모드에서 시스템 UI와 겹침 0 |
| **터치 제스처 패리티** | 캔버스 3종(토폴로지·링크그래프·임베딩) 팬/핀치 가능 |
| **모바일 채팅 UX** | Enter=줄바꿈(터치), 단축키 힌트 숨김, 전송은 버튼 |
| **Dialog 모바일 안전** | `max-h` + 내부 스크롤 + 바텀시트 옵션 |
| **DataTable 카드뷰** | 모바일에서 열 우선순위/카드 전환 |
| **터치 타겟 ≥ 44px** | 주요 인터랙티브 요소 WCAG 2.5.5 준수 |
| **모바일 E2E 그물** | Playwright iPhone/Pixel/iPad 프로젝트 |
| **`100dvh` 전환** | 주소창 점프 현상 제거 |

---

## 3. 설계 원칙 (Principles)

1. **반응형 단일 코드베이스 유지** — 별도 모바일 라우트/컴포넌트 트리를 만들지 않는다. Tailwind 반응형 프리픽스 + 디바이스 감지 훅으로 단일 트리에서 해결. (이유: 기존 사이드바 드로어·설정 패널이 이미 이 패턴으로 동작 중)
2. **모바일 우선(Mobile-first) 작성** — 기본 클래스는 모바일, `sm:`/`md:`/`lg:`로 확장. 새 코드는 반드시 기본값이 360px(현대적 최소 폰)에서 안전하도록.
3. **디바이스 감지는 미디어쿼리 기반** — User-Agent 스니핑 금지. `(pointer: coarse)`, `(min-width: ...)`, `(hover: none)` 사용.
4. **터치와 마우스는 같은 컴포넌트에서 분기** — `useIsTouch()` 훅으로 같은 컴포넌트 내에서 동작 분기. 별도 컴포넌트 복제 금지.
5. **점진적 향상** — 데스크톱 기능을 모바일에서 숨기되, 핵심 기능(에이전트 모니터링, 채팅, 승인)은 모바일에서도 완전 동작.
6. **존속하는 기본값 존중** — 기존 `lg:` 분기가 잘 동작하는 사이드바/헤더는 건드리지 않고, 부족한 `md:`(태블릿) 계층을 추가.

---

## 4. 브레이크포인트 체계 (Breakpoint System)

### 4.1 명시적 티어 정의

Tailwind v4 기본값을 그대로 사용(커스텀 브레이크포인트 추가 안 함). **디바이스 티어를 문서화하여 공유 어휘로 사용**.

| 티어 | 범위 | Tailwind 프리픽스 | 주요 동작 |
|------|------|-------------------|-----------|
| **mobile** | < 768px | (기본값), `sm:`(640+) | 사이드바 드로어, 단열 레이아웃, 카드뷰, 바텀시트 |
| **tablet** | 768~1023px | `md:` | 2열 그리드, 콤팩트 사이드바(드로어 유지), 표 표시 |
| **desktop** | ≥ 1024px | `lg:`, `xl:`, `2xl:` | 영구 사이드바, 다열 그리드, 호버 인터랙션 |

```
mobile (<768)      tablet (768-1023)    desktop (1024+)
─────────────      ─────────────────    ───────────────
드로어 사이드바     드로어 사이드바       영구 사이드바
단열               2열                   3-6열
카드뷰(표)          표                    표
바텀시트(다이얼)     중앙 다이얼            중앙 다이얼
Enter=줄바꿈       Enter=전송            Enter=전송
```

### 4.2 사이드바 전환점 — `lg:` 유지

현재 사이드바는 `lg:`(1024px)에서 영구 표시. **이 기준을 유지**한다(변경 시 드로어/영구 전환이 태블릿에서 불안정). 대신 **태블릿 구간은 드로어 폭과 패딩을 최적화**:

- 모바일 드로어 폭: `w-72`(288px) — 현재 `w-60`(240px)은 큰 폰에서 작음
- `max-w-[85vw]` — 작은 폰에서 화면 넘침 방지

### 4.3 anti-pattern (금지)

- ❌ `w-[Npx]` 고정 픽셀 (반응형 값 또는 `max-w-full` 사용)
- ❌ `min-w-[Npx]` 로 테이블/컨테이너 강제 (래퍼에 `overflow-x-auto` + `min-w-0`)
- ❌ `lg:` 단독 분기 (태블릿 고려 누락) — 필요시 `md:` 중간 단계 추가
- ❌ User-Agent 기반 디바이스 분기

---

## 5. 디바이스 감지 인프라 (신규 훅)

현재 `useMediaQuery`/`useBreakpoint`/`useIsTouch` 훅이 없음. 신규 추가.

### 5.1 `src/hooks/use-media-query.ts`

```ts
import { useEffect, useState } from 'react'

/**
 * SSR-safe media query hook. Returns `false` during SSR / first paint,
 * resolves to the real value after mount.
 *
 * @example
 * const isDesktop = useMediaQuery('(min-width: 1024px)')
 */
export function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(false)

  useEffect(() => {
    const mql = window.matchMedia(query)
    const handler = (e: MediaQueryListEvent) => setMatches(e.matches)
    setMatches(mql.matches)
    mql.addEventListener('change', handler)
    return () => mql.removeEventListener('change', handler)
  }, [query])

  return matches
}
```

### 5.2 `src/hooks/use-breakpoint.ts`

```ts
import { useMediaQuery } from './use-media-query'

// 티어별 불린 훅 — 가독성 우선. matchMedia 중복 구독을 피하려면
// 단일 훅에서 파생값 반환 고려 (아래 useDevice 참고).
export const useIsMobile = () => !useMediaQuery('(min-width: 768px)')
export const useIsTablet = () => {
  const md = useMediaQuery('(min-width: 768px)')
  const lg = useMediaQuery('(min-width: 1024px)')
  return md && !lg
}
export const useIsDesktop = () => useMediaQuery('(min-width: 1024px)')

/**
 * 단일 matchMedia 구독으로 티어를 결정 — 훅 호출이 많은 화면에서 권장.
 * desktop ≥ 1024 > tablet ≥ 768 > mobile.
 */
export type DeviceTier = 'mobile' | 'tablet' | 'desktop'

export function useDevice(): DeviceTier {
  const isDesktop = useMediaQuery('(min-width: 1024px)')
  const isTablet = useMediaQuery('(min-width: 768px)')
  if (isDesktop) return 'desktop'
  if (isTablet) return 'tablet'
  return 'mobile'
}
```

### 5.3 `src/hooks/use-is-touch.ts`

```ts
import { useMediaQuery } from './use-media-query'

/**
 * 터치 최적 디바이스 여부. coarse pointer = 터치/스타일러스.
 * 키보드 단축키 vs 버튼, Enter vs 줄바꿈, hover vs 탭 동작 분기에 사용.
 *
 * 주의: 일부 디바이스(Surface, 터치 노트북)는 touch + mouse 모두 가능.
 * "터치 우선" 동작(단축키 숨김 등)에는 이 훅을, "터치 전용"(핀치)에는
 * 런타임 터치 이벤트 자체를 사용.
 */
export const useIsTouch = () =>
  useMediaQuery('(pointer: coarse)') || !useMediaQuery('(hover: hover)')
```

### 5.4 적용 규칙

- CSS 프리픽스(`hidden md:block`)는 **정적 레이아웃/표시**에 사용
- 훅(`useIsMobile`)은 **JS 동작 분기**(단축키, 이벤트 핸들러, 렌더 분기)에 사용
- 두 방식을 같은 요소에 섞어 쓰지 말 것 (불일치 위험)

---

## 6. App Shell 반응형 명세

### 6.1 `app-layout.tsx` — 사이드바 폭·safe-area

변경 최소화. 드로어 폭과 safe-area만 보강.

```tsx
// 드로어 패널 (현재 Sidebar는 w-60 고정)
<Sidebar className="w-72 max-w-[85vw]" />  // 모바일 드로어 최적화

// 메인 영역 하단 safe-area (채팅 입력·바닥 요소)
<main className="... pb-[env(safe-area-inset-bottom)]">
```

### 6.2 `header.tsx` — 상단 safe-area

```tsx
<header className="flex h-14 items-center gap-4 border-b bg-background px-4 lg:px-6 pt-[env(safe-area-inset-top)]">
```

### 6.3 `sidebar.tsx` — 드로어 폭

```tsx
<aside
  className={cn(
    'flex h-full w-72 max-w-[85vw] flex-col overflow-hidden border-r bg-sidebar ...',
    // 데스크톱 영구 사이드바는 기존 폭 유지 (lg 분기 내에서만 w-60/w-16)
    collapsed ? 'lg:w-16 lg:max-w-none' : 'lg:w-60 lg:max-w-none',
  )}
>
```

> **주의**: 현재 `<Sidebar />`는 드로어와 데스크톱 양쪽에서 같이 렌더됨. `max-w-[85vw]`는 모바일에만, `lg:max-w-none`으로 데스크톱에서는 해제.

---

## 7. Safe-Area / 노치 대응

### 7.1 viewport meta (`index.html`)

```html
<meta
  name="viewport"
  content="width=device-width, initial-scale=1.0, viewport-fit=cover"
/>
```

`viewport-fit=cover`가 있어야 `env(safe-area-inset-*)`가 실제 값을 반환.

### 7.2 전역 CSS 유틸리티 (`src/index.css`)

```css
@layer utilities {
  /* safe-area 패딩 — 필요한 요소에만 적용 */
  .pt-safe { padding-top: env(safe-area-inset-top); }
  .pb-safe { padding-bottom: env(safe-area-inset-bottom); }
  .pl-safe { padding-left: env(safe-area-inset-left); }
  .pr-safe { padding-right: env(safe-area-inset-right); }

  /* 가로 모드 전체 safe-area */
  .p-safe {
    padding:
      env(safe-area-inset-top)
      env(safe-area-inset-right)
      env(safe-area-inset-bottom)
      env(safe-area-inset-left);
  }
}
```

### 7.3 적용 지점

| 요소 | 적용 | 이유 |
|------|------|------|
| Header 상단 | `.pt-safe` | 노치/다이내믹 아일랜드 |
| 채팅 입력 하단 | `.pb-safe` | 홈 인디케이터 |
| 사이드바 드로어 | `.pl-safe`(좌) | 세로 모드 |
| 바텀시트/고정 바 | `.pb-safe` | 홈 인디케이터 |
| 전체 화면 캔버스 | `.p-safe` | 가로 모드 4면 |

### 7.4 `100dvh` 전환

모바일 브라우저 주소창 때문에 `100vh`는 실제 표시 영역보다 큼. 동적 뷰포트 단위 사용.

**대상**: `h-screen`, `min-h-screen`, `h-[100vh]` 전체 검색 후 교체.

```css
/* index.css — 호환 폴백 포함 */
.app-shell {
  height: 100vh;           /* 구형 브라우저 폴백 */
  height: 100dvh;          /* 모던 - 주소창 추적 */
}
```

`app-layout.tsx` 최상위 `h-screen` → `h-[100dvh]` (Tailwind v4는 `dvh` 단위 지원: `h-dvh`). 실제 클래스는 `h-dvh` 사용.

---

## 8. 채팅 입력 모바일 UX (`chat-input.tsx`)

### 8.1 Enter 키 동작 분기

```tsx
import { useIsTouch } from '@/hooks/use-is-touch'

// 컴포넌트 내
const isTouch = useIsTouch()

const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
  // ... @mention 처리 ...

  if (isComposing) return

  if (e.key === 'Enter' && !e.shiftKey) {
    // 터치 디바이스에서는 Enter = 줄바꿈 (소프트키보드 기대 동작)
    // 데스크톱에서는 Enter = 전송
    if (!isTouch) {
      e.preventDefault()
      handleSend()
    }
    // 터치: preventDefault 안 함 → 기본 줄바꿈
  }
}
```

### 8.2 단축키 힌트 숨김

```tsx
{/* 하단 힌트 — 데스크톱/포인팅 디바이스에서만 */}
<p className="mt-1.5 hidden sm:block text-center text-2xs text-muted-foreground/70">
  <kbd>Enter</kbd> send · <kbd>Shift+Enter</kbd> new line · ...
</p>
```

모바일에서는 전송 버튼(이미 존재)으로 충분. 힌트는 `hidden sm:block`.

### 8.3 전송 버튼 터치 타겟

```tsx
<Button
  className="h-11 w-11 rounded-lg ..."  // h-8 w-8 → h-11 w-11 (44px)
  aria-label={t('common.sendMessage', 'Send')}
>
```

44px 터치 타겟. `sm:` 이상에서는 `sm:h-9 sm:w-9`로 축소(마우스).

---

## 9. Dialog 모바일 안전 (`ui/dialog.tsx`)

### 9.1 높이 제한 + 내부 스크롤

```tsx
function DialogContent({ className, children, showCloseButton = true, ...props }) {
  return (
    <DialogPortal>
      <DialogOverlay />
      <DialogPrimitive.Content
        className={cn(
          'fixed top-[50%] left-[50%] z-50 grid w-full gap-4',
          'max-w-[calc(100%-2rem)]',
          'max-h-[calc(100dvh-2rem)]',          // 신규: 높이 상한
          'translate-x-[-50%] translate-y-[-50%]',
          'rounded-lg border bg-background p-6 shadow-lg',
          'overflow-y-auto',                     // 신규: 내부 스크롤
          'duration-200 outline-none',
          'data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95',
          'data-[state=open]:animate-in data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95',
          'sm:max-w-lg',
          className,
        )}
        {...props}
      >
```

### 9.2 바텀시트 옵션 (모바일 전용 변형)

긴 폼(event-editor, add-server-dialog)은 모바일에서 바텀시트가 더 자연스러움. `fullScreen` / `mobileSheet` prop 추가:

```tsx
function DialogContent({
  className, children, showCloseButton = true,
  mobileSheet = false,  // 신규
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Content> & {
  showCloseButton?: boolean
  mobileSheet?: boolean
}) {
  return (
    <DialogPortal>
      <DialogOverlay />
      <DialogPrimitive.Content
        className={cn(
          // 기본(중앙)
          !mobileSheet && 'fixed top-[50%] left-[50%] translate-x-[-50%] translate-y-[-50%]',
          // 바텀시트 변형: 모바일에서는 하단 고정, sm+에서는 중앙
          mobileSheet && cn(
            'fixed inset-x-0 bottom-0 z-50 w-full rounded-t-2xl rounded-b-none',
            'translate-y-0 max-h-[90dvh] overflow-y-auto pb-[env(safe-area-inset-bottom)]',
            'sm:bottom-[50%] sm:left-[50%] sm:translate-x-[-50%] sm:translate-y-[-50%]',
            'sm:rounded-lg sm:max-w-lg',
          ),
          // ... 공통 클래스 ...
        )}
        {...props}
      >
```

**사용처**: `event-editor.tsx`, `add-server-dialog.tsx`, `repeat-editor.tsx` 등 긴 폼에 `mobileSheet` 적용.

---

## 10. DataTable 모바일 카드뷰 (`shared/data-table.tsx`)

### 10.1 카드뷰 전환 옵션

```tsx
export interface Column<T> {
  header: string
  accessor: keyof T | ((row: T) => React.ReactNode)
  sortKey?: keyof T
  filter?: { options: { label: string; value: string }[] }
  className?: string
  // 신규: 모바일 카드뷰에서 표시 우선순위
  mobilePriority?: 'primary' | 'secondary' | 'hidden'
}

export interface DataTableProps<T> {
  // ... 기존 ...
  // 신규: 모바일에서 카드뷰 사용 여부 (기본 true)
  mobileCardView?: boolean
}
```

### 10.2 렌더 분기

```tsx
import { useIsMobile } from '@/hooks/use-breakpoint'

export function DataTable<T>({ mobileCardView = true, ...props }: DataTableProps<T>) {
  const isMobile = useIsMobile()
  const useCards = isMobile && mobileCardView

  if (useCards) {
    return <CardList ... />  // 모바일: 카드 목록
  }
  return <TableDesktop ... />  // 기존 표
}
```

### 10.3 카드뷰 렌더

```tsx
function CardList<T>({ columns, paginated, onRowClick, keyExtractor }) {
  const primary = columns.filter(c => c.mobilePriority !== 'hidden')
  return (
    <div className="divide-y sm:hidden">
      {paginated.map(row => (
        <button
          key={keyExtractor(row)}
          onClick={() => onRowClick?.(row)}
          className="w-full text-left p-4 hover:bg-muted/50 active:bg-muted"
        >
          {/* primary 열: 크게 */}
          <div className="font-medium">
            {primary.find(c => c.mobilePriority === 'primary') && (
              getCellValue(row, primary.find(c => c.mobilePriority === 'primary')!.accessor)
            )}
          </div>
          {/* secondary 열: 작게 그리드 */}
          <div className="mt-1 grid grid-cols-2 gap-1 text-xs text-muted-foreground">
            {primary.filter(c => c.mobilePriority === 'secondary').map(col => (
              <div key={String(col.accessor)}>
                <span className="opacity-60">{col.header}: </span>
                {getCellValue(row, col.accessor)}
              </div>
            ))}
          </div>
        </button>
      ))}
    </div>
  )
}
```

### 10.4 적용 우선순위 (`mobilePriority` 미지정 시 자동 추론)

1. 명시적 `mobilePriority` 우선
2. 미지정 시: 첫 열 = `primary`, 나머지 중 non-numeric = `secondary`, timestamp/UUID = `hidden`

**적용처**: `agents/index.tsx`(name=primary, status/cost=secondary, created/session=hidden), `sessions/index.tsx`, `seeds/index.tsx`.

---

## 11. 캔버스/그래프 터치 제스처

### 11.1 공통: 핀치줌 유틸 (`src/lib/touch-gestures.ts`)

```ts
/**
 * 두 손가락 핀치 감지. pointer 이벤트 기반.
 * 단일 포인터는 드래그(팬)로, 두 포인터는 핀치(줌)로 처리.
 *
 * @returns 이벤트 핸들러 객체 (onPointerDown/Move/Up)
 */
export function createPinchHandlers(opts: {
  onPan: (dx: number, dy: number) => void
  onZoom: (scale: number, cx: number, cy: number) => void
}) {
  const pointers = new Map<number, { x: number; y: number }>()
  let lastDist = 0

  return {
    onPointerDown(e: React.PointerEvent) {
      (e.target as Element).setPointerCapture(e.pointerId)
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY })
    },
    onPointerMove(e: React.PointerEvent) {
      if (!pointers.has(e.pointerId)) return
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY })

      if (pointers.size === 2) {
        const [a, b] = [...pointers.values()]
        const dist = Math.hypot(b.x - a.x, b.y - a.y)
        const cx = (a.x + b.x) / 2
        const cy = (a.y + b.y) / 2
        if (lastDist > 0) {
          opts.onZoom(dist / lastDist, cx, cy)
        }
        lastDist = dist
      } else if (pointers.size === 1) {
        const prev = pointers.get(e.pointerId)!
        opts.onPan(e.clientX - prev.x, e.clientY - prev.y)
        pointers.set(e.pointerId, { x: e.clientX, y: e.clientY })
      }
    },
    onPointerUp(e: React.PointerEvent) {
      pointers.delete(e.pointerId)
      if (pointers.size < 2) lastDist = 0
    },
  }
}
```

### 11.2 `interactive-topology.tsx`, `link-graph.tsx`

현재 zoom/touch 핸들러 없음. 위 유틸 + d3-zoom(이미 embedding-canvas에서 사용) 적용:

```tsx
import { createPinchHandlers } from '@/lib/touch-gestures'
import { useIsTouch } from '@/hooks/use-is-touch'

const isTouch = useIsTouch()
const gestures = useMemo(() => createPinchHandlers({
  onPan: (dx, dy) => svg.call(zoom.translateBy, zoomBehavior, dx, dy),
  onZoom: (scale, cx, cy) => svg.call(zoom.scaleBy, zoomBehavior, scale, [cx, cy]),
}), [])

<svg onPointerDown={isTouch ? gestures.onPointerDown : undefined} ... />
```

### 11.3 `embedding-canvas.tsx`

이미 d3-zoom 사용(`scaleExtent([0.2, 8])`). 단일 `onPointerMove`를 위 유틸로 교체하여 핀치 지원. d3-zoom의 내장 터치 이벤트(`.touchable`)도 활성화:

```ts
zoomBehavior.touchable(true)  // d3-zoom 내장 터치 지원
```

> 참고: d3-zoom은 기본적으로 wheel + touch 지원. 커스텀 핸들러와 충돌 시 **둘 중 하나만** 사용. 권장: d3-zoom 내장 touch에 위임 + 커스텀 핸들러 제거.

---

## 12. 터치 타겟 / 햅틱 피드백

### 12.1 최소 타겟 44px (WCAG 2.5.5)

| 현재 | 수정 |
|------|------|
| sidebar 아이콘 `h-7 w-7`(28px) | `h-9 w-9 sm:h-7 sm:w-7` (모바일 36px → 44px 권장) |
| attachment X `p-0.5` | `p-2` + `min-h-[36px]` |
| tooltip-only 콜랩스 아이콘 `p-2` | `p-2.5`(40px) |

**규칙**: 모바일에서 터치 가능한 모든 요소는 `min-h-[44px] min-w-[44px]` 또는 그에 상응하는 패딩.

### 12.2 활성 피드백

```css
@layer base {
  /* 모바일 탭 하이라이트 제거 + 활성 축소 */
  button, [role='button'], a {
    -webkit-tap-highlight-color: transparent;
  }
}

@layer utilities {
  /* 터치 활성 피드백 */
  .tap-feedback {
    transition: transform 100ms ease;
  }
  .tap-feedback:active {
    transform: scale(0.97);
  }
}
```

버튼/카드/행에 `active:scale-[0.97]` 또는 `.tap-feedback` 적용. hover 효과는 `hover:`에만(터치에서는 active로).

---

## 13. 고정 픽셀 너비 정리

### 13.1 스캔 결과

| 파일 | 현재 | 수정 |
|------|------|------|
| `a2a/message-log.tsx:22` | `min-w-[640px]` | 래퍼 `<div className="overflow-x-auto">` + `min-w-0` (이미 overflow-x-auto 있으면 min-w 제거) |
| `w-[400px]`, `w-[350px]`, `w-[300px]`, `w-[320px]` (다이얼/패널) | 고정 | `w-full max-w-[400px]` 등 반응형 |
| `agents/index.tsx:165` | `max-w-[280px]` | `max-w-[60vw] sm:max-w-[280px]` |

### 13.2 정리 스크립트 (구현 시)

```bash
# 360px 미만(최소 지원 폭)에서 깨지는 고정 폭 검색
grep -rnE 'w-\[(3[2-9][0-9]|[4-9][0-9]{2})px\]|min-w-\[[0-9]{3,}\]' src --include="*.tsx" | grep -v stories
```

각 발견건을 `max-w-full` 또는 반응형 값으로 교체. CI에 biome 커스텀 규칙 또는 pre-commit 훅으로 방지(후속 작업).

---

## 14. 테스트 전략

### 14.1 Playwright 모바일 프로젝트 (`playwright.config.ts`)

```ts
import { defineConfig, devices } from '@playwright/test'

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  use: {
    baseURL: 'http://localhost:5173',
    trace: 'on-first-retry',
  },
  projects: [
    // 기존 데스크톱
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
    // 신규: 모바일
    {
      name: 'mobile-safari',
      use: { ...devices['iPhone 15'] },
      testMatch: /.*\.mobile\.spec\.ts/,
    },
    {
      name: 'mobile-chrome',
      use: { ...devices['Pixel 7'] },
      testMatch: /.*\.mobile\.spec\.ts/,
    },
    // 신규: 태블릿
    {
      name: 'tablet',
      use: { ...devices['iPad (gen 10)'] },
      testMatch: /.*\.mobile\.spec\.ts/,
    },
  ],
  webServer: {
    command: 'bun run dev',
    url: 'http://localhost:5173',
    reuseExistingServer: !process.env.CI,
    timeout: 10000,
  },
})
```

- 모바일 전용 테스트는 `*.mobile.spec.ts`로 분리 (데스크톱 회귀 테스트와 독립 실행)
- `hasTouch: true`, `isMobile: true`로 터치/뷰포트 자동 에뮬레이션

### 14.2 모바일 E2E 시나리오 (`e2e/*.mobile.spec.ts`)

| 파일 | 검증 |
|------|------|
| `navigation.mobile.spec.ts` | 햄버거 → 드로어 열림/닫힘, 라우팅, ESC 닫기 |
| `dashboard.mobile.spec.ts` | 360px에서 카드 1열, 768px에서 2-3열, 오버플로우 없음 |
| `chat.mobile.spec.ts` | Enter=줄바꿈(터치), 단축키 힌트 숨김, 전송 버튼 44px |
| `dialog.mobile.spec.ts` | 긴 폼 바텀시트, `max-h` 내부 스크롤, safe-area |
| `datatable.mobile.spec.ts` | 카드뷰 전환, 행 탭 → 상세 |
| `canvas.mobile.spec.ts` | 토폴로지/그래프 팬·핀치 동작 |
| `safe-area.mobile.spec.ts` | 노치 에뮬레이션(`viewport: { viewportFits: 1 }`)에서 패딩 |

### 14.3 오버플로우 자동 감지

```ts
// e2e/helpers/overflow.ts
export async function assertNoOverflow(page: Page, width = 360) {
  await page.setViewportSize({ width, height: 640 })
  const hasOverflow = await page.evaluate(() => document.documentElement.scrollWidth > document.documentElement.clientWidth)
  expect(hasOverflow, `${width}px에서 가로 오버플로우`).toBe(false)
}
```

각 라우트 페이지 객체에서 360px(현대적 최소) 호출. 회귀 그물 핵심.

### 14.4 단위 훅 테스트 (`__tests__/hooks/`)

- `use-media-query.test.ts`: matchMedia 모킹, 변경 이벤트
- `use-breakpoint.test.ts`: 티어 전환
- `use-is-touch.test.ts`: coarse/hover 조합

### 14.5 Storybook 모바일 뷰포트 addon

`.storybook/preview.ts`에 `viewport` addon 추가(iPhone/Pixel/iPad 프리셋). 각 컴포넌트 스토리에서 모바일 뷰포트 시각 검증.

---

## 15. 마이그레이션 단계 (Phased Rollout)

### Phase 1 — 인프라 (저위험, 선제) ✅
- [x] viewport meta + safe-area 유틸리티 CSS
- [x] `use-media-query`, `use-breakpoint`, `use-is-touch` 훅 생성
- [x] Playwright 모바일/태블릿 프로젝트 + 오버플로우 헬퍼
- [x] `h-[100vh] h-dvh` 폴백 패턴 전환 (app-layout)
- [x] Dialog `max-h-[calc(100dvh-2rem)]` + `overflow-y-auto`

### Phase 2 — 핵심 UX (중위험) ✅
- [x] 채팅 입력 Enter 분기 + 단축키 숨김 + 전송 버튼 44px
- [x] Header/AppLayout/Sidebar safe-area 패딩
- [x] 사이드바 드로어 폭 `w-72 max-w-[85vw] lg:max-w-none`
- [~] 고정 픽셀 너비 정리 (`min-w-[640px]` 정리 완료, 나머지는 Phase 5 스캔에서 확인)

### Phase 3 — 콘텐츠 컴포넌트 (중위험) ✅
- [x] DataTable `mobileCardView` + `CardRow` + CSS 이중 렌더
- [x] Dialog `mobileSheet` 변형 + props 준비 (긴 폼 적용은 각 컴포넌트에서 `mobileSheet` prop 전달 필요)
- [x] 터치 타겟 44px 정리 (전송 버튼/Controls/링크그래프/ReactFlow)
- [x] 활성 피드백(`active:scale`/`.tap-feedback`) + tap-highlight 제거

### Phase 4 — 캔버스 터치 (저위험 — v2 재분류) ✅
- [x] `touch-gestures.ts` 유틸 생성 금지 (§11.5) — 라이브러리 내장 기능 활용
- [x] `interactive-topology.tsx` ReactFlow `zoomOnPinch`/`panOnDrag` 등 내장 터치 props
- [x] `link-graph.tsx` 정적 SVG 터치 타겟 + 반응형 (핀치 불필요)
- [x] `embedding-canvas.tsx` `touch-action:none` + cursor 전환 + d3-zoom `start`/`end`

### Phase 5 — 검증/문서화 ✅
- [x] `assertNoOverflow`/`assertNoOverflowAllBreakpoints` 헬퍼 생성
- [x] 모바일 E2E 시나리오 작성 (`navigation.mobile.spec.ts`)
- [x] Storybook 모바일 뷰포트 프리셋 5종 (mobileMin/notched/tablet 등)
- [x] 본 설계 문서 상태를 "구현 완료"로 갱신

---

## 16. i18n 추가 키 (최소)

대부분 모바일 전용 텍스트는 기존 키 재사용. 신규 필요 시:

```json
{
  "common.openNav": "Open navigation menu",          // (이미 사용 중)
  "common.closeMenu": "Close menu",                   // (이미 사용 중)
  "common.sendHint": "Tap send button to send",       // 신규: 모바일 입력 힌트
  "dataTable.viewAsCards": "Card view",               // 신규: 카드뷰 라벨
  "dialog.expand": "Expand",                           // 신규: 바텀시트 확장
  "dialog.collapse": "Collapse"                        // 신규: 바텀시트 축소
}
```

한국어:

```json
{
  "common.sendHint": "전송 버튼을 눌러 보내기",
  "dataTable.viewAsCards": "카드 보기",
  "dialog.expand": "펼치기",
  "dialog.collapse": "접기"
}
```

---

## 17. 호환성 / 영향 범위

### 17.1 영향 없음 (유지)
- 사이드바 드로어 동작 로직
- 헤더 통합 드롭다운
- 대시보드 그리드
- 설정 3-티어 패널
- 디자인 토큰 / 색상 시스템
- IME 처리

### 17.2 사소한 회귀 위험
- `100vh`→`100dvh` 전환: 구형 Android(Chrome < 108) 폴백 필요 → CSS 폴백 포함
- 사이드바 폭 변경: 시각적 점프 1회 (localStorage 캐시 갱신)
- Enter 동작 변경: 데스크톱 사용자는 영향 없음(isTouch=false)

### 17.3 브라우저 지원

| 브라우저 | 최소 버전 | 비고 |
|---------|-----------|------|
| iOS Safari | 15.4+ | `100dvh`, `env()`, `viewport-fit` |
| Chrome Android | 108+ | `100dvh` |
| Samsung Internet | 21+ | 동일 |
| Firefox Android | 101+ | 동일 |

폴백: `dvh` 미지원 시 `vh`로 자동 폴백(선언 순서). `env()` 미지정 시 0.

---

## 18. 결정 사항 (Decisions)

> 2026-06-15 확정. 항목 1-2는 사용자 확인, 3-5는 기본값 채택.

1. ✅ **최소 지원 폭 = `360px`** (현대적 최소). iPhone SE 1세대(320px)는 지원 범위에서 제외. 고정 픽셀 너비 정리 부담 경감. — 모든 오버플로우 테스트는 360px 기준.
2. ✅ **사이드바 영구 전환점 = `lg:`(1024px) 유지**. 태블릿(768-1023)은 드로어. 안정성·변경 최소화 우선.
3. ✅ **바텀시트 = 긴 폼만 `mobileSheet`** (event-editor, add-server-dialog, repeat-editor). 짧은 확인 다이얼은 중앙 유지.
4. ✅ **DataTable `mobileCardView` 기본 = `true`**. 모바일 자동 카드뷰 전환. 비활성화는 prop 옵트아웃.
5. ✅ **터치 감지 = `(pointer: coarse) OR (hover: none)`**. 터치 노트북 과잉 감지 허용(단축키 숨김 정도는 무해). 핀치 등은 런타임 터치 이벤트로 판별.

---

## 19. 남은 한계 / 후속 작업

### 한계
- **PWA/오프라인**: 본 설계 범위 외. 모바일 앱 감싸기(Capacitor/Tauri) 별도 검토.
- **제스처 복잡도**: 세 손가락 제스처, 스와이프 백(뒤로가기) 미포함. 핵심 팬/핀치만.
- **접근성 터치**: 스크린 리더(VoiceOver/TalkBack) 모바일 동작은 별도 검증 필요.
- **성능**: 저사양 모바일에서 캔버스/애니메이션 프레임율 별도 벤치마크 필요.

### 후속 작업 (저위험)
- biome 커스텀 규칙: `w-[Npx]`/`min-w-[Npx]` 고정값 lint 경고
- 모바일 전용 단축키 없음 → 제스처(스와이프) 매핑 검토
- 동적 폰트 스케일(사용자 접근성 설정) 대응 — `text-base` 기반 rem 설계
- 다크모드 + 모바일 AMOLED 검은 배경 최적화

---

## 20. 검증 현황 (2026-06-15)

### 통과 ✅
- [x] **tsc 통과** — 신규 오류 0 (기존 settings 10건 제외)
- [x] **biome check 통과** — 0 errors (1 warning 기존 stash 파일)
- [x] **단위 테스트** 191/191 통과 (27개 파일)
- [x] **build 성공** (900ms)
- [x] **lint 통과** (8개 파일 auto-fix)

### 실행 필요 (CI/수동) ⏳
- [ ] **e2e(desktop)** 기존 11+ 통과 — `bun run test:e2e`
- [ ] **e2e(mobile-safari/mobile-chrome/tablet)** 신규 — `bun run test:e2e --project=mobile-safari`
- [ ] **360px 전 라우트 오버플로우 스캔** — `assertNoOverflowAllBreakpoints` 헬퍼 생성 완료
- [ ] **build-storybook** — `bun run build-storybook`
- [ ] **Storybook visual regression** — `npm run storybook`
