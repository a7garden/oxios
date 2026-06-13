# Oxios Web UI/UX 전수 감사 보고서

> **감사일**: 2025-06-09
> **범위**: `surface/oxios-web/web/src/` 전체 (299개 파일)
> **기준**: 시각적 일관성, 타이포그래피, 컬러 시스템, 인터랙션, 접근성, 반응형, 레이아웃

---

## Executive Summary

Oxios Web UI는 shadcn/ui + Tailwind v4 + OKLCH 컬러 스페이스 기반의 **건전한 디자인 시스템** 위에 구축되어 있다. DESIGN.md에 명확한 원칙이 문서화되어 있고, semantic token 계층이 잘 정의되어 있다. 하지만 **실제 컴포넌트 구현에서 시스템 원칙을 벗어나는 부분**이 다수 발견된다. 특히 폰트 로딩, 애니메이션 체계, focus 관리, dark mode 대비, 레이아웃 일관성 측면에서 개선 여지가 크다.

**심각도 분류**:
- 🔴 Critical: 즉각 수정 필요 (접근성, 기능 결함)
- 🟠 Major: 사용자 경험에 직접 영향
- 🟡 Minor: 시각적 개선, polish

---

## 1. Typography

### 1.1 🔴 폰트가 로딩되지 않는다 (Critical)

**`index.html`**에 `<link>` Google Fonts나 `@font-face` 선언이 전혀 없다. `index.css`는 `'Inter', system-ui, -apple-system, sans-serif`를 지정하지만, Inter가 시스템 폰트가 아닌 환경(Windows, 일부 Linux)에서는 **system-ui로 폴백**된다. JetBrains Mono도 마찬가지.

```
현재: font-family: 'Inter', system-ui, -apple-system, sans-serif;
현실: Inter가 설치되지 않은 환경에서 → system-ui 사용 → macOS는 San Francisco, Windows는 Segoe UI
```

**영향**: OS별로 완전히 다른 타이포그래피 렌더링. 디자인 시스템의 핵심 전제가 무너짐.

**해결책**:
```html
<!-- index.html <head>에 추가 -->
<link rel="preconnect" href="https://fonts.googleapis.com" />
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet" />
```

또는 `@fontsource/inter` + `@fontsource/jetbrains-mono` npm 패키지를 번들에 포함.

### 1.2 🟠 Inter는 과도하게 범용적인 폰트 (Major)

DESIGN.md의 **"Agent Operating System — technical, professional dashboard"** 포지셔닝에 Inter는 너무 무난하다. 모든 SaaS 제품이 Inter를 사용하는 2025년, Oxios는 **기술적 권위**와 **제어실(control room)** 미학을 표방하면서도 가장 흔한 폰트를 사용하고 있다.

**대안 제안** (디자인 톤에 맞는 선택):
| 폰트 | 특성 | 적합성 |
|------|------|--------|
| **Geist** (Vercel) | 기하학적, 모노톤, 기술적 | ⭐⭐⭐⭐⭐ |
| **IBM Plex Sans** | 엔지니어링, 신뢰감 | ⭐⭐⭐⭐ |
| **Source Sans 3** | Adobe, 전문적 | ⭐⭐⭐⭐ |
| **Outfit** | 모던, 기술적, 가독성 | ⭐⭐⭐ |

모노스페이스도 JetBrains Mono는 좋은 선택이지만, **Geist Mono**로 통일하면 Geist Sans와 조화가 완벽하다.

### 1.3 🟡 typographic hierarchy가 실제 컴포넌트에서 일관되지 않는다 (Minor)

DESIGN.md는 H1(text-2xl, 600) ~ Display(text-4xl, 700) 체계를 정의하지만, 실제 코드에서:
- 대시보드: `<h1 className="text-2xl font-bold">` — bold(700)를 사용. DESIGN.md는 600을 지정.
- Settings 페이지: 섹션 제목이 `<CardTitle className="text-base">` — Design System의 Heading 4(18px)가 아닌 16px.
- Empty state: `<h3 className="text-lg font-semibold">` — text-lg는 18px인데 DESIGN.md에서는 Heading 4로 분류.

