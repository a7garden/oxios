// rehype-link-card — enhances standalone links with OpenGraph-style preview cards.
//
// LobeHub analogue: Conversation/Markdown/plugins/Link.
// When a paragraph contains ONLY a link (no other text), the link is wrapped
// in a <div class="link-card"> for card-style rendering by the component map.
//
// The actual OpenGraph fetch is NOT done client-side (too slow for streaming).
// Instead, the card displays the URL domain + path as a preview, and the
// user can click through. A future enhancement could prefetch OG metadata.

import type { Element, ElementContent, Root } from 'hast'
import type { Plugin } from 'unified'

export const rehypeLinkCard: Plugin<[], Root> = () => {
  return (tree) => {
    visit(tree)
  }
}

function visit(node: Root | ElementContent): void {
  if (node.type !== 'element' && node.type !== 'root') return

  // Process <p> elements that contain only a single link.
  if (node.type === 'element' && node.tagName === 'p') {
    const linkOnly = isLinkOnlyParagraph(node)
    if (linkOnly) {
      const link = node.children.find(
        (c): c is Element => c.type === 'element' && c.tagName === 'a',
      )
      if (link) {
        const href = (link.properties?.href as string) ?? ''
        if (isExternalUrl(href)) {
          // Wrap in a link-card div for the component map to pick up.
          const card: Element = {
            type: 'element',
            tagName: 'div',
            properties: {
              className: ['link-card'],
              dataUrl: href,
              dataDomain: extractDomain(href),
            },
            children: [link],
          }
          node.children = [card as ElementContent]
        }
      }
    }
  }

  const children = (node as Element | Root).children
  if (Array.isArray(children)) {
    for (const c of children) visit(c as ElementContent)
  }
}

function isLinkOnlyParagraph(p: Element): boolean {
  // A paragraph is "link only" if it has exactly one <a> child
  // and optionally whitespace text nodes.
  const meaningful = p.children.filter(
    (c) =>
      c.type === 'element' ||
      (c.type === 'text' && (c as { value: string }).value.trim().length > 0),
  )
  return meaningful.length === 1 && meaningful[0]!.type === 'element' && (meaningful[0] as Element).tagName === 'a'
}

function isExternalUrl(href: string): boolean {
  return href.startsWith('http://') || href.startsWith('https://')
}

function extractDomain(url: string): string {
  try {
    return new URL(url).hostname
  } catch {
    return url.slice(0, 40)
  }
}
