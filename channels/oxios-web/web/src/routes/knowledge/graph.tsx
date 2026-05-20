import { createFileRoute } from '@tanstack/react-router'
import { LinkGraph } from '@/components/knowledge/link-graph'

export const Route = createFileRoute('/knowledge/graph')({
  component: function GraphPage() {
    return (
      <div className="fixed inset-0 z-30 bg-background flex flex-col">
        <div className="flex items-center justify-between px-4 py-3 border-b">
          <h1 className="text-lg font-semibold">🔗 Link Graph</h1>
          <a href="/knowledge/" className="text-sm text-muted-foreground hover:text-foreground">
            ← Back
          </a>
        </div>
        <div className="flex-1 overflow-auto p-6 flex items-start justify-center">
          <LinkGraph className="w-full max-w-2xl" />
        </div>
      </div>
    )
  },
})
