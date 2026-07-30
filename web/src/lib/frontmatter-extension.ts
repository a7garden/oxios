/**
 * Frontmatter properties extension for CodeMirror 6.
 *
 * Replaces a leading YAML frontmatter block (`---\nkey: value\n---`) with an
 * Obsidian-style PropertiesWidget when the cursor is OUTSIDE the block.
 * When the cursor moves INSIDE, the raw YAML shows for editing.
 *
 * Extracted from the old live-preview-extension.ts during the atomic-editor
 * migration. The live-preview and token-hiding parts are now provided by
 * @atomic-editor/editor's built-in inline-preview extension.
 */
import type { EditorState, Range } from '@codemirror/state'
import { StateField } from '@codemirror/state'
import { Decoration, type DecorationSet, EditorView, WidgetType } from '@codemirror/view'
import type { FrontmatterEntry } from '@/lib/frontmatter'
import { findFrontmatterRange, parseFlowArray, parseFrontmatterBlock } from '@/lib/frontmatter'

// ─── Properties Widget ────────────────────────────────────────────────

class PropertiesWidget extends WidgetType {
  private entries: FrontmatterEntry[]

  constructor(entries: FrontmatterEntry[]) {
    super()
    this.entries = entries
  }

  eq(other: PropertiesWidget): boolean {
    if (other.entries.length !== this.entries.length) return false
    return this.entries.every((e, i) => {
      const o = other.entries[i]
      return o && e.key === o.key && e.valueText === o.valueText && e.kind === o.kind
    })
  }

  toDOM(): HTMLElement {
    const wrap = document.createElement('div')
    wrap.className = 'ox-md-properties'
    wrap.setAttribute('contenteditable', 'false')

    for (const entry of this.entries) {
      const row = document.createElement('div')
      row.className = 'ox-md-property-row'

      const keyEl = document.createElement('span')
      keyEl.className = 'ox-md-property-key'
      keyEl.textContent = entry.key

      const valEl = document.createElement('span')
      valEl.className = 'ox-md-property-value'

      if (entry.kind === 'array') {
        const tags = parseFlowArray(entry.valueText)
        for (const tag of tags) {
          const chip = document.createElement('span')
          chip.className = 'ox-md-property-tag'
          chip.textContent = tag
          valEl.appendChild(chip)
        }
      } else if (entry.kind === 'nested') {
        valEl.textContent = '…'
      } else {
        valEl.textContent = entry.valueText
      }

      row.appendChild(keyEl)
      row.appendChild(valEl)
      wrap.appendChild(row)
    }

    return wrap
  }

  ignoreEvent(): boolean {
    return false
  }
}

// ─── Decoration Builder ───────────────────────────────────────────────

/**
 * Build frontmatter decorations for the given editor state. Replaces the
 * entire `---…---` block with a PropertiesWidget when the cursor is outside.
 */
export function buildFrontmatterDecorations(state: EditorState): DecorationSet {
  const ranges: Range<Decoration>[] = []
  const doc = state.doc.toString()
  const fm = findFrontmatterRange(doc)
  if (!fm) return Decoration.set(ranges, true)

  const head = state.selection.main.head
  // Don't fold when cursor is inside the frontmatter block
  if (head >= fm.from && head <= fm.to) return Decoration.set(ranges, true)

  const entries = parseFrontmatterBlock(doc.slice(fm.from, fm.to))
  if (!entries || entries.length === 0) return Decoration.set(ranges, true)

  const widget = new PropertiesWidget(entries)
  ranges.push(Decoration.replace({ widget }).range(fm.from, fm.to))

  return Decoration.set(ranges, true)
}

// ─── StateField ───────────────────────────────────────────────────────

// StateField (not ViewPlugin) so the replace decoration can span line breaks
export const frontmatterExtension = StateField.define<DecorationSet>({
  create(state) {
    return buildFrontmatterDecorations(state)
  },
  update(value, tr) {
    if (!tr.docChanged && !tr.selection) return value
    return buildFrontmatterDecorations(tr.state)
  },
  provide: (f) => EditorView.decorations.from(f),
})
