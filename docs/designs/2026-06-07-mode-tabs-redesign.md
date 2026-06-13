# 모드 전환 UX 재설계 — 레이아웃 분리 + 모바일 대응

> **상태:** Draft  
> **날짜:** 2026-06-07  
> **영역:** oxios-web (frontend only)

---

## 0. 배경 및 문제

Oxios Web UI는 Console / Knowledge / Chat 세 가지 workspace를 사이드바 상단 `ModeTabs`로 전환한다.
현재 구조의 문제:

| # | 문제 | 심각도 |
|---|------|--------|
| P1 | 탭 클릭 → 사이드바만 바뀜, 메인 영역 불변 | 🔴 Critical |
| P2 | Chat 모드: 사이드바 + 이너사이드바 = 3컬럼으로 공간 낭비 | 🔴 Critical |
| P3 | 글로벌 설정(테마/언어/알림)이 사이드바·헤더에 분산 | 🟠 Medium |
| P4 | 모바일에서 모드 전환 경로 불명확 | 🟠 Medium |
| P5 | Knowledge 서브페이지(그래프/해빗)와 파일트리 사이드바의 부조화 | 🟡 Minor |

---

## 1. 핵심 설계 결정

**Console, Knowledge, Chat은 "모드"가 아니라 서로 다른 workspace다.**  
각 workspace는 자신만의 레이아웃을 가지며, 전환은 헤더의 탭 바로 한다.

| 결정 | 내용 |
|------|------|
| 모드 탭 | 사이드바 → **헤더 중앙** |
| 사이드바 | **모드별 전용** (Console=내비, Knowledge=파일트리, Chat=세션목록) |
| Chat 이너 사이드바 | **제거**, 내용을 메인 사이드바로 승격 (3컬럼→2컬럼) |
| 글로벌 설정 | 사이드바 하단+헤더 분산 → **헤더 우측 통합** |
| 모바일 | 헤더 간소화, 사이드바 오버레이 안에 모드 탭, 글로벌 설정은 ⚙️ 드롭다운 |

---

## 2. 최종 레이아웃

### 2.1 데스크톱 (≥768px)

```
┌──────────────────────────────────────────────────────────────┐
│ [🍔]  [Console]  [Knowledge]  [Chat]    [🌙] [🌐] [🔔] [⚙️] │ ← 헤더
├───────────┬──────────────────────────────────────────────────┤
│ SIDEBAR   │  MAIN AREA (<Outlet />)                          │
│ (모드별)  │                                                  │
└───────────┴──────────────────────────────────────────────────┘
```

### 2.2 모바일 (<768px)

```
┌──────────────────────┐        ┌── overlay sidebar ──────────┐
│ [🍔]  Oxios    [⚙️] │        │ [Console] [Knol] [Chat]    │
├──────────────────────┤        │ ─────────────────────────── │
│                      │        │ mode-specific nav / files   │
│  Content (full)      │        │                             │
│                      │        └─────────────────────────────┘
│                      │
│                      │        ┌── settings dropdown ────────┐
│                      │        │ 🌙 Theme → light/dark/sys   │
│                      │        │ 🌐 Language → EN/KO         │
│                      │        │ 🔔 Notifications            │
│                      │        │ ⚙️ Settings                 │
└──────────────────────┘        └─────────────────────────────┘
```

---

## 3. 모드별 사이드바

### 3.1 Console 사이드바

```
┌──────────────┐
│ ☰  Oxios    │  ← 로고 + collapse 토글
├──────────────┤
│ 📊 대시보드  │
│ ✅ 승인      │
│ 💬 채팅      │
│ ─────────── │
│ 🤖 에이전트  │
│ 👥 그룹      │
│ 🧬 시드      │
│ 🎭 페르소나  │
│ ⚡ 스킬      │
│ ─────────── │
│ 📁 프로젝트  │
│ ─────────── │
│ 📝 지식      │
│ 🧠 메모리    │
│ 📂 워크스페이스│
│ ─────────── │
│ 📅 스케줄러  │
│ 🗓️ 캘린더   │
│ ⏱ 크론잡    │
│ 💰 예산      │
│ ─────────── │
│ ⚡ MCP       │
│ ✉️ 이메일    │
│ ⎇ Git       │
│ 🔗 A2A       │
│ ─────────── │
│ 📈 리소스    │
│ 🛡️ 보안     │
│ 🔔 이벤트    │
└──────────────┘
```

