import { create } from 'zustand'
import { persist } from 'zustand/middleware'

/**
 * Editor appearance preferences for the knowledge-base markdown editor.
 *
 * These are purely **client-side UI preferences** (font size, line numbers,
 * live-rendering toggles). They are NOT part of the backend `KnowledgeConfig`
 * — no API round-trip is needed to flip a toggle, and they are device-local
 * (a desktop user may want line numbers while a laptop user may not).
 *
 * Defaults mirror the pre-settings hard-coded behaviour so existing users see
 * no visual change until they open the settings popover.
 */

/** Font-family presets offered in the settings popover. */
export const FONT_PRESETS: { label: string; value: string }[] = [
  { label: 'Geist Sans', value: "'Geist', system-ui, sans-serif" },
  { label: 'Serif', value: "ui-serif, Georgia, 'Times New Roman', serif" },
  { label: 'System Mono', value: 'ui-monospace, SFMono-Regular, Menlo, Monaco, monospace' },
  { label: 'Menlo', value: 'Menlo, monospace' },
  { label: 'Monaco', value: 'Monaco, monospace' },
  { label: 'Courier', value: "'Courier New', monospace" },
]

export interface EditorPrefs {
  // ── Typography ──────────────────────────────────────────────
  /** Font size in pixels (applied via `--editor-font-size`). */
  fontSize: number
  /** Line height unitless (applied via `--editor-line-height`). */
  lineHeight: number
  /** CSS font-family stack (applied via `--editor-font-mono`). */
  fontFamily: string

  // ── Editor chrome ───────────────────────────────────────────
  /** Show line-number gutter. */
  lineNumbers: boolean
  /** Highlight the active line. */
  activeLineHighlight: boolean
  /** Show the fold-gutter (clickable fold markers). */
  foldGutter: boolean
  /** Match brackets around the cursor. */
  bracketMatching: boolean

  // ── Live rendering ──────────────────────────────────────────
  /** Live-preview widgets: heading markers → styled text, `---` → rule, task checkboxes. */
  livePreview: boolean
  /** Hide markdown markup tokens on inactive lines (WYSIWYG-ish). */
  tokenHiding: boolean
  /** Fold emoji shortcodes (`:sparkles:`) into rendered glyphs. */
  emojiFold: boolean
  /** Fold math blocks (`$$…$$`) into rendered KaTeX. */
  mathFold: boolean
  /** Fold image links into inline thumbnails. */
  imageFold: boolean
  /** Fold GFM tables into rendered grid widgets. */
  tableFold: boolean
  /** Fold mermaid code blocks into rendered diagrams. */
  mermaidFold: boolean

  // ── Status bar ───────────────────────────────────────────────
  /** Show the bottom status bar (word/char count, cursor position). */
  showStatusBar: boolean
  /** Enable per-level heading colors. Off = monochrome (size-only hierarchy,
   * overrides oneDark's default heading hue). On = use per-level custom colors. */
  headingColorsEnabled: boolean

  // ── Markdown colors ──────────────────────────────────────────
  /** Per-level heading text colors. Used only when headingColorsEnabled is true. */
  headingColors: {
    h1: string
    h2: string
    h3: string
    h4: string
    h5: string
    h6: string
  }
  /** Markdown syntax marker color (`#`, `*`, `` ` ``, `>`). Empty = inherit theme. */
  markerColor: string
  /** Link / URL color. Empty = inherit theme. */
  linkColor: string

  // ── Actions ─────────────────────────────────────────────────
  setPref: <K extends keyof EditorPrefs>(key: K, value: EditorPrefs[K]) => void
  reset: () => void
}

const DEFAULTS: Omit<EditorPrefs, 'setPref' | 'reset'> = {
  fontSize: 15,
  lineHeight: 1.75,
  fontFamily: "'Geist', system-ui, sans-serif",

  lineNumbers: false,
  activeLineHighlight: true,
  foldGutter: true,
  bracketMatching: true,

  livePreview: true,
  tokenHiding: true,
  emojiFold: true,
  mathFold: true,
  imageFold: true,
  tableFold: true,
  mermaidFold: true,

  showStatusBar: true,
  headingColorsEnabled: false,
  headingColors: {
    h1: '',
    h2: '',
    h3: 'var(--muted-foreground)',
    h4: 'var(--muted-foreground)',
    h5: 'var(--muted-foreground)',
    h6: 'var(--muted-foreground)',
  },
  markerColor: '',
  linkColor: '',
}

export const useEditorPrefs = create<EditorPrefs>()(
  persist(
    (set) => ({
      ...DEFAULTS,
      setPref: (key, value) => set({ [key]: value } as Partial<EditorPrefs>),
      reset: () => set({ ...DEFAULTS }),
    }),
    {
      name: 'oxios-editor-prefs',
      version: 2,
      migrate: (persisted, version) => {
        const state = (persisted ?? {}) as Record<string, unknown>
        // v2: default font switched from mono to Geist sans, font size
        // bumped 14→15px, line height 1.7→1.75, and H3-H6 gained muted
        // color defaults. Drop all changed keys so new DEFAULTS apply.
        if (version < 2) {
          delete state.fontFamily
          delete state.headingColors
          delete state.fontSize
          delete state.lineHeight
        }
        return state
      },
    },
  ),
)
