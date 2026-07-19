# LobeHub ↔ Oxios: Chat UI/UX Deep Dive

> Source: LobeHub v2.2.9 (`src/features/Conversation/`, `src/features/ChatInput/`) vs Oxios (`web/src/components/chat/`)

## 1. Message Architecture

### LobeHub: Multi-Role Dispatcher

LobeHub's `Conversation/Messages/index.tsx` dispatches **12+ message role types** through a switch statement:

| Role | Component | Description |
|------|-----------|-------------|
| `user` | `UserMessage` | Right-aligned bubble with sender avatar in workspaces |
| `assistant` | `AssistantMessage` | ChatItem with avatar, markdown, actions |
| `assistantGroup` | `AssistantGroupMessage` | Multi-step agent turns with collapsible workflow |
| `supervisor` | `AssistantMessage` | Supervisor-badged assistant variant |
| `tool` | `ToolMessage` | Standalone tool result (expandable accordion) |
| `task` | `TaskMessage` | Single task execution display |
| `tasks` | `TasksMessage` | Batch task list |
| `groupTasks` | `GroupTasksMessage` | Group-level task orchestration |
| `agentCouncil` | `AgentCouncilMessage` | Multi-agent deliberation display |
| `compressedGroup` | `CompressedGroupMessage` | Context-compressed group turns |
| `verify` | `VerifyMessage` | Verification report display |
| `taskCallback` | `TaskCallbackMessage` | Task completion callback |

Every message wraps in `ChatItem` which provides: **avatar**, **title bar** (name + time), **error display**, **message body** (with bubble variant for user), **loading state**, **messageExtra**, **hover-revealed action bar**, and **FollowUpChips**.

### Oxios: Single Component, Role-Branched

Oxios's `MessageBubble` handles **4 roles** in one component:

| Role | Rendering |
|------|-----------|
| `user` | Subtle muted card, left-aligned |
| `assistant` | Background-less, full-width markdown prose |
| `tool` | Full-width `ToolCallCard` |
| `system` | Treated like assistant |

Design philosophy: "prose is the hero" — no chat bubbles, no avatars. Clean and restrained.

### Gap Analysis

| Feature | LobeHub | Oxios |
|---------|---------|-------|
| Role types | 12+ | 4 |
| Avatar/identity | Per-agent avatar + title | Model name chip only |
| Message actions | Edit, retry, branch, copy, share, translate, TTS, delete | Retry on error only |
| Hover actions | CSS transition reveal | None |
| Follow-up chips | Auto-generated after each response | None |
| Message branching | Full branch navigation | None |

## 2. Assistant Message Rendering

### LobeHub: Content Pipeline

```
Reasoning → SearchGrounding → FileChunks → DisplayContent → ImageFileListViewer → Reactions
```

Each stage is conditional:
- **Reasoning**: Rendered when `isMessageInReasoning` — shows `Thinking` accordion with animated streaming
- **SearchGrounding**: Collapsible section with citation cards (favicons, URLs), image results grid, search query tags
- **FileChunks**: RAG reference chunks with collapsible card
- **DisplayContent**: Markdown via `@lobehub/ui Markdown` OR multimodal `RichContentRenderer`
- **ImageFileListViewer**: Generated/attached image gallery
- **ReactionDisplay**: Emoji reactions

### Oxios: Activity-First Layout

```
ActivityTimeline (above) → ReactMarkdown (body) → Metadata footer + KnowledgeSaveIndicator
```

- **ActivityTimeline** (RFC-015): Collapsible header with tool count + token summary, expandable `ActivityCard` entries
- **ReactMarkdown**: `remarkGfm` + `rehypeHighlight` — basic but functional
- **ChatMetadata**: Phase badge, evaluation status, duration
- **KnowledgeSaveIndicator** (RFC-016): Per-message save-to-knowledge affordance

### Gap Analysis