**권장**: DESIGN.md와 실제 구현 간 font-weight 합의가 필요. 대시보드 타이틀은 `font-bold`도 괜찮으나, DESIGN.md를 업데이트하거나 코드를 맞춰야 함.

---

## 2. Color & Theme

### 2.1 🔴 Sparkline 색상이 OKLCH 토큰 대신 raw RGB를 사용한다 (Critical)

`stat-card.tsx`의 `COLOR_MAP`:
```typescript
info: { stroke: 'rgb(59 130 246)', fill: 'rgb(59 130 246 / 0.18)' }
success: { stroke: 'rgb(16 185 129)', fill: 'rgb(16 185 129 / 0.18)' }
```

이것은 DESIGN.md의 **"Don't hardcode color values"** 원칙을 직접 위반한다. Recharts는 CSS 변수를 지원하므로:
```typescript
info: { stroke: 'var(--color-info)', fill: 'oklch(0.623 0.214 259.815 / 0.18)' }
```
또는 `getComputedStyle`로 런타임에 CSS 변수를 읽어 RGB로 변환하는 유틸을 만들어야 한다.

`agent-status-card.tsx`도 동일한 문제: `stroke="rgb(16 185 129)"`.

### 2.2 🟠 Dark mode에서 Card elevation이 평면화된다 (Major)

Light mode: `--card: oklch(1 0 0)` vs `--background: oklch(1 0 0)` — Card와 Background가 **동일한 색상**.
Dark mode: `--card: oklch(0.178 0.008 285.89)` vs `--background: oklch(0.141 0.005 285.823)` — 미세한 차이.

Card에 `shadow` 클래스가 있지만, 동일한 밝은 색상 위에서는 그림자로만 elevation을 구분해야 한다. dark mode에서는 L* 차이가 0.178 - 0.141 = 0.037로 **지각적으로 거의 구분되지 않는다**.

**권장**:
- Light: card를 `oklch(0.985 0 0)`으로 낮춰서 background와 구분 (현재는 둘 다 pure white)
- Dark: card를 `oklch(0.20 0.008 285.89)`로 올려서 차이를 극대화
- 또는 card에 항상 `border` + `shadow-sm` 조합을 강제

### 2.3 🟠 Primary 컬러가 Zinc (거의 검정/흰색)이어서 상호작용 피드백이 약하다 (Major)

Light: `--primary: oklch(0.21 0.006 285.885)` = 거의 검정
Dark: `--primary: oklch(0.985 0 0)` = 순백

이는 "quiet confidence" 철학에 맞지만, 실제 사용에서:
- Primary 버튼이 검정 배경에 흰 텍스트 → **CTA로서 시각적 강도가 낮음**
- Agent 상태 색상(success=emerald, warning=amber, error=red)이 더 강렬해서 primary가 묻힘
- Focus ring `--ring: oklch(0.708 0 0)`은 gray로, 컬러리스한 상태에서 시각적 구분이 어려움

**권장**: Primary에 미세한 chroma를 추가하여 "유색 뉴트럴"로 만들기. 예:
```css
--primary: oklch(0.23 0.02 260);  /* 미세한 blue tint — 기술적 느낌 */
```
이렇게 하면 zinc 뉴트럴을 유지하면서도 primary 액션에 미세한 아이덴티티가 생긴다.

### 2.4 🟡 STATUS_PALETTE의 hex 값이 OKLCH 토큰과 동기화되지 않는다 (Minor)

`status-palette.ts`에서 `hex: '#10b981'` (Tailwind emerald-500 RGB)를 사용하지만, `index.css`의 `--success: oklch(0.596 0.145 163)`는 다른 값을 가리킨다. React Flow minimap 등에서 hex가 필요한 경우, 이 두 값이 시각적으로 다를 수 있다.

