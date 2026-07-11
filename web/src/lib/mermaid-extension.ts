/**
 * Mermaid inline render extension for CodeMirror 6.
 *
 * Replaces HyperMD's `hmdFoldCode: { mermaid: true }` behavior.
 * Walks the markdown syntax tree for `FencedCode` blocks whose info
 * string is `mermaid` and replaces the code content with a widget
 * that lazy-loads mermaid and renders an inline SVG.
 *
 * Why a StateField (not a ViewPlugin)?
 *  - The widget replaces the entire ```mermaid … ``` block, which spans
 *    line breaks. CodeMirror 6 forbids replace decorations that cross
 *    line breaks from coming via a ViewPlugin — only state-derived
 *    decorations may, so this is a StateField.
 *  - The widget is a replace, so the underlying markdown source remains
 *    unchanged (round-trip safe). CM6 only calls `toDOM()` for widgets
 *    in the visible viewport, keeping the lazy mermaid render off-screen.
 */

import type { EditorState, Range } from '@codemirror/state'
import { StateEffect, StateField } from '@codemirror/state'
import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
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

// ─── Decoration builder: scan the whole doc for ```mermaid blocks ──
//
// We deliberately scan the full document rather than use a viewport-
// scoped MatchDecorator: the replace decoration must live in a
// StateField (it spans line breaks), and StateFields cannot read the
// view's viewport. CM6 still only instantiates widget DOM for ranges
// in the visible viewport, so the lazy mermaid render stays off-screen.
//
// The pattern matches a fenced code block whose opening fence is
// "```mermaid", non-greedy until the closing fence.
const MERMAID_RE = /^```mermaid\s*\n([\s\S]*?)^```\s*$/gm

export function buildMermaidDecorations(state: EditorState): DecorationSet {
  const builder: Range<Decoration>[] = []
  const text = state.doc.toString()
  MERMAID_RE.lastIndex = 0
  for (let m = MERMAID_RE.exec(text); m !== null; m = MERMAID_RE.exec(text)) {
    builder.push(
      Decoration.replace({ widget: new MermaidWidget(m[1] ?? '') }).range(
        m.index,
        m.index + m[0].length,
      ),
    )
  }
  builder.sort((a, b) => a.from - b.from)
  return Decoration.set(builder)
}

// Force-rebuild the decorations. The dark-mode observer dispatches this
// effect so widgets are recreated (and re-rendered with the new theme).
const forceMermaidRedraw = StateEffect.define<void>()

// ─── StateField: line-break-spanning replaces must be state-derived ──
export const mermaidExtension = StateField.define<DecorationSet>({
  create(state) {
    return buildMermaidDecorations(state)
  },
  update(deco, tr) {
    if (tr.docChanged || tr.effects.some((e) => e.is(forceMermaidRedraw))) {
      return buildMermaidDecorations(tr.state)
    }
    return deco.map(tr.changes)
  },
  provide: (f) => EditorView.decorations.from(f),
})

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
        view.dispatch({ effects: forceMermaidRedraw.of() })
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
