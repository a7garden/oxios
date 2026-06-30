/**
 * Emoji fold extension for CodeMirror 6.
 *
 * Replaces HyperMD's `fold-emoji` behaviour: a recognized `:shortcode:` is
 * rendered inline as its Unicode glyph when the cursor is NOT on that line
 * (±1 active region), so editing the source still works. Mirrors files.md,
 * which folded `:smile:` → 😄 via hmdFoldEmoji.
 *
 * Only shortcodes present in EMOJI_SHORTCODES are folded (so `:8080:` in
 * prose or a port number is left alone), and matches inside code spans /
 * fenced blocks are skipped (code is never emoji-folded).
 *
 * Round-trip safe: the widget is purely visual; the source `:name:` is
 * preserved.
 */
import { syntaxTree } from '@codemirror/language'
import type { EditorState, Range } from '@codemirror/state'
import {
  Decoration,
  type DecorationSet,
  type EditorView as EditorViewType,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from '@codemirror/view'
import { EMOJI_SHORTCODES } from '@/lib/emoji-shortcodes'

const EMOJI_RE = /:([a-z0-9_+]+):/g

class EmojiWidget extends WidgetType {
  constructor(readonly glyph: string) {
    super()
  }
  eq(other: EmojiWidget) {
    return other.glyph === this.glyph
  }
  toDOM() {
    const el = document.createElement('span')
    el.className = 'ox-md-emoji'
    el.textContent = this.glyph
    return el
  }
  ignoreEvent() {
    return false
  }
}

/** Is `pos` inside a code span or fenced block? */
function inCode(state: EditorState, pos: number): boolean {
  let node = syntaxTree(state).resolveInner(pos)
  while (node) {
    if (
      node.name === 'FencedCode' ||
      node.name === 'CodeBlock' ||
      node.name === 'InlineCode' ||
      node.name === 'CodeText'
    ) {
      return true
    }
    const parent = node.parent
    if (!parent || parent === node) break
    node = parent
  }
  return false
}

export function buildEmojiDecorations(state: EditorState): DecorationSet {
  const builder: Range<Decoration>[] = []
  const { doc } = state
  const cursorLine = doc.lineAt(state.selection.main.head).number
  const text = doc.toString()
  EMOJI_RE.lastIndex = 0
  let m: RegExpExecArray | null
  for (m = EMOJI_RE.exec(text); m !== null; m = EMOJI_RE.exec(text)) {
    const code = m[1]
    if (!code) continue
    const glyph = EMOJI_SHORTCODES[code]
    if (!glyph) continue
    const from = m.index
    const to = from + m[0].length
    // Active line ±1: leave `:name:` source visible for editing.
    if (Math.abs(doc.lineAt(from).number - cursorLine) <= 1) continue
    if (inCode(state, from)) continue
    builder.push(Decoration.replace({ widget: new EmojiWidget(glyph) }).range(from, to))
  }
  builder.sort((a, b) => a.from - b.from)
  return Decoration.set(builder)
}

export const emojiFoldExtension = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet
    constructor(view: EditorViewType) {
      this.decorations = buildEmojiDecorations(view.state)
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.selectionSet || update.viewportChanged) {
        this.decorations = buildEmojiDecorations(update.view.state)
      }
    }
  },
  {
    decorations: (v) => v.decorations,
  },
)
