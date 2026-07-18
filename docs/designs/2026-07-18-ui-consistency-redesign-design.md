# Oxios Web UI 일관성 재설계 — 세부 설계

> 2026-07-18 · 감사 보고서(`oxios-ui-consistency-audit.html`)의 발견을 구현 가능한 엔지니어링 설계로 전개.
> 대상: `web/src/` (React + TanStack Router + Tailwind v4 + OKLCH 토큰).

---

## 0. 보고서 리뷰 (정기 평가)

감사는 근거 기반(`file:line`)이며 전 모드를 커버. 강점과 한계를 아래에 정리.

### 강점
- **전 모드 커버리지**: Console 17탭 + Chat + Knowledge + 크로스모드 셸. 초기 누락(Chat/Knowledge)을 보강 완료.
- **정량적 증거**: 헤더 패턴을 grep으로 전 탭 집계(제목 굵기·정렬·부제 크기·앞아이콘·액션갭 5축)하여 "느낌"이 아닌 횟수로 입증.
- **루브릭 일관성**: 8차원(C1–C8)을 전 화면에 동일 적용 → 화면 간 비교 가능.
- **긍정 신호 수록**: 공유 상태 컴포넌트·시맨틱 토큰·`itemBase` 프리미티브를 모범으로 명시하여 "무조건 비판" 회피.

### 한계 / 정정 필요
1. **P3 과소집계**: 보고서는 "`mr-1` vs `gap` 2원"으로 기술했으나, Button 베이스가 `gap-2`를 내장(`button.tsx:7`)임을 확인한 결과 실제로는 **3원 분할**이다.
   - `mr-1/mr-2` + 내장 gap-2 → **~12px** (mcp·personas·cron·projects·workspace·git·email·mounts·chat·agents 등 ~20곳)
   - 내장 gap-2 단독 → **8px** (token-maxing·skills import·skill-editor 등)
   - `className="gap-1.5"` 재정의 → **6px** (skills 토글/편집/삭제·skill-content·quick-ask·settings-shell)
   → 정규형은 "margin 전면 폐지, Button 내장 gap-2에 의존"으로 단일화(8px). 밀집 버튼은 명시 `gap-1.5`로 축소 허용(예외 규칙 명시).
2. **P5 탭 아이콘 판단 재검토**: 보고서는 "무아이콘이 다수이므로 정규=무아이콘"으로 권고했으나, email.tsx(`:208-220`)는 4개 트리거 모두 아이콘을 쓰고 있어 "다수 무아이콘"이 약함. 재조사 필요 — email(아이콘 O)·mcp(아이콘 O) vs memory·budget·skills(무아이콘). 실제로는 **2:3으로 무아이콘이 다수**긴 하나, email+mcp가 주요 인프라 탭이라 단순 "무아이콘 통일"이 비직관적일 수 있음. → 본 설계는 **"핵심 탭은 무아이콘, 단일 페이지 내 섹션 구분이 필요한 트리거는 아이콘 허용"**으로 완화(아래 §3.5).
3. **`[&_svg]:size-4` 함의 누락**: Button이 모든 자식 svg를 16px로 강제(`button.tsx:7`). 따라서 버튼 내 `h-3 w-3`/`h-3.5 w-3.5`는 **사실상 no-op**(특이도에서 짐). 보고서는 이 함의를 다루지 않음. → 정규형: 버튼 내 아이콘은 `className` 생략 또는 `h-4 w-4`만(16px 강제에 맞춤). `sm` 버튼에서 더 작은 아이콘을 원하면 Button 변형 필요(별건, 본 설계 범위 외).
4. **목업 정확도**: Memory 현재 패널의 raw hex(`#ef4444` 등)는 실제 코드 버그의 충실 재현(제안 패널이 토큰으로 해소) — 정상 작동. 단, 일부 compact 섹션은 단일 핀 패널만으로 제안 시각이 약함 → 본 설계가 제안형을 코드로 제공하여 보완.
5. **인덱스 비클릭**: 리포트 인덱스 행이 앵커 링크가 없음(스크롤 전용). 미해결 — 차기 리포트 개선항.

