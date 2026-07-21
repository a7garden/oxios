// markdown-plugins — rehype plugin that recognises reasoning/thinking HTML
// blocks emitted by some models (Claude, Gemini, Qwen) and rewrites them to
// <details class="thinking"> for collapsible inline rendering.
//
// LobeHub analogue: Conversation/Markdown/plugins/Thinking.ts.
//
// Recognised tag forms (case-insensitive):
//   <think>…</think>
//   <thinking>…</thinking>
//   <lobe-thinking>…</lobe-thinking>
//   <reasoning>…</reasoning>
//
// On match, the tag is renamed to <details> with class "thinking-block" so
// the existing Thinking component (or default details styling) renders it.

import type { Element, ElementContent, Root } from 'hast'
import type { Plugin } from 'unified'

const THINKING_TAGS = new Set([
  'think',
  'thinking',
  'lobe-thinking',
  'lobe_thinking',
  'reasoning',
])

export const rehypeThinking: Plugin<[], Root> = () => {
  return (tree) => {
    visit(tree)
  }
}

function visit(node: Root | ElementContent): void {
  if (node.type !== 'element' && node.type !== 'root') return

  if (node.type === 'element') {
    const tag = node.tagName.toLowerCase()
    if (THINKING_TAGS.has(tag)) {
      node.tagName = 'details'
      const existingClass = pickClass(node)
      node.properties = {
        ...node.properties,
        className: combineClass(existingClass, 'thinking-block'),
      }
      if (!hasSummary(node)) {
        const summary: Element = {
          type: 'element',
          tagName: 'summary',
          properties: { className: ['thinking-summary'] },
          children: [{ type: 'text', value: 'Thinking' }],
        }
        node.children = [summary as ElementContent, ...node.children]
      }
    }
  }

  const children = (node as Element | Root).children
  if (Array.isArray(children)) {
    for (const c of children) visit(c as ElementContent)
  }
}

function pickClass(node: Element): string | undefined {
  const v = node.properties?.className
  if (typeof v === 'string') return v
  if (Array.isArray(v)) return v.join(' ')
  return undefined
}

function combineClass(a: string | undefined, b: string): string[] {
  const parts = a ? a.split(/\s+/).filter(Boolean) : []
  parts.push(b)
  return parts
}

function hasSummary(node: Element): boolean {
  return node.children.some(
    (c): c is Element => c.type === 'element' && c.tagName === 'summary',
  )
}
