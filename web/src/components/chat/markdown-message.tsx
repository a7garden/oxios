// MarkdownMessage — enhanced markdown renderer with syntax highlighting + copy button.
//
// Pipeline (Phase 6, 2026-07-21):
//   remark-gfm → rehype-raw → rehype-sanitize → rehype-highlight → rehype-thinking
//
// Security: rehype-raw parses model output as HTML (needed so <think> tags work).
// rehype-sanitize then strips dangerous constructs (event handlers, scripts,
// iframes) using a schema that still permits formatting + our thinking-block
// details/summary. Without sanitize, model output could execute arbitrary JS.

import { Check, Copy } from 'lucide-react'
import { type ComponentPropsWithoutRef, memo, useCallback, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import rehypeHighlight from 'rehype-highlight'
import rehypeRaw from 'rehype-raw'
import rehypeSanitize, { defaultSchema } from 'rehype-sanitize'
import remarkGfm from 'remark-gfm'
import type { Schema } from 'hast-util-sanitize'
import { cn } from '@/lib/utils'
import { rehypeThinking } from './markdown-plugins/rehype-thinking'

// ── Code block with language label + copy button ──────────────────

function CodeBlock({ language, children }: { language?: string; children: string }) {
  const [copied, setCopied] = useState(false)

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(children).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    })
  }, [children])

  return (
    <div className="group relative my-3 rounded-lg border bg-muted/50 overflow-hidden">
      <div className="flex items-center justify-between px-3 py-1.5 bg-muted border-b">
        <span className="text-xs text-muted-foreground font-mono">{language ?? 'text'}</span>
        <button
          type="button"
          onClick={handleCopy}
          className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors opacity-0 group-hover:opacity-100"
        >
          {copied ? <><Check className="w-3 h-3" />Copied</> : <><Copy className="w-3 h-3" />Copy</>}
        </button>
      </div>
      <pre className="overflow-x-auto p-3 text-xs leading-relaxed">
        <code className={`language-${language ?? 'text'} font-mono`}>{children}</code>
      </pre>
    </div>
  )
}

// ── External link ─────────────────────────────────────────────────

function ExternalLink({ href, children, ...props }: ComponentPropsWithoutRef<'a'>) {
  return (
    <a href={href} target="_blank" rel="noopener noreferrer" className="text-primary underline underline-offset-2 hover:opacity-80 transition-opacity" {...props}>
      {children}
    </a>
  )
}

// ── Inline code ───────────────────────────────────────────────────

function InlineCode({ children }: ComponentPropsWithoutRef<'code'>) {
  return <code className="px-1.5 py-0.5 rounded bg-muted text-[0.85em] font-mono">{children}</code>
}

// ── Sanitize schema (extending default) ───────────────────────────
//
// defaultSchema already strips scripts/event handlers. We extend it to:
//   • allow class names on code/pre (for syntax highlight + our CodeBlock)
//   • allow summary/details (for thinking-block rewrite)
//   • keep the safe-by-default denylist for iframes, embeds, etc.

const sanitizeSchema: Schema = {
  ...defaultSchema,
  attributes: {
    ...defaultSchema.attributes,
    code: [...(defaultSchema.attributes?.code ?? []), ['className']],
    pre: [...(defaultSchema.attributes?.pre ?? []), ['className']],
    span: [...(defaultSchema.attributes?.span ?? []), ['className']],
    div: [...(defaultSchema.attributes?.div ?? []), ['className']],
    details: [...(defaultSchema.attributes?.details ?? []), ['className']],
    summary: [...(defaultSchema.attributes?.summary ?? []), ['className']],
  },
  tagNames: [...(defaultSchema.tagNames ?? []), 'details', 'summary'],
}

// ── Component map for react-markdown ──────────────────────────────

const markdownComponents = {
  pre: ({ children }: ComponentPropsWithoutRef<'pre'>) => <>{children}</>,
  code({ className, children, ...props }: ComponentPropsWithoutRef<'code'> & { inline?: boolean }) {
    const inline = 'inline' in props ? (props as { inline?: boolean }).inline : false
    if (inline) return <InlineCode>{children}</InlineCode>

    const langMatch = /language-(\w+)/.exec(className ?? '')
    const language = langMatch ? langMatch[1] : undefined
    return <CodeBlock language={language}>{extractText(children)}</CodeBlock>
  },
  a: ExternalLink,
}

// ── Main ──────────────────────────────────────────────────────────

interface MarkdownMessageProps { children: string; className?: string }

export const MarkdownMessage = memo(function MarkdownMessage({ children, className }: MarkdownMessageProps) {
  return (
    <div className={cn('prose prose-sm dark:prose-invert max-w-none', className)}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[
          [rehypeRaw, { allowDangerousHtml: true }],
          [rehypeSanitize, sanitizeSchema],
          rehypeHighlight,
          rehypeThinking,
        ]}
        components={markdownComponents}
      >
        {children}
      </ReactMarkdown>
    </div>
  )
})

function extractText(node: React.ReactNode): string {
  if (typeof node === 'string') return node
  if (Array.isArray(node)) return node.map((c) => (typeof c === 'string' ? c : '')).join('')
  return String(node ?? '')
}
