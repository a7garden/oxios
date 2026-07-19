# LobeHub → Oxios: Chat UX Implementation Design

> Strategy: **Copy structure/logic directly, replace styling with Tailwind.**
> LobeHub's external UI dependency surface is surprisingly small: just Flexbox, Accordion, ScrollArea, Markdown.
> Everything else is React hooks + Zustand stores + rehype/remark plugins — all directly portable.

## 0. Dependency Translation Table

| LobeHub Import | Oxios Equivalent | Effort |
|---------------|-----------------|--------|
| `Flexbox` from `@lobehub/ui` | `<div className="flex flex-col gap-{n}">` | Trivial |
| `Accordion, AccordionItem` from `@lobehub/ui` | shadcn/ui `<Accordion>` (already have it) | Trivial |
| `ScrollArea` from `@lobehub/ui` | shadcn/ui `<ScrollArea>` (already have it) | Trivial |
| `createStaticStyles` from `antd-style` | Tailwind classes or `cn()` utility | Mechanical |
| `cssVar.colorTextDescription` etc. | Tailwind's `text-muted-foreground` etc. | Mechanical |
| `cx` from `antd-style` | `cn()` from `clsx` + `tailwind-merge` (already have it) | Trivial |
| `Markdown` from `@lobehub/ui` | Our existing `ReactMarkdown` with plugins | Already done |
| `@lobechat/types` | Copy types into `web/src/types/chat.ts` | One-time |
| `@lobechat/const` | Copy constants into `web/src/const/` | One-time |
| `@lobechat/utils` | Copy needed utils inline or as local helpers | Small |
| `ChatInputActions` from `@lobehub/editor` | Skip for now — use our plain textarea | Deferred |

## 1. New File Map

All new files under `web/src/components/chat/`:

```
web/src/components/chat/
├── message-bubble.tsx         # UPDATED: becomes thin dispatcher
├── chat-input.tsx             # KEPT: our input, add ActionBar later
├── model-picker.tsx           # KEPT
│
├── chat-item/                 # NEW: universal message wrapper (from LobeHub ChatItem)
│   ├── index.tsx              #   Main ChatItem component
│   ├── style.ts               #   Tailwind variants (avatar placement, bubble, hover actions)
│   ├── types.ts               #   ChatItemProps
│   ├── components/
│   │   ├── Avatar.tsx          #   Agent avatar + fallback
│   │   ├── Title.tsx           #   Name + timestamp row
│   │   ├── ErrorContent.tsx    #   Inline error display
│   │   └── Actions.tsx         #   Hover-revealed action bar (copy/retry/delete)
│   │
├── messages/                  # NEW: per-role message renderers
│   ├── index.tsx              #   Role dispatcher (switch on message.role)
│   ├── Assistant.tsx          #   Content pipeline: Reasoning → Search → FileChunks → Display → Images
│   ├── User.tsx               #   User message in ChatItem
│   ├── Tool.tsx               #   Tool result in accordion
│   ├── AssistantGroup.tsx     #   Multi-step agent turns (deferred)
│   └── components/
│       ├── DisplayContent.tsx  #   Renders markdown or RichContent
│       ├── ContentLoading.tsx  #   Streaming indicator with elapsed time
│       ├── SearchGrounding.tsx #   Citation cards + image search results
│       ├── FileChunks.tsx      #   RAG reference chunks
│       ├── ImageFileListViewer.tsx #  Image gallery
│       └── Reasoning.tsx       #   Thin wrapper → Thinking
│
├── thinking/                  # NEW: collapsible reasoning block
│   ├── index.tsx              #   Accordion + auto-scroll + animated streaming
│   ├── Title.tsx              #   "Thinking · 2.3s" with status dot
│   └── StatusIndicator.tsx    #   Animated dot during streaming
│
├── tool-call-card.tsx         # REPLACED: becomes generic fallback
├── tool-renders/              # NEW: custom tool render registry
│   ├── registry.ts            #   Map<string, ToolRenderComponent>
│   ├── types.ts               #   ToolRenderProps, ToolInspectorProps
│   ├── FileRead.tsx           #   File path + content preview with syntax highlight
│   ├── FileEdit.tsx           #   Diff view
│   ├── Bash.tsx               #   Terminal output with syntax highlight
│   ├── WebSearch.tsx          #   Search results with favicons
│   └── DefaultTool.tsx        #   Fallback: expandable JSON args + result
│
├── markdown/                  # NEW: markdown plugin system (port from LobeHub)
│   ├── index.tsx              #   MarkdownMessage wrapper
│   ├── useChatMarkdown.ts     #   Plugin assembly hook
│   └── plugins/               #   Port from LobeHub's Conversation/Markdown/plugins/
│       ├── index.ts           #   Plugin registry
│       ├── Thinking.ts        #   Rehype plugin for thinking blocks
│       ├── LobeArtifact.ts    #   Artifact card plugin
│       ├── Tool.tsx           #   Tool call inline render
│       ├── Mention.tsx        #   @mention rendering
│       └── Link.tsx           #   Link card preview
│
├── activity-timeline.tsx      # KEPT: moves ABOVE chat (preamble)
├── activity-card.tsx          # KEPT
├── live-activity-bar.tsx      # KEPT
├── chat-metadata.tsx          # KEPT
└── knowledge-save-indicator.tsx # KEPT
```

