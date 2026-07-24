// Tests for the artifact detection pipeline.
//
// Verifies the security-critical guarantees:
//   1. Tag protocol — <lobeArtifact> inner code is captured as a substring at
//      the string level (pre-parse), so HTML content survives intact and is
//      rewritten to a fenced block whose code is text inside <code> (sanitize-
//      safe). Handles the streaming (unclosed-tag) case.
//   2. Language path — a fenced ```html block's code (incl. <script>) survives
//      the sanitize pipeline as text inside <code>.
//   3. Helper correctness — language mapping + title extraction.

import type { Element, Nodes, Root, Text } from 'hast'
import type { Schema } from 'hast-util-sanitize'
import rehypeRaw from 'rehype-raw'
import rehypeSanitize, { defaultSchema } from 'rehype-sanitize'
import remarkParse from 'remark-parse'
import remarkRehype from 'remark-rehype'
import { unified } from 'unified'
import { describe, expect, it } from 'vitest'
import { preprocessArtifacts } from '@/components/chat/markdown-plugins/preprocess-artifacts'
import {
  ArtifactType,
  languageToArtifactType,
  parseArtifactCode,
  tagTypeToArtifactType,
} from '@/types/artifact'

// Mirror of markdown-message.tsx's sanitizeSchema.
const schema: Schema = {
  ...defaultSchema,
  attributes: {
    ...defaultSchema.attributes,
    code: [...(defaultSchema.attributes?.code ?? []), ['className']],
    pre: [...(defaultSchema.attributes?.pre ?? []), ['className']],
  },
}

/** Run remark → rehype (the same plugin order as markdown-message) on md. */
function runPipeline(md: string): Root {
  const processor = unified()
    .use(remarkParse)
    .use(remarkRehype, { allowDangerousHtml: true })
    .use(rehypeRaw)
    .use(rehypeSanitize, schema)
  return processor.runSync(processor.parse(md)) as Root
}

/** Recursively concatenate text-node values under a hast node. */
function textOf(node: Nodes): string {
  if (node.type === 'text') return (node as Text).value
  if ('children' in node && Array.isArray(node.children)) {
    return node.children.map((c) => textOf(c)).join('')
  }
  return ''
}

/** Find the first <code> element and return its text + language class. */
function findCode(root: Root): { text: string; lang?: string } | undefined {
  let found: { text: string; lang?: string } | undefined
  walk(root)
  return found

  function walk(node: Nodes): void {
    if (found) return
    if (node.type === 'element') {
      const el = node as Element
      if (el.tagName === 'code') {
        const cls = (el.properties?.className as string[] | undefined) ?? []
        const lang = cls.find((c) => c.startsWith('language-'))?.replace('language-', '')
        found = { text: textOf(el), lang }
        return
      }
    }
    if ('children' in node && Array.isArray(node.children)) {
      for (const c of node.children) walk(c)
    }
  }
}

describe('artifact language detection (helpers)', () => {
  it('maps renderable languages to a type', () => {
    expect(languageToArtifactType('html')).toBe(ArtifactType.Html)
    expect(languageToArtifactType('svg')).toBe(ArtifactType.Svg)
    expect(languageToArtifactType('mermaid')).toBe(ArtifactType.Mermaid)
    expect(languageToArtifactType('tsx')).toBe(ArtifactType.React)
  })

  it('returns undefined for non-renderable languages', () => {
    expect(languageToArtifactType('bash')).toBeUndefined()
    expect(languageToArtifactType(undefined)).toBeUndefined()
  })

  it('resolves tag-protocol MIME-ish types', () => {
    expect(tagTypeToArtifactType('image/svg+xml')).toBe(ArtifactType.Svg)
    expect(tagTypeToArtifactType('application/lobe.artifacts.react')).toBe(ArtifactType.React)
    expect(tagTypeToArtifactType('html')).toBe(ArtifactType.Html)
  })
})

describe('parseArtifactCode', () => {
  it('extracts a leading # title and strips it from content', () => {
    const out = parseArtifactCode('# My Widget\n<div>hi</div>')
    expect(out.title).toBe('My Widget')
    expect(out.content).toBe('<div>hi</div>')
  })

  it('extracts a // comment title', () => {
    expect(parseArtifactCode('// Demo\nfoo').title).toBe('Demo')
  })

  it('returns content unchanged when no directive is present', () => {
    expect(parseArtifactCode('<svg></svg>').title).toBeUndefined()
  })
})

describe('preprocessArtifacts (tag protocol → fenced block)', () => {
  it('rewrites a closed SVG artifact, preserving inner HTML as a string', () => {
    const md =
      '<lobeArtifact type="image/svg+xml" title="Demo"><svg onload="alert(1)"><rect/></svg></lobeArtifact>'
    const out = preprocessArtifacts(md)
    expect(out).toContain('```svg')
    // The SVG survives intact (it was a substring, never parsed as HTML).
    expect(out).toContain('<svg onload="alert(1)"><rect/></svg>')
    // Title is encoded as a directive line.
    expect(out).toContain('# Demo')
    expect(out).not.toContain('<lobeArtifact')
  })

  it('preserves a <script> inside an HTML artifact tag as a string', () => {
    const md = '<lobeArtifact type="text/html"><p>x</p><script>alert(1)</script></lobeArtifact>'
    const out = preprocessArtifacts(md)
    expect(out).toContain('```html')
    expect(out).toContain('<script>alert(1)</script>')
    expect(out).toContain('<p>x</p>')
  })

  it('captures the unclosed tag for streaming (to end of input)', () => {
    const md = 'intro\n<lobeArtifact type="image/svg+xml"><svg><circle/></svg>'
    const out = preprocessArtifacts(md)
    expect(out).toContain('```svg')
    expect(out).toContain('<svg><circle/></svg>')
    expect(out).not.toContain('<lobeArtifact')
  })

  it('leaves non-artifact content untouched', () => {
    const md = 'Hello ```bash\nls\n``` world'
    expect(preprocessArtifacts(md)).toBe(md)
  })
})

describe('language-path: fenced code survives sanitize as text', () => {
  it('preserves HTML including <script> as a string inside <code>', () => {
    const tree = runPipeline('```html\n<div>hi</div>\n<script>alert(1)</script>\n```')
    const code = findCode(tree)
    expect(code?.lang).toBe('html')
    // Code text survives intact (text is never stripped by sanitize).
    expect(code?.text).toContain('alert(1)')
    expect(code?.text).toContain('<script>alert(1)</script>')
  })

  it('preserves a preprocessed artifact through the full pipeline', () => {
    const md = preprocessArtifacts(
      '<lobeArtifact type="image/svg+xml" title="D"><svg onload="x()"/></lobeArtifact>',
    )
    const tree = runPipeline(md)
    const code = findCode(tree)
    expect(code?.lang).toBe('svg')
    expect(code?.text).toContain('<svg onload="x()"/>')
  })
})