기존 `ConsoleNav` 그대로 유지. 변경 없음.

### 3.2 Knowledge 사이드바

```
┌──────────────┐
│ ☰  Oxios    │
├──────────────┤
│ 💬 Quick Notes│  ← KnowledgeChat으로 전환
│ 📖 저널      │
│ 🔗 그래프    │
│ 📊 해빗       │
│ ⚙️ 설정      │
│ ─────────── │
│ 📁 파일      │
│ 📄 README.md │  ← FileTree
│ 📁 docs/     │
│   📄 a.md    │
│ 📁 share/    │
└──────────────┘
```

기존 `KnowledgeNav` 그대로 유지. 변경 없음.

### 3.3 Chat 사이드바

```
┌──────────────┐
│ ☰  Oxios    │
├──────────────┤
│ [+ 새 대화]  │
│ ─────────── │
│ 📁 Projects  │
│ ● oxios     │
│ ● other     │
│ ─────────── │
│ 💬 오늘      │
│ ◌ session 1 │
│ ◌ session 2 │
│ 📆 어제      │
│ ◌ session 3 │
│ ...         │
│ ─────────── │
│ 모든 세션 →  │  ← /sessions
│ 프로젝트 →   │  ← /projects
└──────────────┘
```

**중요 변경:** 기존 ChatPage 내부의 `ProjectSessionSidebar` 내용을 **메인 사이드바로 승격**.  
`ChatNav` → `ChatSessionNav`로 확장. ChatPage 자체는 사이드바 없이 순수 채팅 영역만 렌더.

---

## 4. 헤더 설계

### 4.1 데스크톱 헤더

```tsx
<header className="flex h-14 items-center gap-4 border-b bg-background px-4 lg:px-6">
  {/* Mobile hamburger — hidden on desktop */}
  <button className="lg:hidden" onClick={() => setMobileOpen(true)}>
    <Menu className="h-5 w-5" />
  </button>

  {/* Mode tabs */}
  <ModeTabs />

  <div className="flex-1" />

  {/* Global actions */}
  <div className="flex items-center gap-1">
    <ThemeToggle />
    <LanguageSelector />
    <NotificationBell />
    <SettingsLink />
  </div>
</header>
```

- `ThemeToggle`: 🌙/☀️/🖥️ 전환 (기존 sidebar.tsx 로직을 컴포넌트로 추출)
- `LanguageSelector`: 기존 그대로
- `NotificationBell`: 기존 그대로
- `SettingsLink`: `/settings` 링크 (⚙️ 아이콘)

### 4.2 모바일 헤더

```tsx
<header className="flex h-14 items-center gap-2 border-b bg-background px-3">
  <button onClick={() => setMobileOpen(true)}>
    <Menu className="h-5 w-5" />
  </button>

  <span className="font-bold text-lg">Oxios</span>

  <div className="flex-1" />

  {/* Consolidated settings dropdown — replaces 4 separate icons */}
  <SettingsDropdown />
</header>
```

`SettingsDropdown` 컴포넌트:
```tsx
function SettingsDropdown() {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" className="h-8 w-8">
          <Settings className="h-4 w-4" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        {/* Theme submenu */}
        <DropdownMenuSub>
          <DropdownMenuSubTrigger>
            <Sun className="h-4 w-4 mr-2" /> Theme
          </DropdownMenuSubTrigger>
          <DropdownMenuSubContent>
            <DropdownMenuItem onClick={() => setTheme('light')}>☀️ Light</DropdownMenuItem>
            <DropdownMenuItem onClick={() => setTheme('dark')}>🌙 Dark</DropdownMenuItem>
            <DropdownMenuItem onClick={() => setTheme('system')}>🖥️ System</DropdownMenuItem>
          </DropdownMenuSubContent>
        </DropdownMenuSub>

        {/* Language submenu */}
        <DropdownMenuSub>
          <DropdownMenuSubTrigger>
            <Globe className="h-4 w-4 mr-2" /> Language
          </DropdownMenuSubTrigger>
          <DropdownMenuSubContent>
            <DropdownMenuItem onClick={() => changeLanguage('en')}>English</DropdownMenuItem>
            <DropdownMenuItem onClick={() => changeLanguage('ko')}>한국어</DropdownMenuItem>
          </DropdownMenuSubContent>
        </DropdownMenuSub>

        <DropdownMenuSeparator />

        <DropdownMenuItem asChild>
          <Link to="/settings">
            <Settings className="h-4 w-4 mr-2" /> Settings
          </Link>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
```

