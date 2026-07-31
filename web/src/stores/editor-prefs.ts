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
  { label: 'SUIT', value: 'var(--font-sans)' },
  { label: 'System Mono', value: 'ui-monospace, SFMono-Regular, Menlo, Monaco, monospace' },
  { label: 'Menlo', value: 'Menlo, monospace' },
  { label: 'Monaco', value: 'Monaco, monospace' },
  { label: 'Courier', value: "'Courier New', monospace" },
]

export interface EditorPrefs {
  // ── Typography ──────────────────────────────────────────────
  /** Font size in pixels (applied via `--atomic-editor-body-size`). */
  fontSize: number
  /** Line height unitless (applied via `--atomic-editor-body-leading`). */
  lineHeight: number
  /** CSS font-family stack (applied via `--atomic-editor-font`). */
  fontFamily: string

  // ── Live rendering (Oxios-specific, not in @atomic-editor) ──
  /** Fold emoji shortcodes (`:sparkles:`) into rendered glyphs. */
  emojiFold: boolean
  /** Fold math blocks (`$$…$$`) into rendered KaTeX. */
  mathFold: boolean
  /** Fold mermaid code blocks into rendered diagrams. */
  mermaidFold: boolean

  // ── Status bar ───────────────────────────────────────────────
  /** Show the bottom status bar (word/char count, cursor position). */
  showStatusBar: boolean
  /** Enable per-level heading colors. Off = monochrome (size-only hierarchy). */
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
  fontFamily: 'var(--font-sans)',

  emojiFold: true,
  mathFold: true,
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
      version: 3,
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
        // v3: body font migrated Geist→SUIT and the Serif preset was removed
        // (no serif in the oxi system). Drop persisted Geist or serif stacks so
        // the new SUIT default applies; otherwise users keep a ghost font they
        // can no longer select.
        if (version < 3) {
          if (
            typeof state.fontFamily === 'string' &&
            (state.fontFamily.includes('Geist') || state.fontFamily.includes('serif'))
          ) {
            delete state.fontFamily
          }
        }
        return state
      },
    },
  ),
)
