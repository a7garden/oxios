// MarkdownPreview — shared rendered-markdown body used by portal views.
//
// GFM + raw HTML (sanitized) + syntax highlighting. Extracted so both the
// file-preview and document portal views render markdown identically without
// duplicating the rehype/remark plugin chain.

import ReactMarkdown from 'react-markdown'
import rehypeHighlight from 'rehype-highlight'
import rehypeRaw from 'rehype-raw'
import rehypeSanitize, { defaultSchema } from 'rehype-sanitize'
import remarkGfm from 'remark-gfm'
import { cn } from '@/lib/utils'

interface MarkdownPreviewProps {
  content: string
  className?: string
}

export function MarkdownPreview({ content, className }: MarkdownPreviewProps) {
  return (
    <div
      className={cn(
        'prose prose-sm dark:prose-invert max-w-none',
        // Keep markdown code blocks aligned with the surrounding monospace
        // font so they blend with code-mode previews used elsewhere.
        '[&_code]:font-mono [&_code]:text-xs [&_pre]:bg-muted/40 [&_pre]:p-3',
        className,
      )}
    >
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[
          [rehypeRaw, { allowDangerousHtml: true }],
          [rehypeSanitize, defaultSchema],
          rehypeHighlight,
        ]}
      >
        {content}
      </ReactMarkdown>
    </div>
  )
}