### 4.3 모바일 사이드바 오버레이

모바일에서 사이드바가 열릴 때, 상단에 모드 탭을 포함:

```tsx
// sidebar.tsx — mobile overlay content

{mobileOpen && (
  <>
    {/* Mode tabs inside sidebar on mobile */}
    <div className="lg:hidden px-2 py-1.5">
      <ModeTabs variant="sidebar" />
      <Separator className="my-1" />
    </div>
    
    {/* Mode-specific nav */}
    <nav className="flex-1 overflow-y-auto p-2">
      {mode === 'console' && <ConsoleNav />}
      {mode === 'knowledge' && <KnowledgeNav />}
      {mode === 'chat' && <ChatSessionNav />}
    </nav>
  </>
)}
```

---

## 5. ModeTabs 컴포넌트 (공용)

```tsx
// components/layout/mode-tabs.tsx

interface ModeTabsProps {
  variant?: 'header' | 'sidebar'
}

const MODES = [
  { key: 'console' as const, icon: LayoutDashboard, labelKey: 'sidebar.console', href: '/' },
  { key: 'knowledge' as const, icon: NotebookPen, labelKey: 'sidebar.knowledge', href: '/knowledge' },
  { key: 'chat' as const, icon: MessageSquare, labelKey: 'sidebar.chat', href: '/chat' },
]

export function ModeTabs({ variant = 'header' }: ModeTabsProps) {
  const { t } = useTranslation()
  const pathname = useRouterState().location.pathname

  const currentMode: SidebarMode =
    pathname.startsWith('/knowledge') ? 'knowledge'
    : pathname === '/chat' ? 'chat'
    : 'console'

  return (
    <nav aria-label={t('common.modeNavigation')} className={cn(
      'flex items-center',
      variant === 'header' ? 'gap-0.5' : 'gap-0.5'
    )}>
      {MODES.map(({ key, icon: Icon, labelKey, href }) => {
        const isActive = currentMode === key
        return (
          <Link
            key={key}
            to={href}
            aria-current={isActive ? 'page' : undefined}
            className={cn(
              'flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium transition-colors',
              // Header variant: bottom border highlight
              variant === 'header' && [
                'border-b-2 -mb-px',
                isActive
                  ? 'border-primary text-foreground'
                  : 'border-transparent text-muted-foreground/60 hover:text-foreground/80 hover:border-border',
              ],
              // Sidebar variant: bg highlight
              variant === 'sidebar' && [
                'flex-1 justify-center',
                isActive
                  ? 'bg-sidebar-accent text-sidebar-accent-foreground'
                  : 'text-sidebar-foreground/50 hover:bg-sidebar-accent/50',
              ],
            )}
          >
            <Icon className="h-4 w-4" />
            <span>{t(labelKey)}</span>
          </Link>
        )
      })}
    </nav>
  )
}
```

---

## 6. ChatSessionNav (Chat 사이드바 확장)

`components/layout/chat-session-nav.tsx` 신규 — 기존 `ProjectSessionSidebar` 로직을 이식:

