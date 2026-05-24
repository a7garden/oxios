import { BarChart3, Dumbbell } from 'lucide-react'
import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { useKnowledgeHabits } from '@/hooks/use-knowledge'

export function Habits() {
  const currentYear = new Date().getFullYear()
  const [year, setYear] = useState(currentYear)
  const { data: habits, isLoading } = useKnowledgeHabits(year)

  if (isLoading) return <div className="p-6 text-muted-foreground">Loading habits...</div>

  // habits is a flexible object from oxios_markdown
  // For now, show a placeholder with year selector
  // The actual rendering depends on the Habits type structure
  const habitsData = habits as Record<string, unknown> | undefined

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold flex items-center gap-2">
          <Dumbbell className="h-5 w-5" />
          Habits
        </h2>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={() => setYear((y) => y - 1)}>
            ← {year - 1}
          </Button>
          <span className="text-sm font-medium w-16 text-center">{year}</span>
          <Button
            variant="outline"
            size="sm"
            onClick={() => setYear((y) => Math.min(y + 1, currentYear))}
            disabled={year >= currentYear}
          >
            {year + 1} →
          </Button>
        </div>
      </div>

      {habitsData && Object.keys(habitsData).length > 0 ? (
        <div className="space-y-4">
          {Object.entries(habitsData).map(([habitName, data]) => (
            <div key={habitName} className="space-y-1">
              <h3 className="text-sm font-medium">{habitName}</h3>
              <pre className="text-xs bg-muted p-3 rounded-lg overflow-x-auto">
                {JSON.stringify(data, null, 2)}
              </pre>
            </div>
          ))}
        </div>
      ) : (
        <div className="text-center py-12">
          <BarChart3 className="h-8 w-8 text-muted-foreground" />
          <p className="text-muted-foreground">No habit data for {year}</p>
          <p className="text-xs text-muted-foreground mt-1">
            Track habits in your journal to see them here
          </p>
        </div>
      )}
    </div>
  )
}