**권장**: 주석으로 동기화 상태를 명시하거나, 런타임에 CSS 변수에서 변환하는 유틸 도입.

---

## 3. Layout & Spatial Composition

### 3.1 🟠 Sidebar collapsed 상태에서 너비가 64px로 비효율적이다 (Major)

```typescript
collapsed ? 'w-16' : 'w-60'
```

`w-16` = 64px. 아이콘(16px) + 패딩을 고려하면 충분하지만, collapsed 상태에서는 Tooltip으로 텍스트를 보여줘야 해서 UX가 번거롭다. 더 중요한 문제는 **collapsed 상태가 디폴트가 아니라는 것** — 사용자가 240px sidebar를 계속 봐야 한다.

**권장**:
- 기본 너비를 `w-56`(224px)로 줄이거나, 3단 너비(slim 48px / normal 224px / expanded 280px) 지원
- Collapsed 아이콘에 subtle label 텍스트를 아이콘 아래에 10px로 표시 (macOS Dock 스타일)

### 3.2 🟡 Dashboard 그리드 간격이 너무 촘촘하다 (Minor)

```tsx
<div className="grid gap-2 grid-cols-2 sm:grid-cols-3 xl:grid-cols-6">
```

`gap-2` = 8px. 6개의 KPI 카드가 8px 간격으로 나열되면 **시각적 밀도가 너무 높다**. 정보 밀도가 중요하다는 DESIGN.md의 원칙은 이해하지만, 카드 사이의 구분이 어려워진다.

**권장**: `gap-3`(12px)로 증가. 카드 내부 패딩과 외부 갭의 비율을 조정하여 "밀도 있되 정돈된" 느낌 유지.

### 3.3 🟡 Chat 페이지 높이 계산이 부정확하다 (Minor)

```tsx
<div className="flex h-[calc(100vh-8rem)]">
```

`8rem` = 128px. Header가 `h-14`(56px)이므로, 나머지 72px은 어디로 가는지 불명확. Chat input 높이가 동적(auto-grow)이므로 고정 높이 계산은 항상 약간 어긋난다.

**권장**: `flex h-full`을 사용하고 부모 `main`에서 `flex-1 min-h-0`을 적용하여 브라우저가 계산하게 하기. 현재 AppLayout의 chat 브랜치도 이미 `flex-1 min-h-0 overflow-hidden`을 사용 중이므로, ChatPage 내부에서 다시 `calc`로 계산할 필요 없음.

### 3.4 🟡 Settings 페이지 좌측 네비게이션이 스크롤을 고려하지 않는다 (Minor)

```tsx
<nav className="hidden lg:block w-52 shrink-0">
  <div className="sticky top-0 space-y-1">
```

Settings 섹션이 많아지면 `w-52` 영역이 overflow 없이 화면을 벗어날 수 있다. `sticky top-0`은 스크롤 컨텍스트 내에서만 동작.

**권장**: `max-h-[calc(100vh-4rem)] overflow-y-auto` 추가.

---

## 4. Motion & Animation

### 4.1 🟠 애니메이션이 거의 없다 — "정지된 제어판" 느낌 (Major)

전체 코드베이스에서 인위적인 애니메이션:
- `typing-bounce` 키프레임 (채팅 인디케이터)
- `animate-pulse` (상태 도트)
- `animate-spin` (로딩 스피너)
- shadcn/ui 기본 `animate-in/fade-in/zoom-in` (다이얼로그, 드롭다운)

이게 전부다. **페이지 전환, 카드 등장, 리스트 렌더링, 상태 변화**에 애니메이션이 없다.

**영향**: 데이터가 갱신될 때 카드가 "뚝" 바뀜. 리스트가 나타날 때 "툭" 뜸. "Agent OS 제어실"은 실시간 변화를 부드럽게 보여줘야 한다.