| Feature | LobeHub | Oxios |
|---------|---------|-------|
| Thinking/reasoning block | Accordion with auto-scroll, animated streaming | Activity card (static) |
| Search grounding | Full citations + image grid | None |
| RAG chunks | Collapsible reference cards | None |
| Image display | Gallery with lightbox | Via markdown only |
| Markdown plugins | 14 custom rehype/remark plugins | 2 standard plugins |
| Code highlighting | highlight.js with fullFeatured + theme | rehypeHighlight (server-side) |
| Mermaid diagrams | Built-in with theme support | None |
| HTML preview | HtmlPreviewDrawer | None |
| Streaming animation | `animated` + `enableStream` props | Static ReactMarkdown |

## 3. Thinking/Reasoning UX

### LobeHub

```typescript
// Conversation/components/Thinking/index.tsx
<Accordion>
  <Title thinking={thinking} duration={duration} />
  <ScrollArea maxHeight="min(40vh, 320px)">
    <MarkdownMessage content={content} />
  </ScrollArea>
</Accordion>
```

- **Auto-expand** when `thinking=true` (during streaming)
- **Duration display** with elapsed time
- **Dimmed prose** (`colorTextDescription`)
- **Auto-scroll** to bottom
- **Status indicator** with animation

### Oxios

```typescript
// ActivityCard in activity-timeline.tsx
// Shows type icon (Brain), summary text, duration
// No streaming animation, no auto-expand
```

### Gap: **Critical**

Oxios needs a `ThinkingBlock` component that:
1. Auto-expands during streaming, collapses after
2. Shows animated thinking indicator + elapsed time
3. Renders streaming markdown in dimmed style
4. Supports multimodal content (images in reasoning)

## 4. Tool Call Rendering

### LobeHub

Tools render inside `AssistantGroup` messages as `AccordionItem` components:

```
Inspector Header:
  [StatusIndicator] [ToolIcon] ToolName  ArgsPreview  [ExecutionTime]
  [Actions: toggle custom render, debug]

Detail Body:
  CustomRender (builtin-tools/renders)  OR
  FallbackArgumentRender (JSON args)    OR
  RejectedResponse                      OR
  AbortResponse
  + LoadingPlaceholder for in-flight tools
  + InterventionPanel for approval flows
```

Custom renders from `@lobechat/builtin-tools`:
- **File tools**: File path display, diffs, content previews
- **Web browsing**: Search results with favicons, page previews
- **Code execution**: Terminal output with syntax highlighting
- **GitHub/Linear**: Rich integration cards
- **Image generation**: Generated image display
- **Agent dispatch**: Sub-agent status cards

### Oxios

```typescript
// tool-call-card.tsx
<ExpandableCard>
  <Header>🔧 {toolName} · {duration}</Header>
  <Body>
    <pre>Input: {JSON.stringify(args)}</pre>
    <pre>Output: {result}</pre>
  </Body>
</ExpandableCard>
```

Single generic card for all tools. No custom renders. No intervention support. No streaming detection.

### Gap: **Critical**

Oxios needs:
1. **Custom tool render registry**: Per-tool React components
2. **Tool inspector**: Inline expansion with argument preview
3. **Streaming detection**: Loading placeholder for in-flight tools
4. **Intervention panel**: Human approval flow for risky operations
5. **Result formatting**: Syntax highlighting, file previews, diff display

## 5. Chat Input & Controls

### LobeHub

LobeHub uses a **Lexical rich-text editor** (`@lobehub/editor`) with extensive toolbar:

```
┌─────────────────────────────────────────────────────────┐
│ [AgentMode] [Model] [Search] [Tools] [Upload] [Know...] │ ← ActionBar (left)
│ [Memory] [History] [Params] [Clear] [Typo] [Mention]   │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  [Rich text editor with @mentions, file tags,           │
│   action tags, topic references]                        │
│                                                         │
├─────────────────────────────────────────────────────────┤
│ [ControlBar: workspace/git/approval mode]  [Send ▸]     │
└─────────────────────────────────────────────────────────┘
```

16 action bar controls:
- **agentMode**: Chat vs Agent toggle
- **model**: Model switch panel
- **modelLabel**: Current model name display
- **search**: Web search toggle (auto/off) with provider selection
- **tools**: Tool enable/disable picker
- **upload**: File upload with drag-drop
- **knowledge**: Knowledge base selector with file checkboxes
- **memory**: Memory toggle
- **history**: Input history
- **params**: Temperature/model params
- **clear**: Clear input
- **typo**: Typo correction toggle
- **mention**: @mention trigger
- **plus**: Additional actions menu
- **contextWindow**: Token usage display

