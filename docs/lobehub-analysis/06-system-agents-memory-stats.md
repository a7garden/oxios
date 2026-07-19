# LobeHub Phase 4 Analysis: System Agents, Memory Layers, Stats, Tasks

> Deep-dive into LobeHub's automation, memory categorization, usage stats, and scheduled tasks.

## 1. System Agent Model Assignment

LobeHub lets users assign **different models to different system tasks**. This is brilliant cost optimization — use a cheap model for topic naming, expensive model for memory analysis.

### SystemAgentItem type (`packages/types/src/user/settings/systemAgent.ts`)

```typescript
interface SystemAgentItem {
  contextLimit?: number;   // token cap for this task
  customPrompt?: string;   // override system prompt
  enabled?: boolean;       // toggle this system task
  model?: string;          // model id
  provider?: string;       // provider id
}

interface UserSystemAgentConfig {
  topic: SystemAgentItem;              // auto topic naming
  generationTopic: SystemAgentItem;    // AI image topic naming
  translation: SystemAgentItem;        // message translation
  historyCompress: SystemAgentItem;    // conversation compression
  agentMeta: SystemAgentItem;          // agent name/desc/avatar/tags generation
  followUpAction: SystemAgentItem;     // follow-up suggestion chips
  inputCompletion: SystemAgentItem;    // input auto-complete (ghost text)
  promptRewrite: PromptRewriteSystemAgent;  // prompt rewriting
}
```

### Memory service models (separate group)

```typescript
interface UserMemoryServiceModelConfig {
  memoryAnalysisAgentConfig: SystemAgentItem;  // analyze if convo contains memory
  userMemoryEmbedding: SystemAgentItem;        // embedding model
  userMemoryPersonaWriter: SystemAgentItem;    // personalized memory summaries
}
```

### Settings UI (`/settings/service-model`)

3 groups in `ModelAssignmentsForm`:
1. **Default agent** — model used when creating new agents
2. **System agents** (5 items): topic, generationTopic, translation, historyCompress, agentMeta
3. **Memory models** (3 items): memoryAnalysis, embedding, personaWriter
4. **Optional features** (3 items): followUp, inputCompletion, promptRewrite

Each item: model select dropdown + optional contextLimit input + enable toggle.

## 2. Memory 5-Category System

LobeHub's user memory is categorized into **5 semantic layers**:

### Categories (`packages/types/src/userMemory/`)

| Layer | Purpose | Key Fields |
|-------|---------|------------|
| **identity** | Who the user is | role, relationship, type (personal/professional/demographic), description |
| **activity** | What the user does | narrative, feedback, notes, associated actions |
| **context** | Situational context | title, description, associatedObjects, associatedSubjects, currentStatus |
| **experience** | Past learnings | action, situation, keyLearning, possibleOutcome, reasoning, confidence |
| **preference** | Enduring choices | topic, conclusionDirectives, suggestions |

Each layer has:
- Vector embeddings for semantic search (e.g., `descriptionVector`, `actionVector`)
- Timestamps (createdAt, updatedAt, accessedAt)
- Episodic date (when the event occurred)
- Tags/labels for organization

### Memory tools (`builtin-tool-memory`)

The agent has a `memory` tool with operations per layer:
- `createIdentity`, `updateIdentity`, `listIdentities`
- `createActivity`, `updateActivity`, `listActivities`
- `createContext`, `updateContext`, `listContexts`
- `createExperience`, `updateExperience`, `listExperiences`
- `createPreference`, `updatePreference`, `listPreferences`

## 3. Scheduled Task System

### Task schedule API (`builtin-tool-task`)

```typescript
setTaskSchedule({
  automationMode: 'schedule' | 'heartbeat' | null,
  schedulePattern: string,       // cron expression: "0 9 * * *" = daily 9am
  scheduleTimezone: string,      // IANA: "Asia/Seoul"
  heartbeatInterval: number,     // seconds between ticks
  maxExecutions: number | null,  // cap, null = unlimited
})
```

Two automation modes:
- **schedule**: cron-based, fires at specific times
- **heartbeat**: fixed-interval, fires every N seconds

### Use cases (from the user's examples)

| Use case | Implementation |
|----------|---------------|
| "Weekly font + color recommendations" | Task with `schedule: "0 10 * * 3"` (Wed 10am), agent skill = "design curator" |
| "Daily creator tracking" | Task with `heartbeat: 86400` (daily), agent skill = "creator monitor" |
| "Weekly YouTube summary" | Task with `schedule: "0 9 * * 1"` (Mon 9am), agent skill = "YouTube analyst" |

## 4. Stats Dashboard (`/settings/stats`)

### Layout

```
┌─────────────────────────────────────────────────┐
│  Welcome banner + date range picker             │
├─────────────────────────────────────────────────┤
│  Overview cards:                                │
│  [Total Topics] [Total Messages] [Total Tokens] │
│  [Total Assistants] [Share button]              │
├─────────────────────────────────────────────────┤
│  Usage section:                                 │
│  [Today's spend] [Month's spend]                │
│  [Active models table] [Usage trends chart]     │
├─────────────────────────────────────────────────┤
│  Rankings:                                      │
│  [Topics rank] [Assistants rank] [Models rank]  │
├─────────────────────────────────────────────────┤
│  Visualization:                                 │
│  [AI activity heatmap]                          │
└─────────────────────────────────────────────────┘
```

### Tabs: Overview | Usage | Rankings | Visualization

Each tab uses `@lobehub/ui` FormGroup + Grid for responsive layout.

## 5. AI Image Generation (`/settings/image`)

### Settings
- `AI_IMAGE_DEFAULT_IMAGE_NUM`: default image count (1-20, default 4)
- Image generation panel in chat input
- Provider config for image models (DALL-E, Stable Diffusion, etc.)

## 6. What Oxios Already Has

| Feature | Oxios Status |
|---------|-------------|
| Cron jobs | ✅ `/cron-jobs` route + backend scheduler |
| Memory | ⚠️ Hot/Warm/Cold tiers, but no 5-category system |
| Stats | ⚠️ Basic cost tracking, no dashboard |
| System agents | ❌ No per-task model assignment |
| AI image | ❌ Not implemented |
| Task scheduling | ⚠️ Cron exists but no heartbeat mode |

## 7. Recommended Implementation Priority

### Phase 4a: System Agent Model Assignment (High Impact)
- Add `[system_agents]` config section
- Settings UI with model picker per task
- Wire to backend (topic naming, translation, compression)

### Phase 4b: Memory 5-Category System (High Impact)
- Extend MemoryManager with identity/activity/context/experience/preference
- Memory browser UI with category tabs
- Agent memory tool with per-category operations

### Phase 4c: Stats Dashboard (Medium Impact)
- Usage overview cards
- Cost trends chart (recharts already in deps)
- Model usage rankings

### Phase 4d: Task Heartbeat Mode (Medium Impact)
- Extend cron system with heartbeat mode
- Task scheduler UI with both modes
