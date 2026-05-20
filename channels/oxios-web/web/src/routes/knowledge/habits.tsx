import { createFileRoute } from '@tanstack/react-router'
import { Habits } from '@/components/knowledge/habits'

export const Route = createFileRoute('/knowledge/habits')({
  component: function HabitsPage() {
    return (
      <div className="fixed inset-0 z-30 bg-background overflow-y-auto">
        <div className="flex items-center justify-between px-4 py-3 border-b sticky top-0 bg-background z-10">
          <h1 className="text-lg font-semibold">💪 Habits</h1>
          <a href="/knowledge/" className="text-sm text-muted-foreground hover:text-foreground">
            ← Back
          </a>
        </div>
        <Habits />
      </div>
    )
  },
})
