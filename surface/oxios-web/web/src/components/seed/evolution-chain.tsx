import { GitBranch } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import type { EvolutionEntry } from '@/types/seed'

interface EvolutionChainProps {
  entries: EvolutionEntry[]
  currentId: string
  onNavigate: (id: string) => void
}

export function EvolutionChain({ entries, currentId, onNavigate }: EvolutionChainProps) {
  const { t } = useTranslation()
  if (!entries?.length) return null

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <GitBranch className="h-4 w-4" /> {t('seeds.evolutionChain')}
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="flex flex-wrap items-center gap-2">
          {entries.map((entry, idx) => {
            const isCurrent = entry.id === currentId
            return (
              <div key={entry.id} className="flex items-center gap-2">
                {idx > 0 && <span className="text-muted-foreground">→</span>}
                {isCurrent ? (
                  <Badge>
                    {t('seeds.generation', { gen: entry.generation })} (
                    {t('seeds.currentGeneration')})
                  </Badge>
                ) : (
                  <Button variant="outline" size="sm" onClick={() => onNavigate(entry.id)}>
                    {t('seeds.viewGeneration', { gen: entry.generation })}
                  </Button>
                )}
                {entry.score != null && (
                  <span className="text-xs text-muted-foreground">
                    {entry.score.toFixed(2)}
                  </span>
                )}
              </div>
            )
          })}
        </div>
      </CardContent>
    </Card>
  )
}
