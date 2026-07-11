/**
 * Editor settings popover for the knowledge-base markdown editor.
 *
 * Surfaces every configurable aspect of the editor in three groups:
 *  - Typography: font size, line height, font family (CSS custom properties)
 *  - Editor: line numbers, active-line highlight, fold gutter, bracket matching
 *  - Live rendering: live-preview widgets, token hiding, and per-type fold extensions
 *
 * All values read from / write to the client-side `useEditorPrefs` store —
 * changes apply instantly to any mounted MarkdownEditor (CSS vars are live;
 * extensions reconfigure via the reactive `extensions` prop).
 */
import { RotateCcw, Settings2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { Select } from '@/components/ui/select'
import { Separator } from '@/components/ui/separator'
import { Slider } from '@/components/ui/slider'
import { Switch } from '@/components/ui/switch'
import { FONT_PRESETS, useEditorPrefs } from '@/stores/editor-prefs'

/** A single toggle row: label + description on the left, switch on the right. */
function ToggleRow({
  label,
  description,
  checked,
  onCheckedChange,
}: {
  label: string
  description?: string
  checked: boolean
  onCheckedChange: (v: boolean) => void
}) {
  return (
    <div className="flex items-center justify-between gap-3 py-1">
      <div className="min-w-0">
        <div className="text-sm">{label}</div>
        {description && <div className="text-xs text-muted-foreground">{description}</div>}
      </div>
      <Switch checked={checked} onCheckedChange={onCheckedChange} />
    </div>
  )
}

/** A color picker row: label + description on the left, swatch + clear on the right. */
function ColorRow({
  label,
  description,
  value,
  onChange,
}: {
  label: string
  description?: string
  value: string
  onChange: (v: string) => void
}) {
  return (
    <div className="flex items-center justify-between gap-3 py-1">
      <div className="min-w-0">
        <div className="text-sm">{label}</div>
        {description && <div className="text-xs text-muted-foreground">{description}</div>}
      </div>
      <div className="flex items-center gap-1.5 shrink-0">
        {value && (
          <button
            className="text-xs text-muted-foreground hover:text-foreground transition-colors"
            onClick={() => onChange('')}
          >
            <RotateCcw className="h-3 w-3" />
          </button>
        )}
        <input
          type="color"
          value={value || '#71717a'}
          onChange={(e) => onChange(e.target.value)}
          className="h-7 w-7 rounded border border-border cursor-pointer bg-transparent"
        />
      </div>
    </div>
  )
}

export function EditorSettingsPopover() {
  const { t } = useTranslation()
  const prefs = useEditorPrefs()

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8"
          aria-label={t('knowledge.editorSettings')}
        >
          <Settings2 className="h-4 w-4" />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-80 p-0">
        <div className="max-h-[70vh] overflow-y-auto">
          {/* ── Typography ─────────────────────────────────────── */}
          <div className="px-4 pt-4">
            <Label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              {t('knowledge.editorPrefTypography')}
            </Label>
          </div>
          <div className="space-y-3 px-4 py-3">
            <div className="space-y-1.5">
              <div className="flex items-center justify-between">
                <span className="text-sm">{t('knowledge.editorPrefFontSize')}</span>
                <span className="text-xs tabular-nums text-muted-foreground">
                  {prefs.fontSize}px
                </span>
              </div>
              <Slider
                value={[prefs.fontSize]}
                onValueChange={(v) => v[0] !== undefined && prefs.setPref('fontSize', v[0])}
                min={10}
                max={24}
                step={1}
              />
            </div>
            <div className="space-y-1.5">
              <div className="flex items-center justify-between">
                <span className="text-sm">{t('knowledge.editorPrefLineHeight')}</span>
                <span className="text-xs tabular-nums text-muted-foreground">
                  {prefs.lineHeight.toFixed(1)}
                </span>
              </div>
              <Slider
                value={[prefs.lineHeight]}
                onValueChange={(v) => v[0] !== undefined && prefs.setPref('lineHeight', v[0])}
                min={1.0}
                max={2.4}
                step={0.1}
              />
            </div>
            <div className="space-y-1.5">
              <span className="text-sm">{t('knowledge.editorPrefFontFamily')}</span>
              <Select
                value={prefs.fontFamily}
                onValueChange={(v) => prefs.setPref('fontFamily', v)}
                options={FONT_PRESETS.map((p) => ({ value: p.value, label: p.label }))}
              />
            </div>
          </div>

          <Separator />

          {/* ── Editor chrome ──────────────────────────────────── */}
          <div className="px-4 pt-3">
            <Label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              {t('knowledge.editorPrefEditor')}
            </Label>
          </div>
          <div className="space-y-0.5 px-4 py-2">
            <ToggleRow
              label={t('knowledge.editorPrefLineNumbers')}
              checked={prefs.lineNumbers}
              onCheckedChange={(v) => prefs.setPref('lineNumbers', v)}
            />
            <ToggleRow
              label={t('knowledge.editorPrefActiveLine')}
              checked={prefs.activeLineHighlight}
              onCheckedChange={(v) => prefs.setPref('activeLineHighlight', v)}
            />
            <ToggleRow
              label={t('knowledge.editorPrefFoldGutter')}
              checked={prefs.foldGutter}
              onCheckedChange={(v) => prefs.setPref('foldGutter', v)}
            />
            <ToggleRow
              label={t('knowledge.editorPrefBracketMatching')}
              checked={prefs.bracketMatching}
              onCheckedChange={(v) => prefs.setPref('bracketMatching', v)}
            />
            <ToggleRow
              label={t('knowledge.editorPrefStatusBar')}
              description={t('knowledge.editorPrefStatusBarDesc')}
              checked={prefs.showStatusBar}
              onCheckedChange={(v) => prefs.setPref('showStatusBar', v)}
            />
          </div>

          <Separator />

          {/* ── Live rendering ─────────────────────────────────── */}
          <div className="px-4 pt-3">
            <Label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              {t('knowledge.editorPrefLiveRendering')}
            </Label>
          </div>
          <div className="space-y-0.5 px-4 py-2">
            <ToggleRow
              label={t('knowledge.editorPrefLivePreview')}
              description={t('knowledge.editorPrefLivePreviewDesc')}
              checked={prefs.livePreview}
              onCheckedChange={(v) => prefs.setPref('livePreview', v)}
            />
            <ToggleRow
              label={t('knowledge.editorPrefTokenHiding')}
              description={t('knowledge.editorPrefTokenHidingDesc')}
              checked={prefs.tokenHiding}
              onCheckedChange={(v) => prefs.setPref('tokenHiding', v)}
            />
            <ToggleRow
              label={t('knowledge.editorPrefMermaidFold')}
              checked={prefs.mermaidFold}
              onCheckedChange={(v) => prefs.setPref('mermaidFold', v)}
            />
            <ToggleRow
              label={t('knowledge.editorPrefMathFold')}
              checked={prefs.mathFold}
              onCheckedChange={(v) => prefs.setPref('mathFold', v)}
            />
            <ToggleRow
              label={t('knowledge.editorPrefImageFold')}
              checked={prefs.imageFold}
              onCheckedChange={(v) => prefs.setPref('imageFold', v)}
            />
            <ToggleRow
              label={t('knowledge.editorPrefTableFold')}
              checked={prefs.tableFold}
              onCheckedChange={(v) => prefs.setPref('tableFold', v)}
            />
            <ToggleRow
              label={t('knowledge.editorPrefEmojiFold')}
              checked={prefs.emojiFold}
              onCheckedChange={(v) => prefs.setPref('emojiFold', v)}
            />
          </div>

          <Separator />

          {/* ── Markdown colors ────────────────────────────────── */}
          <div className="px-4 pt-3">
            <Label className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              {t('knowledge.editorPrefMarkdownColors')}
            </Label>
          </div>
          {/* Heading colors — compact grid, one swatch per level */}
          <div className="px-4 py-2">
            <div className="flex items-center justify-between">
              <span className="text-sm">{t('knowledge.editorPrefHeadingColors')}</span>
              {Object.values(prefs.headingColors).some(Boolean) && (
                <button
                  className="text-xs text-muted-foreground hover:text-foreground transition-colors"
                  onClick={() =>
                    prefs.setPref('headingColors', {
                      h1: '', h2: '', h3: '', h4: '', h5: '', h6: '',
                    })
                  }
                >
                  <RotateCcw className="h-3 w-3" />
                </button>
              )}
            </div>
            <div className="mt-2 grid grid-cols-6 gap-2">
              {(['h1', 'h2', 'h3', 'h4', 'h5', 'h6'] as const).map((lvl) => {
                const hasCustom = Boolean(prefs.headingColors[lvl])
                return (
                  <div key={lvl} className="flex flex-col items-center gap-1">
                    <input
                      type="color"
                      value={prefs.headingColors[lvl] || '#71717a'}
                      onChange={(e) =>
                        prefs.setPref('headingColors', {
                          ...prefs.headingColors,
                          [lvl]: e.target.value,
                        })
                      }
                      className={`h-7 w-full rounded border cursor-pointer bg-transparent ${
                        hasCustom ? 'ring-2 ring-primary ring-offset-1 ring-offset-background' : 'border-border'
                      }`}
                    />
                    <span className="text-[10px] text-muted-foreground">{lvl.toUpperCase()}</span>
                  </div>
                )
              })}
            </div>
          </div>
          {/* Marker and link colors */}
          <div className="space-y-0.5 px-4 pb-2">
            <ColorRow
              label={t('knowledge.editorPrefMarkerColor')}
              description={t('knowledge.editorPrefMarkerColorDesc')}
              value={prefs.markerColor}
              onChange={(v) => prefs.setPref('markerColor', v)}
            />
            <ColorRow
              label={t('knowledge.editorPrefLinkColor')}
              description={t('knowledge.editorPrefLinkColorDesc')}
              value={prefs.linkColor}
              onChange={(v) => prefs.setPref('linkColor', v)}
            />
          </div>

          <Separator />

          {/* ── Reset ──────────────────────────────────────────── */}
          <div className="px-4 py-3">
            <Button
              variant="ghost"
              size="sm"
              className="w-full text-muted-foreground"
              onClick={() => prefs.reset()}
            >
              <RotateCcw className="h-3.5 w-3.5 mr-1.5" />
              {t('knowledge.editorPrefReset')}
            </Button>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  )
}