```tsx
export function ChatSessionNav() {
  const { collapsed } = useSidebarStore()
  const { activeProjectId, activeSessionId, setActiveProject, loadSession, newSession } = useChatStore()

  // ... projects/sessions fetching, grouping (기존 ProjectSessionSidebar 로직)

  if (collapsed) {
    return (
      <div className="flex flex-col items-center gap-1 py-1">
        <Tooltip content={t('chat.newConversation')} side="right">
          <button onClick={newSession} className="p-2 rounded-md hover:bg-sidebar-accent/50">
            <Plus className="h-4 w-4" />
          </button>
        </Tooltip>
      </div>
    )
  }

  return (
    <>
      {/* New session button */}
      <div className="p-2">
        <Button size="sm" className="w-full" onClick={newSession}>
          <Plus className="h-3 w-3 mr-1" /> {t('chat.newConversationButton')}
        </Button>
      </div>

      {/* Projects list */}
      <div className="p-2 border-t">
        <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-1">
          {t('chat.projectsLabel')}
        </p>
        {projects.map(p => (
          <button onClick={() => setActiveProject(p.id)} className={cn(...)}>
            <span className="h-2 w-2 rounded-full bg-success" />
            {p.name}
          </button>
        ))}
      </div>

      {/* Sessions list */}
      <div className="flex-1 overflow-y-auto p-2">
        {groupedSessions.map(([label, sessions]) => (
          <div key={label}>
            <p className="text-xs text-muted-foreground">{label}</p>
            {sessions.map(s => (
              <button onClick={() => loadSession(s.id)} className={cn(...)}>
                {s.message_count} messages · {formatDate(s.created_at)}
              </button>
            ))}
          </div>
        ))}
      </div>

      {/* Footer links */}
      <div className="p-2 border-t">
        <Link to="/sessions">{t('chat.manageSessions')}</Link>
        <Link to="/projects">{t('chat.manageProjects')}</Link>
      </div>
    </>
  )
}
```

---

## 7. ChatPage 간소화

`routes/chat.tsx`에서 `ProjectSessionSidebar` 제거. 순수 채팅 UI만:

```tsx
// chat.tsx — simplified

function ChatPage() {
  // ... stores, handlers

  return (
    <div className="flex flex-col h-[calc(100vh-4rem)]">
      {/* Minimal header: just session title + actions */}
      <div className="flex items-center justify-between px-4 py-2 border-b">
        <h2 className="text-sm font-semibold">
          {activeSessionId ? t('chat.activeConversation') : t('chat.newConversation')}
        </h2>
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="sm" onClick={() => loadSession(activeSessionId!)}>
            <RefreshCw className="h-3 w-3 mr-1" /> {t('chat.refreshing')}
          </Button>
          <Button variant="outline" size="sm" onClick={newSession}>
            + {t('chat.newConversationButton')}
          </Button>
        </div>
      </div>

      {/* Reconnect banner */}
      {!connected && <ReconnectBanner />}

      {/* Messages */}
      <Card className="flex-1 flex flex-col min-h-0 mx-4 my-3">
        <ScrollArea className="flex-1 p-4">
          {messages.length === 0 && !activeInterview ? (
            <EmptyChatState onSuggestionClick={setInput} />
          ) : (
            <div className="space-y-4">
              {messages.map(msg => <MessageBubble key={msg.id} message={msg} />)}
              {activeInterview && <InterviewResponse ... />}
              <div ref={bottomRef} />
            </div>
          )}
        </ScrollArea>

        {!activeInterview && <ChatInput ... />}
      </Card>
    </div>
  )
}
```

---

## 8. AppLayout 변경

```tsx
// app-layout.tsx

export function AppLayout() {
  const pathname = useRouterState().location.pathname
  const isKnowledge = pathname.startsWith('/knowledge')

  return (
    <div className="flex h-screen overflow-hidden">
      {/* Mobile overlay */}
      {mobileOpen && (
        <div className="fixed inset-0 z-40 bg-black/50 lg:hidden"
          onClick={() => setMobileOpen(false)} />
      )}

      {/* Sidebar — desktop: always visible, mobile: overlay */}
      <div className={cn(
        'hidden lg:flex',
        mobileOpen && 'fixed inset-y-0 left-0 z-50 flex flex-col bg-sidebar w-60',
      )}>
        <Sidebar />
      </div>

      {/* Main area */}
      <div className="flex flex-1 flex-col min-w-0 overflow-hidden">
        <Header />

        {isKnowledge ? (
          <div className="flex flex-1 min-h-0 overflow-hidden">
            <div className="flex flex-1 min-w-0 overflow-hidden">
              <Outlet />
            </div>
            {infoPanelOpen && <InfoPanel />}
          </div>
        ) : (
          <main className="flex-1 overflow-y-auto min-h-0">
            <Outlet />
          </main>
        )}
      </div>

      {/* Knowledge modals */}
      {isKnowledge && <><SearchModal /><MoveModal /></>}
    </div>
  )
}
```