**권장** (우선순위 순):
1. **Dashboard 카드 stagger 등장**: CSS `@keyframes fadeInUp` + `animation-delay` per card
2. **StatCard 숫자 변화**: CountUp 애니메이션 (react-countup 또는 CSS `@property`)
3. **리스트 아이템**: `animate-in fade-in slide-in-from-top-2` on mount (tailwind-animate 유틸)
4. **페이지 전환**: TanStack Router `beforeLoad`/after load 훅으로 opacity transition
5. **상태 변화**: success/error 배지에 scale bounce

```css
/* 제안: index.css에 추가 */
@keyframes fadeInUp {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}
@keyframes countUp {
  from { opacity: 0; transform: translateY(4px); }
  to { opacity: 1; transform: translateY(0); }
}
```

### 4.2 🟡 Transition이 `transition-colors`로만 제한된다 (Minor)

대부분의 컴포넌트가 `transition-colors`만 사용. 배경색만 부드럽게 변하고, 크기/위치/투명도 변화는 "뚝"이다.

**권장**: 인터랙티브 요소에 `transition-all duration-200` 또는 `transition-[color,background,box-shadow,transform]` 적용.

---

## 5. Interaction Design

### 5.1 🟠 Focus management가 불충분하다 (Major)

**문제 1: Header icon 버튼에 focus 스타일이 없다**
```tsx
<button className="inline-flex items-center justify-center rounded-md p-2 text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors">
```
`focus-visible:ring`이 없음. 키보드 네비게이션 시 포커스 위치를 알 수 없음.

**문제 2: Sidebar nav 항목에 focus 스타일이 없다**
```tsx
<Link className={cn(itemBase, isActive ? itemActive : itemInactive)}>
```
focus-visible에 대한 처리가 없음.

**문제 3: DataTable 행에 focus 스타일이 없다**
```tsx
<tr className={cn('border-b last:border-0 transition-colors', onRowClick && 'cursor-pointer hover:bg-muted/50')}>
```
클릭 가능한 행이지만 focus-visible이 없음.

**해결책**: `itemBase`, `NavItemLink`, DataTable 행 등 모든 인터랙티브 요소에 `focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring rounded-md` 추가.

### 5.2 🟠 Keyboard shortcut 표시가 일관되지 않는다 (Major)

Knowledge sidebar: `<span className="text-2xs ...">⌘K</span>`
Chat input: `<kbd className="rounded border bg-muted/50 px-1 py-0.5 text-2xs font-mono">@</kbd>`

두 가지 서로 다른 단축키 표시 스타일이 공존. 하나는 plain `<span>`, 다른 하나는 `<kbd>`.

**권장**: `<kbd>` 태그로 통일하고, 공통 스타일 유틸 생성:
```typescript
export const kbdStyle = 'rounded border bg-muted/50 px-1.5 py-0.5 text-2xs font-mono text-muted-foreground'
```

### 5.3 🟡 Row 클릭 시 cursor가 pointer이지만 hover 효과가 미미하다 (Minor)

DataTable 행: `cursor-pointer hover:bg-muted/50` — `muted/50`은 50% opacity의 muted 색상으로, hover 시 매우 미세한 변화만 보임.

Dashboard 에이전트 카드: `hover:bg-accent/40` — 40% opacity.

**권장**: `hover:bg-muted`(100%) 또는 `hover:bg-accent`(100%)로 변경. 대시보드의 경우 카드 자체에 `hover:shadow-md hover:-translate-y-px`를 추가하여 물리적 리프트 효과 부여.

### 5.4 🟡 Chat input의 textarea가 focus 시 시각적 변화가 약하다 (Minor)

```tsx
'focus-within:shadow-md focus-within:border-primary/20'
```

`border-primary/20`은 20% opacity로 거의 보이지 않음. `focus-within:border-primary/40` 또는 `focus-within:ring-1 focus-within:ring-ring`으로 변경 권장.

