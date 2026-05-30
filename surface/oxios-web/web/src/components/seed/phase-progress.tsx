import { Check, Circle } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { OuroborosPhase } from '@/types/seed'

const PHASES: OuroborosPhase[] = ['interview', 'seed', 'execute', 'evaluate', 'evolve']

export function PhaseProgress({ phaseReached }: { phaseReached: OuroborosPhase }) {
  const { t } = useTranslation()
  const currentIdx = PHASES.indexOf(phaseReached)

  return (
    <div className="flex items-center gap-2">
      {PHASES.map((phase, idx) => {
        const isComplete = idx < currentIdx
        const isCurrent = idx === currentIdx
        return (
          <div key={phase} className="flex items-center gap-2">
            {idx > 0 && (
              <div className={`h-0.5 w-8 ${isComplete ? 'bg-primary' : 'bg-muted'}`} />
            )}
            <div
              className={`flex items-center gap-1.5 rounded-full px-2 py-1 text-xs font-medium transition-colors ${
                isComplete
                  ? 'bg-primary/10 text-primary'
                  : isCurrent
                    ? 'bg-primary text-primary-foreground'
                    : 'bg-muted text-muted-foreground'
              }`}
            >
              {isComplete ? <Check className="h-3 w-3" /> : <Circle className="h-3 w-3" />}
              {t(`seeds.${phase}`)}
            </div>
          </div>
        )
      })}
    </div>
  )
}
