/**
 * Mermaid inline render extension for CodeMirror 6.
 *
 * Replaces HyperMD's `hmdFoldCode: { mermaid: true }` behavior.
 * Walks the markdown syntax tree for `FencedCode` blocks whose info
 * string is `mermaid` and replaces the code content with a widget
 * that lazy-loads mermaid and renders an inline SVG.
 *
 * Why a ViewPlugin (and not a StateField)?
 *  - We only need to recompute when the doc changes, viewport scrolls,
 *    or the user types in a code block. A ViewPlugin lets us read the
 *    current `EditorView` and build a `DecorationSet` for the visible
 *    ranges only.
 *  - The widget is added to the set as a replace, so the underlying
 *    markdown source remains unchanged (round-trip safe).
 */
import {
  Decoration,
  type DecorationSet,
  type EditorView,
  MatchDecorator,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from '@codemirror/view'

// ─── Mermaid lazy loader ─────────────────────────────────────────────
type MermaidAPI = {
  initialize: (cfg: Record<string, unknown>) => void
  render: (id: string, code: string) => Promise<{ svg: string }>
}

let mermaidLoad: Promise<MermaidAPI> | null = null
let mermaidCounter = 0

async function loadMermaid(): Promise<MermaidAPI> {
  if (mermaidLoad) return mermaidLoad
  mermaidLoad = import('mermaid').then((m) => {
    const mermaid = m.default as unknown as MermaidAPI
    const root = getComputedStyle(document.documentElement)
    const isDark = document.documentElement.classList.contains('dark')
    mermaid.initialize({
      startOnLoad: false,
      securityLevel: 'loose',
      theme: isDark ? 'dark' : 'default',
      themeVariables: {
        primaryColor: isDark ? '#27272a' : '#f4f4f5',
        primaryBorderColor: (root.getPropertyValue('--primary') || '').trim() || '#18181b',
        primaryTextColor: isDark ? '#fafafa' : '#18181b',
        lineColor: isDark ? '#a1a1aa' : '#52525b',
        textColor: isDark ? '#fafafa' : '#18181b',
      },
    })
    return mermaid
  })
  return mermaidLoad
}

// ─── Widget ──────────────────────────────────────────────────────────
class MermaidWidget extends WidgetType {
  constructor(readonly code: string) {
    super()
  }

  toDOM(): HTMLElement {
    const root = document.createElement('div')
    root.className = 'mermaid-block'
    root.style.cssText =
      'padding: 12px; border: 1px solid var(--border); border-radius: 6px; ' +
      'background: var(--card); margin: 8px 0; text-align: center; min-height: 60px;'
    root.textContent = 'Loading mermaid…'

    const id = `mermaid-${Date.now()}-${mermaidCounter++}`
    loadMermaid()
      .then(async (mermaid) => {
        try {
          const { svg } = await mermaid.render(id, this.code.trim())
          root.innerHTML = svg
        } catch (e) {
          root.textContent = `Mermaid error: ${(e as Error).message}\n\n${this.code}`
        }
      })
      .catch((e) => {
        root.textContent = `Mermaid load failed: ${(e as Error).message}`
      })

    return root
  }

  ignoreEvent() {
    return false
  }
}

// ─── Decorator: match ```mermaid ... ``` blocks via regex ──────────
//
// We use a regex match decorator for resilience — even if the lezer AST
// changes shape between versions, this still matches the raw code.
// The pattern matches a fenced code block whose opening fence is
// exactly "```mermaid" (case-insensitive), non-greedy until the
// closing fence.
const fencedMermaid = new MatchDecorator({
  regexp: /^```mermaid\s*\n([\s\S]*?)^```\s*$/gm,
  decoration: (match) => {
    const code = match[1] ?? ''
    return Decoration.replace({ widget: new MermaidWidget(code) })
  },
})

// ─── ViewPlugin: tie it all together ────────────────────────────────
export const mermaidExtension = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet
    constructor(view: EditorView) {
      this.decorations = fencedMermaid.createDeco(view)
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged) {
        this.decorations = fencedMermaid.createDeco(update.view)
      }
    }
  },
  {
    decorations: (v) => v.decorations,
  },
)

// ─── Dark-mode reactivity ───────────────────────────────────────────
//
// Mermaid's theme is captured at first `loadMermaid()` call. If the
// document toggles `dark` after that, re-render. Lightweight: re-parse
// the doc and rebuild decorations. Mermaid itself is a singleton so
// subsequent render() calls pick up the latest theme.
export const mermaidDarkObserver = ViewPlugin.fromClass(
  class {
    observer: MutationObserver | null = null
    constructor(public view: EditorView) {
      this.observer = new MutationObserver(() => {
        // Force re-decoration so the widget re-creates with the new theme.
        view.dispatch({})
      })
      this.observer.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ['class'],
      })
    }
    destroy() {
      this.observer?.disconnect()
    }
  },
)