---

## 6. Component-Level Issues

### 6.1 🟠 Empty state가 지나치게 단조롭다 (Major)

```tsx
// EmptyChatState
<Bot className="h-12 w-12 mx-auto mb-3 text-primary/60" />
<p className="text-base font-medium text-foreground">
  {t('chat.greeting')}
</p>
```

빈 상태가 **아이콘 + 텍스트 한 줄**이 전부. Agent OS의 첫인상이 되는 채팅 빈 화면이 너무 밋밋하다.

**권장**: 빈 상태에 제안 프롬프트(chips) 추가:
```tsx
<div className="grid grid-cols-2 gap-2 mt-4 max-w-md">
  {['Summarize recent agent activity', 'Check memory for...', 'Create a new seed for...'].map(q => (
    <button className="text-left text-sm p-3 rounded-lg border hover:bg-muted/50">
      {q}
    </button>
  ))}
</div>
```

### 6.2 🟠 Loading skeleton이 동적 데이터를 반영하지 않는다 (Major)

```tsx
// LoadingCards
<div className="rounded-xl border p-6">
  <Skeleton className="h-4 w-1/3 mb-4" />
  <Skeleton className="h-3 w-2/3 mb-2" />
  <Skeleton className="h-3 w-1/2" />
</div>
```

모든 로딩 상태가 **동일한 3줄 스켈레톤**. DataTable 로딩, 카드 그리드 로딩, KPI 로딩이 모두 같은 패턴.

**권장**: 컴포넌트별 맞춤 스켈레톱:
- StatCard: 큰 숫자 + 작은 라벨 모양
- DataTable: 헤더 + 3~5개 행 모양
- Card grid: 카드 모양 스켈레톤

### 6.3 🟡 Notification 패널이 portal을 사용하지 않는다 (Minor)

```tsx
// NotificationBell
<div className="absolute right-0 top-full mt-2 z-50 w-80 ...">
```

`position: absolute`로 렌더링되어, 화면 오른쪽에 가까울 때 패널이 화면을 벗어날 수 있음. Radix Popover 또는 Portal을 사용하여 해결.

### 6.4 🟡 Badge variant가 error를 지원하지 않는다 (Minor)

```typescript
// badge.tsx variants
default | secondary | destructive | outline | success | warning
```

`error` variant가 없고 `destructive`만 존재. `STATUS_PALETTE`는 `error` 상태를 가지지만 Badge에서는 `destructive`를 사용해야 함. 명명 일관성 문제.

**권장**: `error` variant를 추가하거나, `destructive`를 `error`로 리네임.

### 6.5 🟡 ToolCallCard의 확장/축소에 화살표 아이콘이 불명확하다 (Minor)

ChevronRight/ChevronDown을 사용하지만, Wrench 아이콘 옆에 있어서 "도구 이름"인지 "확장 버튼"인지 혼동. 전체 행이 버튼이지만 시각적으로 버튼처럼 보이지 않음.

**권장**: 전체 행에 `hover:bg-muted/50 rounded-md` 추가, 또는 chevron을 좌측 끝으로 이동.

---

## 7. Responsive Design

### 7.1 🟠 모바일 대응이 최소한이다 (Major)

- Sidebar: `hidden lg:flex` — 모바일에서 오버레이로 전환되지만, **오버레이에 애니메이션이 없다** (`translate-x` transition 없이 바로 나타남)
- Dashboard KPI 그리드: `grid-cols-2 sm:grid-cols-3 xl:grid-cols-6` — 모바일 2열은 좋으나, 카드 내용이 너무 많이 잘림
- DataTable: `overflow-x-auto`로 수평 스크롤 — 모바일에서 테이블 UX의 최후보루
- Settings: 모바일 드롭다운으로 전환 — 좋은 패턴이나, 드롭다운에 애니메이션 없음

