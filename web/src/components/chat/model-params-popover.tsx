// ModelParamsPopover — temperature + max_tokens sliders for the next message.
//
// LobeHub analogue: features/ChatInput/ActionBar/Params/ (sliders popover).
// Oxios version: a native details/summary disclosure with range inputs.
// Values flow into the WS payload as `temperature` and `max_tokens`; the
// backend reads them from IncomingMessage metadata and threads them
// through `MsgCtx` → `ExecEnv::model_params` → `AgentConfig`.
//
// When a slider is reset to the provider default (null), the field is
// omitted from the WS payload and the agent runtime falls back to its
// built-in defaults (0.7 / 8192).

import { ChevronDown, SlidersHorizontal } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { useChatStore } from '@/stores/chat'

export function ModelParamsPopover() {
  const { t } = useTranslation()
  const temperature = useChatStore((s) => s.temperature)
  const maxTokens = useChatStore((s) => s.maxTokens)
  const setTemperature = useChatStore((s) => s.setTemperature)
  const setMaxTokens = useChatStore((s) => s.setMaxTokens)
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const close = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', close)
    return () => document.removeEventListener('mousedown', close)
  }, [open])

  const hasOverrides = temperature != null || maxTokens != null

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className={cn(
          'flex h-8 items-center gap-1 rounded-md border px-2 text-xs transition-colors',
          hasOverrides
            ? 'border-primary/40 bg-primary/5 text-primary'
            : 'border-input text-muted-foreground hover:bg-accent hover:text-foreground',
        )}
        aria-label={t('chat.modelParams')}
        aria-expanded={open}
      >
        <SlidersHorizontal className="size-3.5" />
        <ChevronDown className="size-3" />
      </button>
      {open && (
        <div className="absolute bottom-full right-0 z-20 mb-1 w-72 rounded-lg border bg-popover p-3 shadow-lg">
          <div className="mb-3">
            <div className="flex items-center justify-between">
              <label htmlFor="temp-slider" className="text-xs font-medium">
                {t('chat.temperature')}
              </label>
              <span className="text-2xs text-muted-foreground">
                {temperature != null ? temperature.toFixed(1) : t('chat.default')}
              </span>
            </div>
            <input
              id="temp-slider"
              type="range"
              min={0}
              max={2}
              step={0.1}
              value={temperature ?? 0.7}
              onChange={(e) => setTemperature(Number(e.target.value))}
              className="mt-1 w-full"
            />
            <button
              type="button"
              onClick={() => setTemperature(null)}
              className="mt-1 text-2xs text-muted-foreground hover:text-foreground"
            >
              {t('chat.reset')}
            </button>
          </div>
          <div>
            <div className="flex items-center justify-between">
              <label htmlFor="maxtok-slider" className="text-xs font-medium">
                {t('chat.maxTokens')}
              </label>
              <span className="text-2xs text-muted-foreground">
                {maxTokens != null ? maxTokens : t('chat.default')}
              </span>
            </div>
            <input
              id="maxtok-slider"
              type="range"
              min={256}
              max={32768}
              step={256}
              value={maxTokens ?? 8192}
              onChange={(e) => setMaxTokens(Number(e.target.value))}
              className="mt-1 w-full"
            />
            <button
              type="button"
              onClick={() => setMaxTokens(null)}
              className="mt-1 text-2xs text-muted-foreground hover:text-foreground"
            >
              {t('chat.reset')}
            </button>
          </div>
        </div>
      )}
    </div>
  )
}