## 2. Component Porting: Concrete Examples

### 2.1 ChatItem (universal message wrapper)

**LobeHub original** (`ChatItem.tsx`):
```tsx
import { Flexbox } from '@lobehub/ui';
import { cx } from 'antd-style';

// Uses createStaticStyles for: container layout, avatar placement,
// message content, error, actions hover reveal, loading dot
```

**Oxios port** (`chat-item/index.tsx`):
```tsx
// No @lobehub/ui — just React + Tailwind
import { cn } from '@/lib/utils';
import { Avatar } from './components/Avatar';
import { Title } from './components/Title';
import { ErrorContent } from './components/ErrorContent';
import { Actions } from './components/Actions';
import { MessageContent } from './components/MessageContent';
// ^ all internal, no external UI lib dependency

interface ChatItemProps {
  avatar: AgentAvatar;
  placement?: 'left' | 'right';
  loading?: boolean;
  error?: ChatError;
  time?: number;
  showTitle?: boolean;
  actions?: ReactNode;
  children: ReactNode;
  // ...
}

export function ChatItem({ placement = 'left', ... }: ChatItemProps) {
  return (
    <div
      className={cn(
        'group flex gap-3 px-4 py-2',
        placement === 'right' && 'flex-row-reverse'
      )}
    >
      {/* Avatar column */}
      <Avatar {...avatar} className="shrink-0 mt-1" />

      {/* Content column */}
      <div className="flex-1 min-w-0">
        {/* Title row: name + time — hidden until hover */}
        {showTitle && (
          <div className="flex items-center gap-2 mb-1 opacity-0 group-hover:opacity-100 transition-opacity">
            <span className="text-sm font-medium">{avatar.name}</span>
            <span className="text-xs text-muted-foreground">{formatTime(time)}</span>
          </div>
        )}

        {/* Error display */}
        {error && <ErrorContent error={error} />}

        {/* Message body */}
        <MessageContent loading={loading} placement={placement}>
          {children}
        </MessageContent>

        {/* Actions — hidden until hover */}
        <div className="opacity-0 group-hover:opacity-100 transition-opacity mt-1">
          {actions ?? <Actions />}
        </div>
      </div>
    </div>
  );
}
```

**Translation notes:**
- `Flexbox gap={8}` → `flex gap-2` (8px = Tailwind gap-2)
- `opacity: 0` on hover reveal → `opacity-0 group-hover:opacity-100`
- `cx(styles.xxx, className)` → `cn('tailwind-classes', className)`
- All antd-style `createStaticStyles` blocks → Tailwind utility classes

### 2.2 Thinking Block

**LobeHub original** (`Thinking/index.tsx`):
```tsx
import { Accordion, AccordionItem, ScrollArea } from '@lobehub/ui';
import { createStaticStyles } from 'antd-style';

const styles = createStaticStyles(({ css, cssVar }) => ({
  contentScroll: css`
    max-height: min(40vh, 320px);
    color: ${cssVar.colorTextDescription};
  `,
}));

<Accordion>
  <AccordionItem title={<Title thinking={thinking} duration={duration} />}>
    <ScrollArea className={styles.contentScroll}>
      <MarkdownMessage content={content} />
    </ScrollArea>
  </AccordionItem>
</Accordion>
```