**권장 (우선순위)**:
1. Sidebar 오버레이에 `transform transition-transform duration-300` 추가
2. 모바일 KPI 카드에서 sparkline 숨기기 (`hidden sm:block`)
3. DataTable 모바일에서 카드 뷰로 전환하는 옵션 고려

### 7.2 🟡 Mobile header가 hamburger 버튼 외에 빈 공간이다 (Minor)

```tsx
<button className="lg:hidden" onClick={() => setMobileOpen(true)}>
  <Menu className="h-5 w-5" />
</button>
```

모바일에서 ModeTabs가 숨겨지고, 설정이 dropdown으로 통합됨. 하지만 **페이지 타이틀이 없어서** 사용자가 현재 위치를 모름.

**권장**: 모바일 header에 현재 페이지 이름 표시. 또는 ModeTabs를 모바일에서도 보이게 하되 더 작은 버전으로.

---

## 8. Accessibility

### 8.1 🔴 `aria-label`이 한국어 번역과 충돌한다 (Critical)

```tsx
<Bot className="h-4 w-4" />
// Bot 아이콘에 대체 텍스트가 없음
```

여러 아이콘이 `aria-hidden="true"` 없이 렌더링되어 스크린 리더가 의미 없는 요소를 읽을 수 있음. 반면 일부 버튼은 `aria-label={t('...')}`로 i18n 적용이 되어 있어 불일치.

### 8.2 🟠 Color-only status indicators (Major)

```tsx
<div className={cn('h-2 w-2 rounded-full', statusDot(status))} />
<span className="text-sm capitalize">{status}</span>
```

상태 점(dot)이 색상만으로 의미를 전달. 적록색약 사용자에게 success(green)와 error(red)가 구분되지 않을 수 있음. 텍스트 라벨이 옆에 있지만, badge 시스템 등에서 dot-only로 사용되는 경우가 있음.

**권장**: 점 외에 형태(shape)나 패턴 추가. 또는 APCA contrast 기준 Lc ≥ 60 검증.

### 8.3 🟡 Sidebar overlay에 `role="dialog"`가 있지만 focus trap이 없다 (Minor)

```tsx
<div role="dialog" aria-label={t('common.closeMenu')} className="fixed inset-0 z-40 bg-black/50">
```

`role="dialog"`를 사용하지만, Radix Dialog나 `focus-trap` 라이브러리를 사용하지 않아 tab이 모달 밖으로 빠져나감.

---

## 9. Dark Mode Specific

### 9.1 🟠 Dark mode에서 shadow가 보이지 않는다 (Major)

```css
--shadows.sm: '0 1px 3px oklch(0 0 0 / 0.06)';
```

dark 배경에서 black shadow는 당연히 보이지 않음. Card elevation을 shadow에 의존하면 dark mode에서 평면화됨.

**권장**:
- Dark mode에서 shadow 색상을 `oklch(0 0 0 / 0.3)` 이상으로 증가
- 또는 Card에 `ring-1 ring-white/5` 추가하여 경계로 elevation 표현

### 9.2 🟡 Dark mode에서 placeholder 텍스트 대비가 낮다 (Minor)

```css
'placeholder:text-muted-foreground/60'
```

`muted-foreground`가 dark에서 `oklch(0.705 0.015 286.067)`인데, 60% opacity면 `oklch(0.42 ...)` 수준으로 떨어져서 입력 필드 placeholder가 거의 안 보임.

---

## 10. Code Quality (UI-related)

### 10.1 🟡 Inline `<style>` 태그가 컴포넌트에 있다 (Minor)

`typing-indicator.tsx`:
```tsx
<style>{`
  @keyframes typing-bounce { ... }
`}</style>
```

컴포넌트당 하나의 `<style>` 태그. 이 패턴이 반복되면 CSS 중복 문제가 발생할 수 있음.

**권장**: `index.css`의 `@layer utilities`로 이동.

### 10.2 🟡 Time formatting이 하드코딩되어 있다 (Minor)