> 결론: 보고서의 방향과 상위 패턴(P1·P2·P4)은 건강. P3는 설계 단계에서 정정(3원→1원). P5는 완화. 본 문서가 그 정정을 반영한다.

---

## 1. 정규형(Canonical) 규칙 — 코드 수준

### 1.1 PageHeader (P1 해결)
모든 Console 탭 헤더를 단일 컴포넌트로. 변형 분산을 원천 차단.

**API**:
```tsx
interface PageHeaderProps {
  title: string
  subtitle?: string
  /** 우측 액션 슬롯 (RefreshButton, 생성 Button 등). gap-2로 정렬. */
  actions?: React.ReactNode
  /** 제목 옆 메타(예: RFC 뱃지, 카운트). 드물게 사용. */
  titleMeta?: React.ReactNode
  className?: string
}
```

**렌더링(정규 클래스)**:
```tsx
<div className={cn('flex items-center justify-between gap-4', className)}>
  <div className="min-w-0">
    <h1 className="text-2xl font-bold truncate">{title}</h1>
    {subtitle && <p className="text-sm text-muted-foreground mt-0.5">{subtitle}</p>}
    {titleMeta}
  </div>
  {actions && <div className="flex items-center gap-2 shrink-0">{actions}</div>}
</div>
```

규칙: `h1 = text-2xl font-bold` (Dashboard의 `font-semibold tracking-tight` 제거 통일) · 부제 = **`text-sm` 명시** · 정렬 = `items-center` · 페이지 루트 = `space-y-6`.

### 1.2 간격 리듬 (P2 해결)
페이지 루트 컨테이너 = **`space-y-6`** 단일. 현재 `space-y-4` 5면(Dashboard·Agents·Mounts·Projects index·ProjectDetail)을 `space-y-6`으로 승격. 카드 그리드 내부는 기존대로 `gap-3`(KPI)/`gap-4`(일반) 유지.

### 1.3 버튼 아이콘 간격 (P3 해결 — 정정)
Button 베이스가 이미 `gap-2`(`button.tsx:7`). 그러므로:
- **정규**: 버튼 자식 아이콘에서 `mr-1`/`mr-2`/`ml-1` **전면 제거**. 간격은 Button 내장 `gap-2`(8px)에 의존.
- **예외(밀집)**: 토글/편집/삭제 같은 sm 행 버튼은 명시 `className="gap-1.5"`(6px) 허용 — 단 한 화면 내 일관.
- 버튼 내 아이콘 `className`은 원칙 생략(Button이 `[&_svg]:size-4`로 16px 강제). 명시는 `h-4 w-4`만.

### 1.4 사이드바 아이콘 맵 (P4 해결)
| 항목 | 현재 | 정정 | 근거 |
|---|---|---|---|
| Skills | `Zap` | `Sparkles` | MCP와의 중복 해소; 앱에서 AI=Sparkles 관례(model-picker·activity-card·mounts auto) |
| Security | `Bell` | `ShieldCheck` | Bell은 알림 관례이고 이미 4곳(approvals·notification-center·notification-section·section-icons)이 "알림"으로 사용 → 보안은 ShieldCheck가 정합 |
| (logo) | `Zap` | 유지 | 브랜드 마크 |

### 1.5 탭 패턴 (P5 — 완화)
- **핵심 다중 탭 페이지**(MCP·Memory·Budget·Skills): 트리거는 **무아이콘**으로 통일(MCP 이니셜 4개 제거; Memory/Budget/Skills는 이미 무아이콘).
- **섹션 구분이 명목적인 단일 페이지의 보조 탭**(예: email overview/setup/history/templates)은 아이콘 **허용**(단, Email이 핵심 다중 탭으로 간주되면 무아이콘으로). → 실사 시 email을 핵심으로 분류하여 **무아이콘 통일** 권장(일관성 우선).
- 탭 콘텐츠: 외부 `<Card>` 래핑 **금지**(MCP 제거). 탭 스트립이 그룹핑 제공.

