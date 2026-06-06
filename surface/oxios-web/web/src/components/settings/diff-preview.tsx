import { AlertTriangle, ArrowRight, Check } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Separator } from '@/components/ui/separator'

export interface ConfigDiff {
  /** Dotted config path, e.g. `exec.allowed_commands`. */
  path: string
  /** Previous value (serialised). */
  before: unknown
  /** New value (serialised). */
  after: unknown
  /** True if the change is applied immediately. */
  hotReload: boolean
  /** Optional restart scope (kernel / gateway / etc.). */
  scope?: string
}

interface DiffPreviewProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  diffs: ConfigDiff[]
  onConfirm: () => void
  isPending?: boolean
}

/**
 * Modal shown right before saving changes. Lists every changed field
 * with old → new, plus a callout for fields that need a daemon restart.
 */
export function DiffPreview({ open, onOpenChange, diffs, onConfirm, isPending }: DiffPreviewProps) {
  const { t } = useTranslation()

  const restartRequired = diffs.filter((d) => !d.hotReload)
  const hotReload = diffs.filter((d) => d.hotReload)

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{t('settings.confirmChanges')}</DialogTitle>
          <DialogDescription>
            {t('settings.confirmChangesDescription', { count: diffs.length })}
          </DialogDescription>
        </DialogHeader>

        <div className="my-4 max-h-80 space-y-2 overflow-y-auto pr-1" data-testid="diff-list">
          {diffs.length === 0 ? (
            <p className="text-sm text-muted-foreground py-6 text-center">
              {t('settings.noChanges')}
            </p>
          ) : (
            diffs.map((diff) => <DiffRow key={diff.path} diff={diff} />)
          )}
        </div>

        {restartRequired.length > 0 && (
          <>
            <Separator />
            <div className="mt-4 rounded-lg border border-warning-subtle p-3 text-sm">
              <p className="font-medium flex items-center gap-2 text-warning">
                <AlertTriangle className="h-4 w-4" />
                {t('settings.restartRequiredWarning', { count: restartRequired.length })}
              </p>
              <ul className="mt-2 list-disc list-inside text-xs text-warning/90 space-y-0.5">
                {restartRequired.map((d) => (
                  <li key={d.path}>
                    <code className="font-mono">{d.path}</code>
                  </li>
                ))}
              </ul>
            </div>
          </>
        )}

        {hotReload.length > 0 && (
          <div className="text-xs text-muted-foreground">
            <p className="flex items-center gap-1.5">
              <Check className="h-3 w-3 text-success" />
              {t('settings.appliedImmediately', { count: hotReload.length })}
            </p>
          </div>
        )}

        <DialogFooter className="mt-4 flex justify-end gap-2">
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={isPending}>
            {t('common.cancel')}
          </Button>
          <Button
            onClick={onConfirm}
            disabled={isPending || diffs.length === 0}
            data-testid="confirm-save"
          >
            {t('common.save')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function DiffRow({ diff }: { diff: ConfigDiff }) {
  const { t } = useTranslation()
  return (
    <div className="flex flex-col gap-0.5 rounded-md border bg-muted/30 px-3 py-2">
      <div className="flex items-center justify-between">
        <code className="font-mono text-xs text-muted-foreground">{diff.path}</code>
        {diff.hotReload ? (
          <span className="text-2xs uppercase tracking-wider text-success">
            {t('settings.hotReload')}
          </span>
        ) : (
          <span className="text-2xs uppercase tracking-wider text-warning flex items-center gap-1">
            <AlertTriangle className="h-2.5 w-2.5" />
            {t('settings.requiresRestart')}
          </span>
        )}
      </div>
      <div className="flex items-center gap-2 text-xs">
        <ValuePreview value={diff.before} />
        <ArrowRight className="h-3 w-3 text-muted-foreground shrink-0" />
        <ValuePreview value={diff.after} />
      </div>
    </div>
  )
}

function ValuePreview({ value }: { value: unknown }) {
  const formatted =
    value === null || value === undefined
      ? '∅'
      : Array.isArray(value)
        ? `[${value.map((v) => JSON.stringify(v)).join(', ')}]`
        : typeof value === 'object'
          ? JSON.stringify(value)
          : String(value)
  return (
    <span className="font-mono truncate max-w-[260px] inline-block" title={formatted}>
      {formatted}
    </span>
  )
}