```typescript
// notification-bell.tsx, message-bubble.tsx
function timeAgo(iso: string): string {
  // 영어로 하드코딩: 'just now', '5m ago', '2h ago'
}
```

`useTranslation()`이 같은 파일에 있지만 time formatting에 i18n이 적용되지 않음. 한국어 환경에서 "just now", "5m ago"가 그대로 표시됨.

**권장**: `t('common.justNow')`, `t('common.minutesAgo', { count })` 등으로 번역 키 사용.

---

## Summary: Priority Matrix

| # | Category | Issue | Severity | Effort |
|---|----------|-------|----------|--------|
| 1.1 | Typography | 폰트 미로딩 | 🔴 Critical | S |
| 2.1 | Color | Sparkline raw RGB | 🔴 Critical | M |
| 8.1 | A11y | aria-label/i18n 불일치 | 🔴 Critical | M |
| 9.1 | Dark Mode | Shadow 가시성 | 🟠 Major | S |
| 5.1 | Interaction | Focus management | 🟠 Major | M |
| 2.2 | Color | Card elevation 평면화 | 🟠 Major | S |
| 4.1 | Motion | 애니메이션 부재 | 🟠 Major | M |
| 2.3 | Color | Primary 컬러 약함 | 🟠 Major | S |
| 7.1 | Responsive | 모바일 대응 최소화 | 🟠 Major | L |
| 6.1 | Components | Empty state 단조 | 🟠 Major | S |
| 6.2 | Components | Loading skeleton 단일 | 🟠 Major | M |
| 1.2 | Typography | Inter 범용성 | 🟠 Major | M |
| 5.2 | Interaction | 단축키 표시 불일치 | 🟠 Major | S |
| 3.1 | Layout | Sidebar 너비 | 🟡 Minor | S |
| 5.3 | Interaction | Hover 효과 미미 | 🟡 Minor | S |
| 7.2 | Responsive | 모바일 페이지 타이틀 | 🟡 Minor | S |
| 8.2 | A11y | Color-only status | 🟡 Minor | M |
| 6.4 | Components | Badge error variant | 🟡 Minor | XS |
| 10.2 | Code | Time formatting i18n | 🟡 Minor | S |
| 10.1 | Code | Inline style 태그 | 🟡 Minor | XS |

---

## Recommended Implementation Order

### Phase 1: Quick Wins (1-2일)
1. **폰트 로딩** (#1.1) — `<link>` 추가만으로 해결
2. **Focus-visible 스타일** (#5.1) — `itemBase` 등 프리미티브에 추가
3. **Dark mode shadow** (#9.1) — CSS 변수 값 수정
4. **Card elevation** (#2.2) — CSS 변수 값 수정
5. **Time i18n** (#10.2) — 번역 키 연결

### Phase 2: Core Polish (3-5일)
1. **Sparkline 토큰 통합** (#2.1) — CSS 변수 → Recharts 브릿지
2. **애니메이션 기초** (#4.1) — fadeInUp 키프레임 + stagger
3. **Empty state 개선** (#6.1) — 제안 칩 추가
4. **Loading skeleton 다양화** (#6.2) — 컴포넌트별 스켈레톤
5. **단축키 표시 통일** (#5.2) — `<kbd>` + 공통 스타일

### Phase 3: Strategic (1-2주)
1. **폰트 교체** (#1.2) — Geist 또는 IBM Plex Sans
2. **Primary 컬러 조정** (#2.3) — 미세한 chroma 추가
3. **모바일 대응 강화** (#7.1) — sidebar animation, 반응형 카드
4. **Sidebar UX 개선** (#3.1) — 3단 너비 또는 아이콘+라벨

---

*이 보고서는 코드 정적 분석 기반으로 작성되었습니다. 브라우저에서 실제 렌더링된 상태를 검수하면 추가적인 시각적 이슈가 발견될 수 있습니다.*
