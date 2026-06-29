import { describe, it, expect } from 'vitest'
import { EditorState } from '@codemirror/state'
import { markdown } from '@codemirror/lang-markdown'
import { ensureSyntaxTree } from '@codemirror/language'
import { buildImageDecorations, resolveImageUrl } from '@/lib/image-fold-extension'

describe('resolveImageUrl', () => {
  it('passes absolute http(s) URLs through', () => {
    expect(resolveImageUrl('https://a.b/x.png', 'notes')).toBe('https://a.b/x.png')
  })
  it('resolves a relative URL against the file directory', () => {
    expect(resolveImageUrl('media/x.png', 'notes')).toBe('/api/knowledge/asset/notes/media/x.png')
  })
  it('treats a leading slash as root-relative', () => {
    expect(resolveImageUrl('/media/x.png', 'notes')).toBe('/api/knowledge/asset/media/x.png')
  })
  it('round-trips percent-escaped segments', () => {
    expect(resolveImageUrl('a%20b.png', '')).toBe('/api/knowledge/asset/a%20b.png')
  })
})

describe('buildImageDecorations', () => {
  // Cursor sits at position 0 (line 1); the ±1 active region covers lines 1–2.
  function widgetAt(doc: string, fileDir: string) {
    const state = EditorState.create({ doc, extensions: [markdown()] })
    ensureSyntaxTree(state, state.doc.length)
    const set = buildImageDecorations(state, () => fileDir)
    const cursor = set.iter()
    while (cursor.value) {
      const w = cursor.value.spec?.widget
      if (w?.constructor?.name === 'ImageWidget') return w
      cursor.next()
    }
    return undefined
  }

  it('replaces an image with ImageWidget and resolves its URL', () => {
    const w = widgetAt('# Title\n\n![](media/x.png)', 'notes')
    expect(w).toBeTruthy()
    expect((w as { url: string }).url).toBe('/api/knowledge/asset/notes/media/x.png')
  })

  it('leaves the source visible on the active line', () => {
    expect(widgetAt('![](media/x.png)', 'notes')).toBeUndefined()
  })

  it('passes an absolute URL through unchanged', () => {
    const w = widgetAt('# Title\n\n![](https://a.b/x.png)', 'notes')
    expect((w as { url: string }).url).toBe('https://a.b/x.png')
  })
})