**Oxios port** (`thinking/index.tsx`):
```tsx
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion'; // shadcn/ui
import { ScrollArea } from '@/components/ui/scroll-area'; // shadcn/ui
import { MarkdownMessage } from '../markdown';
import { ThinkingTitle } from './Title';

interface ThinkingProps {
  content: string;
  thinking: boolean;      // true during streaming → auto-expand
  duration?: number;      // elapsed seconds
}

export function Thinking({ content, thinking, duration }: ThinkingProps) {
  // Auto-expand during streaming, collapse after
  const [open, setOpen] = useState(thinking);
  useEffect(() => { setOpen(thinking); }, [thinking]);

  return (
    <Accordion type="single" collapsible value={open ? 'thinking' : ''} onValueChange={v => setOpen(v === 'thinking')}>
      <AccordionItem value="thinking" className="border-0">
        <AccordionTrigger className="py-1 hover:no-underline">
          <ThinkingTitle thinking={thinking} duration={duration} />
        </AccordionTrigger>
        <AccordionContent>
          <ScrollArea className="max-h-[min(40vh,320px)]">
            <div className="px-2 pb-2 text-muted-foreground text-sm">
              <MarkdownMessage content={content} />
            </div>
          </ScrollArea>
        </AccordionContent>
      </AccordionItem>
    </Accordion>
  );
}
```

### 2.3 Assistant Message Content Pipeline

**LobeHub original** (`MessageContent.tsx`) — the pipeline:
```tsx
<Flexbox gap={8}>
  {showReasoning && <Reasoning {...reasoning} />}
  {showSearch && <SearchGrounding search={search} />}
  {showFileChunks && <FileChunks chunksList={chunksList} />}
  <DisplayContent content={content} generating={generating} markdownProps={markdownProps} />
  {showImageItems && <ImageFileListViewer imageList={imageList} />}
  <ReactionDisplay reactions={reactions} onClick={handleReactionClick} />
</Flexbox>
```

**Oxios port** — **identical logic, different wrapper**:
```tsx
// messages/Assistant.tsx
export function AssistantMessage({ id, ... }: AssistantMessageProps) {
  const message = useChatStore(s => s.messages[id]);
  const generating = useChatStore(s => s.generating.has(id));
  const isReasoning = useChatStore(s => s.reasoning.has(id));

  return (
    <ChatItem avatar={message.agent} error={message.error} time={message.timestamp}>
      <div className="flex flex-col gap-2">
        {/* Exact same conditional pipeline as LobeHub */}
        {(message.reasoning || isReasoning) && (
          <Reasoning
            content={message.reasoning?.content ?? ''}
            thinking={isReasoning}
            duration={message.reasoning?.duration}
          />
        )}
        {message.search && <SearchGrounding search={message.search} />}
        {message.chunksList?.length > 0 && <FileChunks chunksList={message.chunksList} />}
        <DisplayContent
          content={message.content}
          generating={generating}
          markdownProps={markdownProps}
        />
        {message.imageList?.length > 0 && <ImageFileListViewer imageList={message.imageList} />}
      </div>
    </ChatItem>
  );
}
```

### 2.4 Tool Render Registry

LobeHub has `registerBuiltinRenders()` — we need the same pattern but simpler:

```typescript
// tool-renders/registry.ts
import type { ComponentType } from 'react';

interface ToolRenderProps {
  toolName: string;
  args: Record<string, unknown>;
  result: unknown;
  isRunning: boolean;
  durationMs?: number;
}

type ToolRenderComponent = ComponentType<ToolRenderProps>;

const registry = new Map<string, ToolRenderComponent>();

export function registerToolRender(toolName: string, component: ToolRenderComponent) {
  registry.set(toolName, component);
}

export function getToolRender(toolName: string): ToolRenderComponent | undefined {
  return registry.get(toolName);
}

// Register built-in renders
import { FileReadRender } from './FileRead';
import { FileEditRender } from './FileEdit';
import { BashRender } from './Bash';
import { WebSearchRender } from './WebSearch';
import { DefaultToolRender } from './DefaultTool';

registerToolRender('read', FileReadRender);
registerToolRender('write', FileEditRender);
registerToolRender('edit', FileEditRender);
registerToolRender('bash', BashRender);
registerToolRender('web_search', WebSearchRender);
// All others fall through to DefaultToolRender
```

**Usage in ToolCallCard** (replaces current single card):
```tsx
export function ToolCallCard({ toolName, args, result, isRunning, durationMs }: ToolCallCardProps) {
  const Render = getToolRender(toolName) ?? DefaultToolRender;

  return (
    <Accordion type="single" collapsible defaultValue="tool">
      <AccordionItem value="tool" className="border rounded-lg px-3">
        <AccordionTrigger className="py-2 hover:no-underline">
          <div className="flex items-center gap-2 text-sm">
            <Wrench className="w-4 h-4 text-muted-foreground" />
            <span className="font-medium">{toolName}</span>
            {isRunning && <Loader2 className="w-3 h-3 animate-spin" />}
            {durationMs != null && (
              <span className="text-xs text-muted-foreground ml-auto">
                {formatDuration(durationMs)}
              </span>
            )}
          </div>
        </AccordionTrigger>
        <AccordionContent>
          <Render toolName={toolName} args={args} result={result} isRunning={isRunning} durationMs={durationMs} />
        </AccordionContent>
      </AccordionItem>
    </Accordion>
  );
}
```

