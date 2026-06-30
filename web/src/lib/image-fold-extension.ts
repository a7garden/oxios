/**
 * Image fold extension for CodeMirror 6.
 *
 * Replaces HyperMD's `fold-image`: a `![alt](url)` on an inactive line is
 * rendered inline as an `<img>` instead of the raw markdown. Relative URLs
 * resolve against the current note's directory to the backend asset route
 * (`/api/knowledge/asset/…`); absolute http(s)/data/blob URLs pass through.
 *
 * The asset route serves bytes via the knowledge base's sandboxed path
 * resolution (same `safe_path` → `verify_under_root` as the file route), so a
 * knowledge-local `![](media/x.png)` actually loads.
 *
 * Robustness: a load error degrades to the browser's broken-image state (the
 * editor never crashes). The widget is active-region gated (±1 of the match's
 * line range) so the source stays editable while the cursor is on it.
 *
 * Round-trip safe: purely visual; the markdown source is preserved.
 */
import { syntaxTree } from '@codemirror/language'
import type { EditorState, Range } from '@codemirror/state'
import {
  Decoration,
  type DecorationSet,
  EditorView,
  type EditorView as EditorViewType,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from '@codemirror/view'

class ImageWidget extends WidgetType {
  constructor(
    readonly url: string,
    readonly alt: string,
  ) {
    super()
  }
  eq(other: ImageWidget) {
    return other.url === this.url && other.alt === this.alt
  }
  toDOM() {
    const img = document.createElement('img')
    img.src = this.url
    img.alt = this.alt
    img.className = 'ox-md-image'
    img.loading = 'lazy'
    // A failed load degrades to a broken-image state, never a crash.
    img.addEventListener('error', () => img.classList.add('ox-md-image-broken'))
    return img
  }
  ignoreEvent() {
    return false
  }
}

/**
 * Resolve a markdown image URL to something the browser can fetch. Absolute
 * http(s)/data/blob/cmd URLs pass through; everything else is treated as a
 * knowledge-relative path. A leading `/` is root-relative (file dir ignored).
 */
export function resolveImageUrl(rawUrl: string, fileDir: string): string {
  if (/^(https?:|data:|blob:|cmd:)/i.test(rawUrl)) return rawUrl
  const clean = rawUrl.replace(/^\/+/, '')
  const decoded = decodeURIComponent(clean)
  const full = rawUrl.startsWith('/') || !fileDir ? decoded : `${fileDir}/${decoded}`
  return `/api/knowledge/asset/${encodeURI(full)}`
}

export function buildImageDecorations(state: EditorState, getFileDir: () => string): DecorationSet {
  const builder: Range<Decoration>[] = []
  const { doc } = state
  const cursorLine = doc.lineAt(state.selection.main.head).number
  const fileDir = getFileDir()

  syntaxTree(state).iterate({
    enter(node) {
      if (node.name !== 'Image') return
      const fromLine = doc.lineAt(node.from).number
      const toLine = doc.lineAt(node.to).number
      // Active region (±1 of any spanned line): leave source editable.
      if (cursorLine >= fromLine - 1 && cursorLine <= toLine + 1) return
      const text = state.doc.sliceString(node.from, node.to)
      const urlMatch = text.match(/\]\(([^)\s]+)(?:\s+"[^"]*")?\)/)
      if (!urlMatch?.[1]) return
      const altMatch = text.match(/^!\[([\s\S]*)\]/)
      const url = resolveImageUrl(urlMatch[1], fileDir)
      const alt = altMatch?.[1] ?? ''
      builder.push(
        Decoration.replace({ widget: new ImageWidget(url, alt) }).range(node.from, node.to),
      )
    },
  })

  builder.sort((a, b) => a.from - b.from)
  return Decoration.set(builder)
}

/** Factory: binds a `getFileDir` getter (the current note's directory) so the
 * extension reads the latest path on each rebuild without being recreated. */
export function createImageFoldExtension(getFileDir: () => string) {
  return ViewPlugin.fromClass(
    class {
      decorations: DecorationSet
      constructor(view: EditorViewType) {
        this.decorations = buildImageDecorations(view.state, getFileDir)
      }
      update(update: ViewUpdate) {
        if (update.docChanged || update.selectionSet || update.viewportChanged) {
          this.decorations = buildImageDecorations(update.view.state, getFileDir)
        }
      }
    },
    {
      decorations: (v) => v.decorations,
      provide: () =>
        EditorView.baseTheme({
          '.ox-md-image': {
            maxWidth: '100%',
            maxHeight: '30em',
            borderRadius: '4px',
            display: 'block',
            margin: '0.4em 0',
          },
        }),
    },
  )
}
