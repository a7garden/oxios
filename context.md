# i18n Update Context for Web Routes

## Overview
Update four route files to use `react-i18next` `useTranslation()` hook. Keep all logic unchanged.

---

## File 1: routes/skills.tsx

**Status:** Missing `useTranslation` import. Needs full i18n setup.

### Lines to replace (82, 93, 123, 126, 150, 153, 155):

| Line | Current String | Context |
|------|---------------|---------|
| ~82 | `Skills` | Page title in h1 |
| ~93 | `Manage and discover agent skills` | Page subtitle |
| ~123 | `Installed` | Tab button |
| ~126 | `Marketplace` | Tab button |
| ~150 | `All` | Filter button |
| ~153 | `Ready` | Filter button |
| ~155 | `Disabled` | Filter button (note: `needs_setup` on ~154 uses 'Needs Setup') |

### Additional hardcoded strings to translate:
- `Search installed skills...` (line ~99 placeholder)
- `No skills installed` (EmptyState title)
- `No matching skills` (EmptyState title)
- `Install skills from the Marketplace tab above.` (EmptyState description)
- `Try adjusting your filter or search query.` (EmptyState description)
- `Discover new skills` (Marketplace empty title)
- `Search thousands of community skills on ClawHub.` (Marketplace empty desc)
- `No results` (empty state title)
- `Search ClawHub marketplace...` (search input placeholder)
- `always` (Badge variant)
- `requires` / `install` (section headers, uppercase)
- `bins`, `any_bins`, `env`, `config` (ReqRow labels)
- `(missing)` (ReqRow)
- `Install` (MarketplaceCard button)
- `by` (SkillCard author prefix)

### Required change:
```tsx
import { useTranslation } from 'react-i18next'

function SkillsPage() {
  const { t } = useTranslation()
  // ... then use t('skills.title'), t('skills.manage'), etc.
}
```

---

## File 2: routes/cron-jobs.tsx

**Status:** Missing `useTranslation` import. Entire file needs i18n.

### Hardcoded strings to replace:
| String | Location |
|--------|----------|
| `Cron Jobs` | Page title |
| `Scheduled task management` | Page subtitle |
| `New Job` | Button |
| `Create Cron Job` | Card title |
| `Job name` | Input placeholder |
| `Cron schedule (e.g. */5 * * * *)` | Input placeholder |
| `Command to execute` | Input placeholder |
| `Create` | Button |
| `Cancel` | Button |
| `No cron jobs` | EmptyState title |
| `Create scheduled tasks to automate recurring work.` | EmptyState description |
| `Enabled` | Badge |
| `Disabled` | Badge |
| `Last:` | Label |
| `Next:` | Label |
| `Disable job` | aria-label |
| `Enable job` | aria-label |
| `Delete job` | aria-label |

### Required change:
```tsx
import { useTranslation } from 'react-i18next'

function CronJobsPage() {
  const { t } = useTranslation()
  // Use t('cron.title'), t('cron.newJob'), etc.
}
```

---

## File 3: routes/scheduler.tsx

**Status:** Has `useTranslation` imported. Partially uses i18n.

### Already using i18n:
- `t('scheduler.title')`
- `t('scheduler.subtitle')`
- `t('scheduler.status')`
- `t('scheduler.queued')`
- `t('scheduler.active')`
- `t('scheduler.maxConcurrent')`
- `t('scheduler.taskQueue')`
- `t('scheduler.noTasks')`
- `t('scheduler.noTasksDescription')`

### Hardcoded strings remaining (priority/status badges):
```tsx
<Badge variant="outline">{task.priority ?? '?'}</Badge>   // task.priority can be 'High', 'Medium', 'Low'
<Badge variant={...}>{task.status}</Badge>              // task.status can be 'Running', 'Queued', 'Completed', 'Failed'
```

These use dynamic values from API. Options:
1. Use `t('scheduler.priority.' + task.priority?.toLowerCase())` with fallback
2. Map to translation keys in code
3. Keep as-is if priority/status are raw values not meant to be translated