### 2.5 Markdown Plugins (Direct Port)

LobeHub's 14 rehype/remark plugins are **framework-agnostic** — they manipulate the AST and return React components. They can be copied almost verbatim:

```typescript
// markdown/plugins/Thinking.ts — PORTED FROM LobeHub
// Original: /tmp/lobehub/src/features/Conversation/Markdown/plugins/Thinking

import type { Element } from 'hast';
import type { Plugin } from 'unified';
import { visit } from 'unist-util-visit';

// LobeHub uses 'lobe-thinking' custom tag in markdown
// We keep the exact same AST transformation logic
export const rehypeThinking: Plugin = () => {
  return (tree) => {
    visit(tree, 'element', (node: Element) => {
      if (node.tagName === 'lobe-thinking') {
        // Replace with our Thinking component
        node.tagName = 'thinking-block';
        // ... same logic as LobeHub
      }
    });
  };
};
```

The only change: the React component that renders the transformed AST node uses Tailwind instead of antd-style.

## 3. What We DON'T Port (Yet)

| LobeHub Feature | Reason |
|----------------|--------|
| Rich text editor (Lexical) | Plain textarea is sufficient; Lexical adds 200KB+ bundle |
| Message branching | Oxios doesn't have this concept yet |
| Emoji reactions | Nice-to-have, not core |
| Follow-up chips | Requires AI-suggested follow-ups; deferred |
| Chat minimap | Nice-to-have for long threads |
| Group agent orchestration UI | Requires multi-agent support |
| WeChat/Feishu/QQ adapters | Oxios is personal, not multi-platform bot |
| Mermaid diagrams | Can add via rehype-mermaid plugin later |
| HTML preview drawer | Edge case |

## 4. Implementation Order

### Phase 1: Foundation (Day 1-2)
1. Copy `@lobechat/types` chat-related types → `web/src/types/chat.ts`
2. Copy `@lobechat/const` chat constants → `web/src/const/chat.ts`
3. Create `ChatItem` component (Tailwind port of LobeHub ChatItem)
4. Create `Thinking` block (Tailwind port of LobeHub Thinking)

### Phase 2: Message Pipeline (Day 2-3)
5. Create `messages/` role dispatcher + `Assistant.tsx` content pipeline
6. Port `DisplayContent`, `ContentLoading` from LobeHub → Tailwind
7. Wire up Zustand stores with the same selector pattern

### Phase 3: Tool Renders (Day 3-4)
8. Create `tool-renders/registry.ts`
9. Port 4 custom tool renders: FileRead, FileEdit, Bash, WebSearch
10. Update `ToolCallCard` to use registry + Accordion

### Phase 4: Markdown Plugins (Day 4-5)
11. Port 5 core markdown plugins: Thinking, Artifact, Tool, Mention, Link
12. Create `MarkdownMessage` wrapper with plugin assembly

### Phase 5: Polish (Day 5-6)
13. Port `SearchGrounding` (citations + image results)
14. Port `FileChunks` (RAG references)
15. Port error display variants (BaseErrorForm, PlanLimitCard)
16. Add hover actions to ChatItem (copy/retry/delete)

## 5. Estimated Code Volume

| Component | From LobeHub (LOC) | New Oxios (LOC) | Reuse % |
|-----------|-------------------|-----------------|---------|
| ChatItem | ~100 | ~120 | 80% logic, 100% new styling |
| Thinking | ~80 | ~60 | 90% logic, 100% new styling |
| MessageContent | ~80 | ~60 | 95% logic |
| DisplayContent | ~40 | ~30 | 90% logic |
| ContentLoading | ~60 | ~40 | 85% logic |
| SearchGrounding | ~200 | ~150 | 80% logic |
| Tool renders (4) | ~400 | ~300 | 70% logic |
| Markdown plugins (5) | ~300 | ~250 | 90% logic |

**Total: ~1,260 lines from LobeHub logic → ~1,010 lines in Oxios.**
Plus ~200 lines of Tailwind config/design tokens.

The logic is an 80-95% port. The styling is a full rewrite, but Tailwind is concise — most `createStaticStyles` blocks become 2-4 utility classes.
