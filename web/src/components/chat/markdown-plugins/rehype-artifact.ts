// rehype-artifact — enhances fenced code blocks with artifact card styling.
//
// LobeHub analogue: Conversation/Markdown/plugins/LobeArtifact.
// Adds a wrapper <div class="artifact-card"> around <pre><code> blocks that
// have a language label, with the language as a badge and optional title
// extracted from a preceding heading or comment.
//
// The existing CodeBlock component in markdown-message.tsx already handles
// copy buttons + language labels. This plugin adds metadata to the hast so
// the component can optionally render a title/download affordance.
//
// Strategy: walk <pre> elements, look for a `language-*` class on the inner
// <code>, and add `data-artifact` + `data-language` attributes. The React
// component map reads these for richer rendering.

import type { Element, ElementContent, Root } from 'hast'
import type { Plugin } from 'unified'

export const rehypeArtifact: Plugin<[], Root> = () => {
  return (tree) => {
    visit(tree)
  }
}

function visit(node: Root | ElementContent): void {
  if (node.type !== 'element' && node.type !== 'root') return

  if (node.type === 'element' && node.tagName === 'pre') {
    enhancePre(node)
  }

  const children = (node as Element | Root).children
  if (Array.isArray(children)) {
    for (const c of children) visit(c as ElementContent)
  }
}

function enhancePre(pre: Element): void {
  // Find inner <code> with language class.
  const code = pre.children.find(
    (c): c is Element => c.type === 'element' && c.tagName === 'code',
  )
  if (!code) return

  const langClass = (code.properties?.className as string[] | undefined)?.find((c) =>
    c.startsWith('language-'),
  )
  if (!langClass) return

  const language = langClass.replace('language-', '')

  // Mark this <pre> as an artifact so the component map can enhance it.
  pre.properties = {
    ...pre.properties,
    className: [...((pre.properties?.className as string[]) ?? []), 'artifact-block'],
    dataLanguage: language,
    dataArtifact: 'true',
  }

  // Try to extract a title from the first comment in the code block.
  const title = extractTitleFromCode(code)
  if (title) {
    pre.properties.dataTitle = title
  }
}

function extractTitleFromCode(code: Element): string | undefined {
  const text = code.children
    .filter((c) => c.type === 'text')
    .map((c) => (c as { value: string }).value)
    .join('')

  // Check for common title patterns in first line.
  const firstLine = text.split('\n')[0] ?? ''

  // Pattern: "# Title" or "// Title" or "<!-- Title -->"
  const hashMatch = firstLine.match(/^#\s+(.+)/)
  if (hashMatch) return hashMatch[1]

  const commentMatch = firstLine.match(/^\/\/\s+(.+)/)
  if (commentMatch) return commentMatch[1]

  const htmlCommentMatch = firstLine.match(/^<!--\s*(.+?)\s*-->/)
  if (htmlCommentMatch) return htmlCommentMatch[1]

  return undefined
}