---

## 9. 구현 파일 목록

| 파일 | 작업 | 설명 |
|------|------|------|
| `components/layout/mode-tabs.tsx` | **신규** | 헤더/사이드바 공용 모드 탭 |
| `components/layout/chat-session-nav.tsx` | **신규** | Chat 사이드바 — ProjectSessionSidebar 로직 이식 |
| `components/layout/header.tsx` | **수정** | ModeTabs + 글로벌 액션 통합. KnowledgeBreadcrumb 제거 |
| `components/layout/sidebar.tsx` | **수정** | ModeTabs 제거, Chat일 때 ChatSessionNav, 글로벌 설정 제거, 모바일 ModeTabs 조건부 |
| `components/layout/app-layout.tsx` | **수정** | Chat 모드에서 padding 제거 (채팅이 전체 높이 사용) |
| `routes/chat.tsx` | **간소화** | ProjectSessionSidebar 제거, 순수 채팅 UI만 |
| `stores/sidebar.ts` | **수정** | `deriveSidebarMode` 유지 (캐싱 추가 고려) |
| `components/ui/dropdown-menu.tsx` | 확인 | 모바일 SettingsDropdown에 필요한 Sub/SubTrigger/SubContent 존재 확인 |

---

## 10. 구현 순서

1. **ModeTabs 컴포넌트** — `mode-tabs.tsx` 신규
2. **Header 수정** — ModeTabs + 글로벌 액션 통합, KnowledgeBreadcrumb 제거
3. **Sidebar 수정** — ModeTabs 제거, 글로벌 설정 제거, 모바일 조건부 ModeTabs 추가
4. **ChatSessionNav** — `chat-session-nav.tsx` 신규 (ProjectSessionSidebar 로직 이식)
5. **ChatPage 간소화** — ProjectSessionSidebar 제거
6. **모바일 SettingsDropdown** — `header.tsx` 내 구현 (또는 별도 컴포넌트)
7. **AppLayout** — Chat 모드 레이아웃 조정
8. **i18n** — 필요한 키 추가
9. **반응형 테스트** — 1440px, 1280px, 768px, 375px
10. **접근성** — 키보드 내비게이션, `aria-current`, `aria-label`

---

## 11. 고려사항

### 11.1 모드별 사이드바 콜랩스 상태

사이드바 콜랩스는 모드와 무관하게 유지. 현재 `persist`되어 있으므로 변경 없음.

### 11.2 Knowledge 서브페이지 (그래프/해빗/설정)

Knowledge 사이드바(파일트리+링크)는 서브페이지 진입 시에도 유지.
사용자가 파일을 클릭하면 에디터로 복귀. (현재 구조와 동일)

### 11.3 Chat 사이드바 데이터 로딩

`ChatSessionNav`에서 `useChatStore`와 API 쿼리를 사용.
세션/프로젝트 로딩은 ChatPage 진입 시에만 활성화 (lazy).

### 11.4 사이드바 콜랩스 시 Chat

Chat 사이드바가 콜랩스되면 새 대화(+), 프로젝트, 세션 아이콘만 표시.
툴팁으로 상세 정보 제공.

### 11.5 테마/언어 상태 관리

`useThemeStore`, `i18n` 기존 상태 관리 그대로 사용.
`SettingsDropdown`에서 `setTheme()`, `changeLanguage()` 호출.

### 11.6 모바일 사이드바 닫기

모드 탭 클릭 또는 내비게이션 링크 클릭 시 사이드바 닫기 (`setMobileOpen(false)`).
오버레이 배경 클릭 시에도 닫기 (기존 동작 유지).