---

## 2. PageHeader 컴포넌트 코드

`web/src/components/shared/page-header.tsx`:

```tsx
import { cn } from '@/lib/utils'

interface PageHeaderProps {
  title: string
  subtitle?: string
  actions?: React.ReactNode
  titleMeta?: React.ReactNode
  className?: string
}

/**
 * PageHeader — Console 탭 헤더의 단일 진실 원천.
 *
 * 감사(2026-07-18) P1: 제목 굵기·정렬·부제 크기·앞아이콘·액션갭 5축 분화를
 * 이 컴포넌트로 흡수. 모든 Console 탭은 <PageHeader>를 사용한다.
 *
 * 정규형: h1 text-2xl font-bold · 부제 text-sm text-muted-foreground ·
 * items-center · actions gap-2. (Chat/Knowledge는 의도적 미사용.)
 */
export function PageHeader({ title, subtitle, actions, titleMeta, className }: PageHeaderProps) {
  return (
    <div className={cn('flex items-center justify-between gap-4', className)}>
      <div className="min-w-0">
        <div className="flex items-center gap-2 min-w-0">
          <h1 className="text-2xl font-bold truncate">{title}</h1>
          {titleMeta}
        </div>
        {subtitle && <p className="text-sm text-muted-foreground mt-0.5 truncate">{subtitle}</p>}
      </div>
      {actions && <div className="flex items-center gap-2 shrink-0">{actions}</div>}
    </div>
  )
}
```

---

## 3. 화면별 마이그레이션 매트릭스

`space-y-6` 루트 + `<PageHeader>` 도입 + P3 margin 제거를 한 번에.

