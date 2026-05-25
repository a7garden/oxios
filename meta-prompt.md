# Meta-Prompt: Update chat.tsx to use i18n

## Goal
Add internationalization (i18n) support to `routes/chat.tsx` by replacing all hardcoded strings with translation function calls. Update both `en.json` and `ko.json` locale files with the required translation keys.

## Context/Evidence

**Source file:** `channels/oxios-web/web/src/routes/chat.tsx`
**Locale files:** 
- `channels/oxios-web/web/src/i18n/locales/en.json`
- `channels/oxios-web/web/src/i18n/locales/ko.json`

**Existing i18n pattern in codebase:**
```tsx
import { useTranslation } from 'react-i18next'

function SomeComponent() {
  const { t } = useTranslation()
  return <span>{t('common.someKey')}</span>
}
```

---

## Required Changes

### 1. Update chat.tsx

**a) Add import at top:**
```tsx
import { useTranslation } from 'react-i18next'
```

**b) Add hook inside `ChatPage` function:**
```tsx
const { t } = useTranslation()
```

**c) Replace hardcoded strings with t() calls:**

| Current | Replace With |
|---------|--------------|
| `'대화 중'` | `t('chat.activeConversation')` |
| `'새 대화'` | `t('chat.newConversation')` |
| `'연결 중...'` | `t('chat.connecting')` |
| `새로고침` (static text) | `t('common.refresh')` |
| `+ 새 대화` (static text) | `t('chat.newConversationButton')` |
| `'서버에 연결 중...'` | `t('chat.connectingToServer')` |
| `'메시지를 보내 대화를 시작하세요.'` | `t('chat.startConversationHint')` |
| `'Thinking...'` | `t('chat.thinking')` |
| `'메시지를 입력하세요...'` | `t('chat.inputPlaceholder')` |
| `'연결 대기 중...'` | `t('chat.waitingForConnection')` |
| `Spaces` (static text) | `t('common.spaces')` |
| `Spaces 로드 중...` | `t('chat.loadingSpaces')` |
| `Sessions` (static text) | `t('common.sessions')` |
| `+ 새 대화` (sidebar button) | `t('chat.newConversationButton')` |
| `` `${s.message_count}개 메시지` `` | `` t('chat.messageCount', { count: s.message_count }) `` |
| `` `전체 ${sessions.length}개 세션 보기 →` `` | `` t('chat.viewAllSessions', { count: sessions.length }) `` |
| `'← 간략히 보기'` | `t('chat.showLess')` |
| `'세션 관리 →'` | `t('chat.manageSessions')` |
| `'Spaces 관리 →'` | `t('chat.manageSpaces')` |

**d) Handle date grouping labels in `groupSessionsByDate`:**

The helper function returns hardcoded Korean labels. Change it to return numeric day counts, then translate in component:

```tsx
// Change return type and logic in groupSessionsByDate:
function groupSessionsByDate(sessions: Session[]): Record<number, Session[]> {
  const now = new Date()
  const groups: Record<number, Session[]> = {}

  for (const s of sessions) {
    const d = new Date(s.created_at)
    const diffDays = Math.floor((now.getTime() - d.getTime()) / (1000 * 60 * 60 * 24))
    
    if (!groups[diffDays]) groups[diffDays] = []
    groups[diffDays]!.push(s)
  }

  return groups
}

// Then in SpaceSessionSidebar, map diffDays to translation keys:
const getDateLabel = (diffDays: number) => {
  if (diffDays === 0) return t('chat.today')
  if (diffDays === 1) return t('chat.yesterday')
  if (diffDays < 7) return t('chat.thisWeek')
  return t('chat.previous')
}

// In the JSX where label is displayed:
<p className="text-xs text-muted-foreground px-2 mb-1">{getDateLabel(Number(label))}</p>
```

Note: The label variable will now be a number (diffDays), not a string. Update the rendering code accordingly.

### 2. Update en.json

Add to the `chat` object:

```json
"chat": {
  "title": "Chat",
  "activeConversation": "In conversation",
  "connecting": "Connecting...",
  "connectingToServer": "Connecting to server...",
  "startConversationHint": "Send a message to start the conversation.",
  "thinking": "Thinking...",
  "inputPlaceholder": "Type a message...",
  "waitingForConnection": "Waiting for connection...",
  "loadingSpaces": "Loading Spaces...",
  "newConversationButton": "+ New conversation",
  "messageCount": "{{count}} messages",
  "viewAllSessions": "View all {{count}} sessions →",
  "showLess": "← Show less",
  "manageSessions": "Manage sessions →",
  "manageSpaces": "Manage Spaces →",
  "today": "Today",
  "yesterday": "Yesterday",
  "thisWeek": "This week",
  "previous": "Previous"
}
```

### 3. Update ko.json

Add to the `chat` object:

```json
"chat": {
  "title": "채팅",
  "activeConversation": "대화 중",
  "connecting": "연결 중...",
  "connectingToServer": "서버에 연결 중...",
  "startConversationHint": "메시지를 보내 대화를 시작하세요.",
  "thinking": "Thinking...",
  "inputPlaceholder": "메시지를 입력하세요...",
  "waitingForConnection": "연결 대기 중...",
  "loadingSpaces": "Spaces 로드 중...",
  "newConversationButton": "+ 새 대화",
  "messageCount": "{{count}}개 메시지",
  "viewAllSessions": "전체 {{count}}개 세션 보기 →",
  "showLess": "← 간략히 보기",
  "manageSessions": "세션 관리 →",
  "manageSpaces": "Spaces 관리 →",
  "today": "오늘",
  "yesterday": "어제",
  "thisWeek": "이번 주",
  "previous": "이전"
}
```

---

## Success Criteria

- [ ] `useTranslation` imported at top of chat.tsx
- [ ] `const { t } = useTranslation()` called inside `ChatPage` function
- [ ] All hardcoded Korean/English strings replaced with `t()` calls
- [ ] All new translation keys added to `en.json` with English strings
- [ ] All new translation keys added to `ko.json` with Korean translations
- [ ] Date grouping labels (오늘/어제/이번 주/이전) properly translated
- [ ] Parameter interpolation works (e.g., `{ count: sessions.length }`)
- [ ] Code compiles without TypeScript errors
- [ ] No logic or API calls modified

---

## Hard Constraints

1. **DO NOT modify API calls** - all `fetch()`, `useQuery()`, store calls unchanged
2. **DO NOT modify business logic** - session grouping, message handling unchanged
3. **Keep existing `chat:` section keys** - merge new keys with existing ones
4. **Reuse existing `common:` keys** where appropriate (`common.refresh`, `common.spaces`, `common.sessions`)
5. **Only string replacement** - no structural changes to components except adding i18n

---

## Suggested Approach

1. First update `en.json` and `ko.json` with all new chat keys
2. Update `groupSessionsByDate` to return numeric diffDays
3. Update `chat.tsx` with import and hook
4. Replace each hardcoded string one by one
5. Add date label translation helper function
6. Test that TypeScript compiles cleanly

---

## Validation

Run these checks:
```bash
# TypeScript check
cd channels/oxios-web/web && npx tsc --noEmit

# Or use bun (as per project convention)
cd channels/oxios-web/web && bun tsc --noEmit
```

Verify no hardcoded Korean strings remain in the component after changes.

---

## Stop/Escalation Rules

- If TypeScript compilation fails → stop and fix errors
- If translation keys are missing at runtime → ensure they're added to both locale files
- If existing keys conflict → verify key names match exactly