**Recommendation:** Add translation keys in the badge display logic with lowercase mapping.

---

## File 4: routes/chat.tsx

**Status:** Has Korean strings, needs `useTranslation` import and i18n.

### Mixed language strings (Korean → i18n):
| Current | Location |
|---------|----------|
| `대화 중` / `새 대화` | Header title |
| `연결 중...` | Connecting indicator |
| `새로고침` | Refresh button |
| `+ 새 대화` | New session button |
| `서버에 연결 중...` | Empty state |
| `메시지를 보내 대화를 시작하세요.` | Empty state |
| `Thinking...` | Streaming indicator |
| `메시지를 입력하세요...` | Input placeholder |
| `연결 대기 중...` | Input placeholder (disabled) |
| `Spaces` | Sidebar section header |
| `Sessions` | Sidebar section header |
| `Spaces 로드 중...` | Loading placeholder |
| `+ 새 대화` | New session button (sidebar) |
| `개 메시지` | Message count suffix |
| `전체 X개 세션 보기 →` | View all link |
| `← 간략히 보기` | Toggle back |
| `세션 관리 →` | Footer link |
| `Spaces 관리 →` | Footer link |

### Date group labels in `groupSessionsByDate`:
```tsx
if (diffDays === 0) label = '오늘'
else if (diffDays === 1) label = '어제'
else if (diffDays < 7) label = '이번 주'
else label = '이전'
```
These should become `t('chat.today')`, `t('chat.yesterday')`, etc.

---

## Translation Key Recommendations

### skills.ts keys:
```
skills.title
skills.manage
skills.installed
skills.marketplace
skills.all
skills.ready
skills.needsSetup
skills.disabled
skills.searchPlaceholder
skills.noSkillsInstalled
skills.noMatchingSkills
skills.installFromMarketplace
skills.adjustFilter
skills.discoverSkills
skills.searchClawHub
skills.noResults
skills.noResultsFor
skills.always
skills.requires
skills.install
skills.bins
skills.anyBins
skills.env
skills.config
skills.missing
skills.install
skills.by
```

### cron.ts keys:
```
cron.title
cron.subtitle
cron.newJob
cron.createTitle
cron.jobName
cron.schedulePlaceholder
cron.commandPlaceholder
cron.create
cron.cancel
cron.noJobs
cron.noJobsDescription
cron.enabled
cron.disabled
cron.last
cron.next
cron.disableJob
cron.enableJob
cron.deleteJob
```

### chat.ts keys:
```
chat.chatting
chat.newChat
chat.connecting
chat.refresh
chat.newChat
chat.connectingServer
chat.sendMessage
chat.thinking
chat.inputPlaceholder
chat.waiting
chat.spaces
chat.sessions
chat.loadingSpaces
chat.viewAllSessions
chat.showLess
chat.sessionManage
chat.spacesManage
chat.today
chat.yesterday
chat.thisWeek
chat.previous
chat.messages
```

---

## Implementation Notes

1. **Import pattern:** Always import `useTranslation` at top of file
2. **Component pattern:** `const { t } = useTranslation()` inside each component (not at file level)
3. **Key format:** Use dot notation (`section.subsection.key`)
4. **Fallbacks:** If translation key doesn't exist, t() returns the key itself — safe to deploy incrementally
5. **Logic preservation:** Do NOT change any variable names, function signatures, or conditional logic — only string literals
6. **scheduler.ts:** Already has i18n structure — only needs priority/status badge i18n updates if desired

---

## Output Files

Write i18n-enabled versions of:
1. `/Volumes/MERCURY/PROJECTS/oxios/channels/oxios-web/web/src/routes/skills.tsx`
2. `/Volumes/MERCURY/PROJECTS/oxios/channels/oxios-web/web/src/routes/cron-jobs.tsx`
3. `/Volumes/MERCURY/PROJECTS/oxios/channels/oxios-web/web/src/routes/scheduler.tsx` (optional: update badges)
4. `/Volumes/MERCURY/PROJECTS/oxios/channels/oxios-web/web/src/routes/chat.tsx`