| # | 화면 | 파일 | PageHeader | space-y | P3 margin | 비고 |
|---|---|---|---|---|---|---|
| 01 | Dashboard | routes/index.tsx | ✅ 도입 | 4→6 | — | 제목 `font-semibold tracking-tight`→`font-bold`; `items-end`→PageHeader; 버전 뱃지는 titleMeta 또는 actions |
| 02 | Agents | routes/agents/index.tsx | ✅ | 4→6 | — | 제목 앞 `Bot` 아이콘 → titleMeta 또는 제거(정규=무아이콘) |
| 03 | Personas | routes/personas.tsx | ✅ | 6 | Plus mr-1 제거 | 3줄 헤더 → 2줄(hint는 subtitle에 통합 또는 본문) |
| 04 | Skills | routes/skills.tsx | ✅ | 6 | — | 제목 앞 `Zap` 제거 |
| 05 | Projects | routes/projects/index.tsx | ✅ | 4→6 | Plus mr-1 제거(×2) | |
| 05d | ProjectDetail | routes/projects/$projectId.tsx | (부분) | 4→6 | Edit/Trash mr-1 제거 | emoji+제목은 커스텀 유지, 액션만 정규화 |
| 06 | Mounts | routes/mounts/index.tsx | ✅ | 4→6 | FolderPlus mr-2 제거(×2) | |
| 07 | Memory | routes/memory.tsx | ✅ | 6 | — | |
| 08 | Workspace | routes/workspace/index.tsx | ✅ | 6 | Plus/Upload mr-1 제거 | |
| 09 | Cron Jobs | routes/cron-jobs.tsx | ✅ | 6 | Plus mr-1 제거 | 액션 gap-3→PageHeader의 gap-2 |
| 10 | Cost | routes/budget.tsx | ✅ | 6 | — | |
| 11 | Token Maxing | routes/token-maxing.tsx | ✅ | 6 | — | RFC 뱃지 → titleMeta |
| 12 | MCP | routes/mcp.tsx | ✅(참조) | 6 | Plus mr-1 제거 | 탭 아이콘 제거(P5); Card 래핑 제거(P5) |
| 13 | Email | routes/email.tsx | ✅ | 6 | 탭/버튼 margin 다수 제거 | 탭 아이콘 제거(P5) 검토 |
| 14 | Git | routes/git.tsx | ✅ | 6 | ShieldCheck mr-1 제거 | |
| 15 | Resources | routes/resources.tsx | ✅ | 6 | — | |
| 16 | Security | routes/security.tsx | ✅ | 6 | — | (사이드바 아이콘은 P4에서 ShieldCheck) |
| 17 | Settings | routes/settings.tsx | (SettingsShell 유지) | — | — | 패러다임 단절은 의도적; 경계 명시만 검토 |
| 18 | Chat | routes/chat.tsx | 미사용(의도) | — | RefreshCw mr-1 제거 | EmptyChatState→공유 EmptyState는 별건 |
| 19 | Knowledge | components/knowledge/* | (브레드크럼 유지) | — | — | Home h1 리듬 정규화만 |
| 셸 | sidebar | components/layout/sidebar.tsx | — | — | — | P4 아이콘: Zap→Sparkles(Skills), Bell→ShieldCheck(Security) |

---

## 4. 순서 & 위험

**Phase A — 기반(본 에이전트 직접)**:
1. `shared/page-header.tsx` 생성.
2. P4 사이드바 아이콘 정정(sidebar.tsx 1파일, 2 아이콘 + import).
3. MCP(routes/mcp.tsx)를 PageHeader 참조 마이그레이션으로 완료 + 탭 아이콘·Card 래핑 제거 + Plus margin 제거. **빌드 검증**.

**Phase B — 병렬 마이그레이션(서브에이전트 배치)**: Phase A가 green이면 잔여 화면을 3배치로 분산(A: Dashboard/Agents/Personas/Skills, B: Projects·detail/Mounts/Workspace/Cron/Cost, C: TokenMaxing/Email/Git/Resources/Security). 각 배치: PageHeader 도입 + space-y-6 + margin 제거. Dashboard는 제목 굵기/정렬 편차 포함.

**위험**:
- PageHeader 도입 시 `actions` 슬롯으로 기존 조건부 렌더링(예: MCP의 `tab==='servers'` 조건부 버튼)을 그대로 전달 — 동작 보존.
- Dashboard 버전 뱌지, TokenMaxing RFC 뱃지, Personas hint, Agents 뷰 토글은 `titleMeta`/`actions`/커스텀으로 수용 — 기능 손실 없게.
- `truncate` 추가로 긴 제목(세션 ID·에이전트 ID) 처리 — `$sessionId`/`$agentId` 상세는 titleMeta로 ID 분리 권장.
- Settings(17)은 SettingsShell 유지 — 패러다임 단열은 본 설계에서 수용(경계 명시는 별건).

## 5. 검증
- `cd web && bun run build`(또는 `tsc --noEmit` + `vite build`)로 타입/번들 확인.
- 브라우저 smoke: Console 각 탭 헤더가 정렬·간격·부제 크기에서 동일하게 렌더되는지 대표 5탭(Dashboard·MCP·Memory·Settings·Skills) 확인.
- `git diff --stat`으로 범위 확인(예상 ~20파일).

## 6. 비범위(명시적 제외)
- EmptyChatState → 공유 EmptyState 전환(screen-18 MED): 구조 변경 동반, 별도 PR.
- Dashboard KPI 밀도/레이아웃 재설계: 본 설계는 헤더 일관화에 한정.
- Settings 3-zone → 타 패턴 통합: 의도적 패러다임으로 유지.
- `[&_svg]:size-4`로 인한 sm 버튼 아이콘 16px 강제: Button 변형 설계 필요, 별건.
