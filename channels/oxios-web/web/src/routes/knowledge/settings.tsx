import { createFileRoute, Link } from '@tanstack/react-router'
import { ArrowLeft } from 'lucide-react'
import { KnowledgeSettings } from '@/components/knowledge/knowledge-settings'

export const Route = createFileRoute('/knowledge/settings')({
  component: function SettingsPage() {
    return (
      <div className="flex flex-col h-full">
        <div className="flex items-center gap-3 px-4 py-3 border-b shrink-0">
          <Link
            to="/knowledge/"
            className="flex items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground transition-colors"
          >
            <ArrowLeft className="h-4 w-4" />
            <span>Knowledge</span>
          </Link>
          <span className="text-muted-foreground">/</span>
          <h1 className="text-lg font-semibold">⚙️ Settings</h1>
        </div>
        <div className="flex-1 overflow-y-auto">
          <KnowledgeSettings />
        </div>
      </div>
    )
  },
})