### Oxios

```
┌─────────────────────────────────────────────────────────┐
│                                                         │
│  [Plain textarea with auto-grow]                        │
│  [@mention popover for knowledge/memory/mounts]         │
│                                                         │
├─────────────────────────────────────────────────────────┤
│ [ModelPicker pill]              [Send ▸] [Stop ■]       │
└─────────────────────────────────────────────────────────┘
```

Features:
- **ModelPicker**: Unified pill with model + role routing (mutually exclusive)
- **@mention**: Knowledge, memory, mount context attachment
- **Context chips**: Attached context shown as removable chips
- **Queue indicator**: Queued message count on send button
- **Composition-aware Enter**: IME-safe send on Enter

### Gap Analysis

| Feature | LobeHub | Oxios |
|---------|---------|-------|
| Rich text editor | Lexical with full plugin system | Plain textarea |
| Search toggle | Per-message toggle with provider choice | None |
| File upload | Drag-drop with preview | None |
| Knowledge toggle | Multi-select knowledge base picker | @mention only |
| Tool selector | Per-message tool enable/disable | @mention only |
| Input history | Up/down arrow history | None |
| Agent mode toggle | Chat vs Agent | Implicit (no toggle) |
| Model label | Current model name display | In picker pill only |
| Token counter | Context window usage | None |
| Expand/fullscreen | Portal-based expand mode | None |

## 6. Streaming & Loading States

### LobeHub

**`ContentLoading`** component:
- Shows operation label ("Generating…", "Searching…", "Executing…") with elapsed time
- `BubblesLoading` fallback for unknown operations
- Stream-retry status
- Heterogeneous agent names
- Hides for 'reasoning' operations (Thinking component covers it)

### Oxios

**`LiveActivityBar`** (RFC-015):
- Real-time activity descriptor ("Thinking…", "Running tool…", "Reasoning…")
- Fades out when streaming text begins
- Derived from `deriveCurrentActivity` + `describeLiveActivity` helpers

### Gap: **Medium**

Oxios's `LiveActivityBar` is actually good — clean and unobtrusive. Missing: elapsed time display, operation-specific labels.

## 7. Error Handling

### LobeHub

```typescript
// Error/index.tsx
- BaseErrorForm: Generic error with message + trace ID
- PlanLimitCard: Quota/plan limit warnings
- TraceIdError: Debug trace ID display
- Heterogeneous auto-retry: Automatic retry logic with backoff
- Retry parent message: Context-aware retry
```

### Oxios

```typescript
// message-bubble.tsx → inline error cards (RFC-032)
- Subtle red-bordered card with error message
- Retry button calling onRetry callback
- No error classification, no trace IDs
```

### Gap: **Medium**

Oxios should add: error classification (quota vs network vs model), trace ID display, auto-retry for transient errors.

## 8. Design Recommendations for Oxios

### Phase 1: High-Impact, Low-Effort

1. **Thinking/Reasoning Block** — Collapsible accordion with streaming animation, elapsed time, dimmed prose
2. **Custom Tool Renders** — Registry + per-tool React components (start with 5: file read, file edit, bash, web search, knowledge search)
3. **Tool Streaming Detection** — Loading placeholder for in-flight tool calls
4. **Message Actions** — Copy, retry, delete (minimal set)

### Phase 2: Medium Effort

5. **Search Grounding** — Citation cards with favicons when web search is used
6. **Code Block Enhancement** — Syntax highlighting improvements, copy button per block, mermaid support
7. **File Upload** — Drag-drop area, preview thumbnails
8. **Input History** — Up/down arrow to cycle past messages

### Phase 3: High Effort

9. **Rich Text Editor** — Consider Lexical or TipTap for @mentions, file tags, action tags
10. **Follow-up Chips** — AI-suggested follow-up questions
11. **Chat Input Toolbar** — Search toggle, knowledge toggle, model label
12. **Message Branching** — Alternative response branches (if Oxios supports it)
