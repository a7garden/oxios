import { useTranslation } from 'react-i18next'
import { Card, CardContent } from '@/components/ui/card'

/**
 * Legend for the Memory Embedding Map.
 *
 * Kept tiny on purpose — the visual encoding is documented inline so
 * the user does not have to cross-reference the RFC.
 */
export function ClusterLegend() {
  const { t } = useTranslation()
  return (
    <Card className="w-fit" data-testid="cluster-legend">
      <CardContent className="flex flex-wrap items-center gap-3 p-3 text-xs">
        <div className="flex items-center gap-1.5">
          <span className="inline-block h-3 w-3 rounded-full bg-emerald-500" aria-hidden />
          <span>{t('memory.hot')}</span>
        </div>
        <div className="flex items-center gap-1.5">
          <span className="inline-block h-3 w-3 rounded-full bg-amber-500" aria-hidden />
          <span>{t('memory.warm')}</span>
        </div>
        <div className="flex items-center gap-1.5">
          <span className="inline-block h-3 w-3 rounded-full bg-zinc-500" aria-hidden />
          <span>{t('memory.cold')}</span>
        </div>
        <div className="mx-1 h-4 w-px bg-border" aria-hidden />
        <div className="flex items-center gap-1.5">
          <span className="inline-block h-0 w-0 border-l-[6px] border-r-[6px] border-b-[10px] border-l-transparent border-r-transparent border-b-emerald-500" />
          <span>{t('memory.episode')}</span>
        </div>
        <div className="flex items-center gap-1.5">
          <span className="inline-block h-3 w-3 bg-sky-500" />
          <span>{t('memory.decision')}</span>
        </div>
        <div className="flex items-center gap-1.5">
          <span className="text-amber-500">★</span>
          <span>{t('memory.skill')}</span>
        </div>
      </CardContent>
    </Card>
  )
}
