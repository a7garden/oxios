import Placeholder from '@tiptap/extension-placeholder'
import { EditorContent, useEditor } from '@tiptap/react'
import StarterKit from '@tiptap/starter-kit'
import {
  BookOpen,
  Brain,
  Clock,
  FileText,
  HardDrive,
  Image,
  Paperclip,
  Send,
  Sparkles,
  Square,
  X,
} from 'lucide-react'
import { type DragEvent, useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { useIsTouch } from '@/hooks/use-is-touch'
import { useKnowledgeSearch } from '@/hooks/use-knowledge'
import { useMemorySemanticSearch } from '@/hooks/use-memory'
import { useMounts } from '@/hooks/use-mounts'
import { cn } from '@/lib/utils'
import { ModelPickerContainer } from './model-picker'

// ── Types ──

export interface AttachedFile {
  name: string
  size: number
  type: string
  dataUrl?: string
  content?: string
}

export interface ContextAttachment {
  type: 'knowledge' | 'memory' | 'file'
  id: string
  label: string
  snippet?: string
}

interface MentionResult {
  type: 'mount' | 'knowledge' | 'memory' | 'role'
  id: string
  label: string
  snippet: string
  score?: number
}

interface ChatInputProps {
  value: string
  onChange: (value: string) => void
  onSend: (content: string, contextItems: ContextAttachment[], files: AttachedFile[]) => void
  onCancel?: () => void
  disabled?: boolean
  isStreaming?: boolean
  connected?: boolean
  queuedCount?: number
  roles?: { name: string; model: string }[]
  activeRole?: string | null
  setActiveRole?: (role: string | null) => void
  activeModelId?: string | null
  setActiveModelId?: (id: string | null) => void
  activeMounts?: { id: string; label: string }[]
  onAttachMount?: (id: string) => void
  onRemoveMount?: (id: string) => void
  placeholder?: string
  showNewChatHint?: boolean
}

// ── Slash commands ──

interface SlashCommand {
  id: string
  label: string
  description: string
  icon: string
  action: (editor: ReturnType<typeof useEditor>) => void
}

const SLASH_COMMANDS: SlashCommand[] = [
  {
    id: 'compact',
    label: '/compact',
    description: 'Summarize the conversation to save context',
    icon: '📝',
    action: (ed) => ed?.commands.insertContent('/compact '),
  },
  {
    id: 'new-topic',
    label: '/new-topic',
    description: 'Start a new topic branch',
    icon: '🆕',
    action: (ed) => ed?.commands.insertContent('/new-topic '),
  },
  {
    id: 'clear',
    label: '/clear',
    description: 'Clear the current input',
    icon: '🗑️',
    action: (ed) => ed?.commands.clearContent(),
  },
]

// ── Component ──

export function ChatInput({
  value,
  onChange,
  onSend,
  onCancel,
  disabled,
  isStreaming,
  connected,
  queuedCount = 0,
  roles = [],
  activeRole = null,
  setActiveRole = () => {},
  activeModelId = null,
  setActiveModelId = () => {},
  activeMounts = [],
  onAttachMount = () => {},
  onRemoveMount = () => {},
  placeholder,
  showNewChatHint = true,
}: ChatInputProps) {
  const { t } = useTranslation()
  const isTouch = useIsTouch()

  // State
  const [contextAttachments, setContextAttachments] = useState<ContextAttachment[]>([])
  const [attachedFiles, setAttachedFiles] = useState<AttachedFile[]>([])
  const [isDragOver, setIsDragOver] = useState(false)
  const dragCounter = useRef(0)
  const maxFileSize = 10 * 1024 * 1024
  const [showSlashMenu, setShowSlashMenu] = useState(false)
  const [slashFilter, setSlashFilter] = useState('')
  const [mentionQuery, setMentionQuery] = useState<string | null>(null)
  const [mentionIndex, setMentionIndex] = useState(0)
  const [mentionResults, setMentionResults] = useState<MentionResult[]>([])
  const mentionSearchTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Search hooks
  const knowledgeSearch = useKnowledgeSearch()
  const memorySearch = useMemorySemanticSearch()
  const { data: mountsData } = useMounts()

  // Mention search
  const searchMentions = useCallback(
    async (query: string): Promise<MentionResult[]> => {
      const results: MentionResult[] = []
      try {
        const kRes = await knowledgeSearch.mutateAsync({ query, limit: 5 })
        for (const hit of kRes.results)
          results.push({
            type: 'knowledge',
            id: hit.path,
            label: hit.name,
            snippet: hit.snippet.slice(0, 80),
          })
      } catch {
        /* offline */
      }
      try {
        const mRes = await memorySearch.mutateAsync({ query, limit: 5 })
        for (const entry of mRes.entries)
          results.push({
            type: 'memory',
            id: entry.id,
            label: entry.key || entry.id.slice(0, 12),
            snippet: (entry.summary || entry.content).slice(0, 80),
            score: entry.score,
          })
      } catch {
        /* offline */
      }
      const mq = query.toLowerCase()
      for (const m of mountsData?.items ?? []) {
        if (m.name.toLowerCase().includes(mq) || m.auto_description.toLowerCase().includes(mq))
          results.push({
            type: 'mount',
            id: m.id,
            label: m.name,
            snippet: m.auto_description.slice(0, 80),
          })
      }
      for (const r of roles) {
        if (r.name.toLowerCase().includes(mq))
          results.push({ type: 'role', id: r.model, label: r.name, snippet: r.model })
      }
      const kindRank = (t: MentionResult['type']) => {
        switch (t) {
          case 'role':
            return 0
          case 'mount':
            return 1
          case 'knowledge':
            return 2
          default:
            return 3
        }
      }
      results.sort((a, b) => kindRank(a.type) - kindRank(b.type) || (b.score ?? 0) - (a.score ?? 0))
      return results.slice(0, 8)
    },
    [knowledgeSearch, memorySearch, mountsData, roles],
  )

  // Editor
  const editor = useEditor({
    extensions: [
      StarterKit.configure({ heading: false, codeBlock: false }),
      Placeholder.configure({
        placeholder:
          placeholder ?? (connected ? t('chat.inputPlaceholder') : t('chat.waitingForConnection')),
      }),
    ],
    content: value,
    editable: !disabled && !!connected,
    onUpdate: ({ editor }) => {
      const text = editor.getText()
      onChange(text)
      const anchor = editor.state.selection.anchor
      const textBefore = text.slice(0, anchor)
      // /commands
      const slashMatch = textBefore.match(/(?:^|\n)\/(\w*)$/)
      if (slashMatch) {
        setShowSlashMenu(true)
        setSlashFilter(slashMatch[1] || '')
      } else {
        setShowSlashMenu(false)
      }
      // @mentions
      const mentionMatch = textBefore.match(/@(\S*)$/)
      if (mentionMatch) {
        setMentionQuery(mentionMatch[1] || '')
      } else {
        setMentionQuery(null)
        setMentionResults([])
      }
    },
  })

  // Sync
  useEffect(() => {
    if (editor && value !== editor.getText()) editor.commands.setContent(value)
  }, [value, editor])

  // Mention search effect
  useEffect(() => {
    if (mentionQuery === null) {
      setMentionResults([])
      return
    }
    clearTimeout(mentionSearchTimer.current!)
    mentionSearchTimer.current = setTimeout(async () => {
      const results = await searchMentions(mentionQuery)
      setMentionResults(results)
      setMentionIndex(0)
    }, 200)
    return () => {
      clearTimeout(mentionSearchTimer.current!)
    }
  }, [mentionQuery, searchMentions])

  // File handling
  const readFile = useCallback(async (file: File): Promise<AttachedFile> => {
    const result: AttachedFile = { name: file.name, size: file.size, type: file.type }
    if (file.type.startsWith('image/')) {
      result.dataUrl = await new Promise<string>((resolve) => {
        const r = new FileReader()
        r.onload = () => resolve(r.result as string)
        r.readAsDataURL(file)
      })
    } else if (/\.(md|json|txt|csv|yml|yaml|toml|xml|log|rs|ts|js|py|html|css)$/i.test(file.name)) {
      result.content = await file.text()
    }
    return result
  }, [])
  const addFiles = useCallback(
    async (fileList: FileList | File[]) => {
      const files = Array.from(fileList)
        .filter((f) => f.size <= maxFileSize)
        .slice(0, 5)
      if (files.length === 0) return
      const results = await Promise.all(files.map(readFile))
      setAttachedFiles((prev) => [...prev, ...results].slice(0, 10))
    },
    [maxFileSize, readFile],
  )
  const removeFile = useCallback(
    (index: number) => setAttachedFiles((prev) => prev.filter((_, i) => i !== index)),
    [],
  )

  // Drag-drop
  const handleDragEnter = useCallback((e: DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    dragCounter.current++
    if (e.dataTransfer?.types.includes('Files')) setIsDragOver(true)
  }, [])
  const handleDragLeave = useCallback((e: DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    dragCounter.current--
    if (dragCounter.current <= 0) {
      dragCounter.current = 0
      setIsDragOver(false)
    }
  }, [])
  const handleDragOver = useCallback((e: DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
  }, [])
  const handleDrop = useCallback(
    (e: DragEvent) => {
      e.preventDefault()
      e.stopPropagation()
      dragCounter.current = 0
      setIsDragOver(false)
      if (e.dataTransfer?.files?.length) addFiles(e.dataTransfer.files)
    },
    [addFiles],
  )

  // Send
  const getContent = useCallback(() => editor?.getText() ?? '', [editor])
  const handleSend = useCallback(() => {
    const content = getContent()
    if (!content.trim() || !connected) return
    onSend(content, contextAttachments, attachedFiles)
    editor?.commands.clearContent()
    setContextAttachments([])
    setAttachedFiles([])
  }, [getContent, connected, contextAttachments, attachedFiles, onSend, editor])

  const canSend = editor?.getText().trim() && connected

  // Enter to send
  useEffect(() => {
    if (!editor) return
    const el = editor.view.dom
    const h = (e: Event) => {
      const ke = e as KeyboardEvent
      if (ke.key === 'Enter' && !ke.shiftKey && !isTouch && !showSlashMenu && !mentionQuery) {
        e.preventDefault()
        handleSend()
      }
    }
    el.addEventListener('keydown', h)
    return () => el.removeEventListener('keydown', h)
  }, [editor, isTouch, showSlashMenu, mentionQuery, handleSend])

  const filteredCommands = SLASH_COMMANDS.filter(
    (c) => c.id.includes(slashFilter) || c.label.includes(slashFilter),
  )

  return (
    <div className="w-full max-w-3xl mx-auto px-4 pb-4 pt-2 relative">
      {/* File chips */}
      {attachedFiles.length > 0 && (
        <div className="flex flex-wrap gap-1.5 mb-2">
          {attachedFiles.map((file, i) => (
            <span
              key={`${file.name}-${i}`}
              className="inline-flex items-center gap-1 rounded-full bg-blue-50 dark:bg-blue-950 border border-blue-200 dark:border-blue-800 px-2.5 py-0.5 text-xs text-blue-700 dark:text-blue-300"
            >
              {file.type.startsWith('image/') ? (
                <Image className="h-3 w-3" />
              ) : (
                <Paperclip className="h-3 w-3" />
              )}
              <span className="truncate max-w-[140px]">{file.name}</span>
              <button
                type="button"
                onClick={() => removeFile(i)}
                className="ml-0.5 -mr-1 rounded-full p-0.5 hover:bg-blue-200 dark:hover:bg-blue-800"
              >
                <X className="h-2.5 w-2.5" />
              </button>
            </span>
          ))}
        </div>
      )}
      {/* Context chips */}
      {(activeMounts.length > 0 || contextAttachments.length > 0) && (
        <div className="flex flex-wrap gap-1.5 mb-2">
          {activeMounts.map((m) => (
            <span
              key={`mount-${m.id}`}
              className="inline-flex items-center gap-1 rounded-full bg-primary/10 border border-primary/20 px-2.5 py-0.5 text-xs text-primary"
            >
              <HardDrive className="h-3 w-3" />
              <span className="truncate max-w-[140px]">{m.label}</span>
              <button
                type="button"
                onClick={() => onRemoveMount(m.id)}
                className="ml-0.5 -mr-1 rounded-full p-0.5 hover:bg-primary/20"
              >
                <X className="h-2.5 w-2.5" />
              </button>
            </span>
          ))}
          {contextAttachments.map((ctx) => (
            <span
              key={`${ctx.type}-${ctx.id}`}
              className="inline-flex items-center gap-1 rounded-full bg-muted/80 px-2.5 py-0.5 text-xs text-foreground"
            >
              {ctx.type === 'knowledge' ? (
                <BookOpen className="h-3 w-3 text-blue-500" />
              ) : (
                <Brain className="h-3 w-3 text-purple-500" />
              )}
              <span className="truncate max-w-[140px]">{ctx.label}</span>
              <button
                type="button"
                onClick={() => setContextAttachments((prev) => prev.filter((a) => a.id !== ctx.id))}
                className="ml-0.5 -mr-1 rounded-full p-0.5 hover:bg-muted-foreground/20"
              >
                <X className="h-2.5 w-2.5" />
              </button>
            </span>
          ))}
        </div>
      )}
      {/* @mention Popover */}
      {mentionQuery !== null && (
        <div className="absolute bottom-full left-4 right-4 z-50 mb-1 rounded-xl border bg-popover shadow-lg">
          <div className="p-1.5 max-h-64 overflow-y-auto">
            {mentionResults.length > 0 ? (
              mentionResults.map((result, idx) => (
                <button
                  key={`${result.type}-${result.id}`}
                  type="button"
                  onClick={() => {
                    if (result.type === 'mount') {
                      onAttachMount(result.id)
                    } else if (result.type === 'role') {
                      setActiveRole(result.label)
                    } else {
                      const ctx: ContextAttachment = {
                        type: result.type as 'knowledge' | 'memory',
                        id: result.id,
                        label: result.label,
                        snippet: result.snippet,
                      }
                      setContextAttachments((prev) =>
                        prev.some((a) => a.id === ctx.id && a.type === ctx.type)
                          ? prev
                          : [...prev, ctx],
                      )
                    }
                    setMentionQuery(null)
                    editor?.commands.focus()
                  }}
                  className={cn(
                    'flex items-start gap-2.5 w-full rounded-lg px-2.5 py-2 text-left transition-colors',
                    idx === mentionIndex
                      ? 'bg-accent text-accent-foreground'
                      : 'hover:bg-accent/50',
                  )}
                >
                  {result.type === 'mount' ? (
                    <HardDrive className="h-4 w-4 mt-0.5 shrink-0 text-emerald-500" />
                  ) : result.type === 'knowledge' ? (
                    <FileText className="h-4 w-4 mt-0.5 shrink-0 text-blue-500" />
                  ) : result.type === 'role' ? (
                    <Sparkles className="h-4 w-4 mt-0.5 shrink-0 text-amber-500" />
                  ) : (
                    <Brain className="h-4 w-4 mt-0.5 shrink-0 text-purple-500" />
                  )}
                  <div className="min-w-0 flex-1">
                    <p className="text-sm font-medium truncate">{result.label}</p>
                    {result.snippet && (
                      <p className="text-xs text-muted-foreground truncate">{result.snippet}</p>
                    )}
                  </div>
                  <span className="text-2xs text-muted-foreground/60 shrink-0 mt-0.5">
                    {result.type === 'mount'
                      ? 'Mount'
                      : result.type === 'knowledge'
                        ? 'KB'
                        : result.type === 'role'
                          ? 'Agent'
                          : 'Memory'}
                  </span>
                </button>
              ))
            ) : (
              <p className="px-2.5 py-3 text-xs text-muted-foreground text-center">
                {mentionQuery === '' ? 'Type to search...' : 'No results'}
              </p>
            )}
          </div>
        </div>
      )}
      {/* Slash command menu */}
      {showSlashMenu && (
        <div className="absolute bottom-full left-4 z-50 mb-1 rounded-xl border bg-popover shadow-lg w-64">
          <div className="p-1.5">
            {filteredCommands.map((cmd) => (
              <button
                key={cmd.id}
                type="button"
                onClick={() => {
                  cmd.action(editor)
                  setShowSlashMenu(false)
                }}
                className="flex items-center gap-2.5 w-full rounded-lg px-2.5 py-2 text-left hover:bg-accent/50 transition-colors"
              >
                <span className="text-sm">{cmd.icon}</span>
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium">{cmd.label}</p>
                  <p className="text-xs text-muted-foreground">{cmd.description}</p>
                </div>
              </button>
            ))}
          </div>
        </div>
      )}
      {/* Input */}
      <div
        onDragEnter={handleDragEnter}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        className={cn(
          'relative rounded-lg border bg-background shadow-sm transition-all',
          'focus-within:shadow-md focus-within:border-primary/40 focus-within:ring-1 focus-within:ring-ring/30',
          !connected && 'opacity-60',
          isStreaming && 'border-destructive/30',
          isDragOver && 'border-primary ring-2 ring-primary/30',
        )}
      >
        {isDragOver && (
          <div className="absolute inset-0 z-10 flex items-center justify-center rounded-lg bg-primary/5 backdrop-blur-[1px] pointer-events-none">
            <span className="text-sm text-primary font-medium">Drop files to attach</span>
          </div>
        )}
        <div className="px-4 py-3">
          <EditorContent
            editor={editor}
            className="prose prose-sm dark:prose-invert max-w-none [&_.ProseMirror]:outline-none [&_.ProseMirror]:min-h-[1.5em] [&_.ProseMirror]:max-h-[280px] [&_.ProseMirror]:overflow-y-auto [&_.ProseMirror_p.is-editor-empty:first-child::before]:text-muted-foreground/70 [&_.ProseMirror_p.is-editor-empty:first-child::before]:content-[attr(data-placeholder)] [&_.ProseMirror_p.is-editor-empty:first-child::before]:float-left [&_.ProseMirror_p.is-editor-empty:first-child::before]:pointer-events-none [&_.ProseMirror_p.is-editor-empty:first-child::before]:h-0"
          />
        </div>
        <div className="flex items-center justify-between gap-2 px-3 pb-2.5 pt-1.5">
          <div className="flex items-center gap-1.5 min-w-0 flex-1">
            <ModelPickerContainer
              activeModelId={activeModelId}
              setActiveModelId={setActiveModelId}
              roles={roles}
              activeRole={activeRole}
              setActiveRole={setActiveRole}
            />
          </div>
          <div className="flex items-center shrink-0 gap-1.5">
            {queuedCount > 0 && (
              <span className="mr-0.5 flex items-center gap-1 rounded-full bg-muted px-2 py-0.5 text-2xs text-muted-foreground">
                <Clock className="h-3 w-3" />
                {t('chat.queued', { count: queuedCount, defaultValue: '{{count}} queued' })}
              </span>
            )}
            {isStreaming && (
              <Button
                onClick={onCancel}
                variant="destructive"
                size="sm"
                className="h-8 rounded-lg px-3 text-xs gap-1.5"
              >
                <Square className="h-3 w-3 fill-current" />
                {t('chat.stop')}
              </Button>
            )}
            <Button
              onClick={handleSend}
              disabled={!canSend}
              size="icon"
              className={cn(
                'h-8 w-8 rounded-lg transition-all',
                canSend
                  ? 'bg-primary text-primary-foreground hover:bg-primary/90 shadow-sm'
                  : 'bg-muted text-muted-foreground',
              )}
            >
              <Send className="h-3.5 w-3.5" />
            </Button>
          </div>
        </div>
      </div>
      <div className="mt-1.5 flex items-center justify-center gap-3 text-2xs text-muted-foreground/70 hidden sm:flex">
        <Hint kbd="Enter" label={t('chat.send')} />
        <Hint kbd="Shift+Enter" label={t('chat.input.newline')} />
        {showNewChatHint && <Hint kbd="⌘⇧N" label={t('chat.newConversation')} />}
      </div>
    </div>
  )
}

function Hint({ kbd, label }: { kbd: string; label: string }) {
  return (
    <span className="inline-flex items-center gap-1">
      <kbd className="rounded border bg-muted/60 px-1.5 py-0.5 text-2xs font-mono text-muted-foreground">
        {kbd}
      </kbd>
      <span>{label}</span>
    </span>
  )
}
