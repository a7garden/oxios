// MarkdownMessage — enhanced markdown renderer with syntax highlighting + copy button

'use client'

import { Check, Copy } from 'lucide-react'
import { type ComponentPropsWithoutRef, memo, useCallback, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import rehypeHighlight from 'rehype-highlight'
import remarkGfm from 'remark-gfm'
import { cn } from '@/lib/utils'

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

// ── Component map for react-markdown ──────────────────────────────

const markdownComponents = {
  pre: ({ children }: ComponentPropsWithoutRef<'pre'>) => <>{children}</>,
  code({ className, children, ...props }: ComponentPropsWithoutRef<'code'> & { inline?: boolean }) {
    // react-markdown v9+ provides `inline` prop on code elements
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
        rehypePlugins={[rehypeHighlight]}
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
