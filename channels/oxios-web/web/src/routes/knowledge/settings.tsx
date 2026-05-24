import { createFileRoute, Link } from '@tanstack/react-router'
import { ArrowLeft, Settings } from 'lucide-react'
import { KnowledgeSettings } from '@/components/knowledge/knowledge-settings'

export const Route = createFileRoute('/knowledge/settings')({
  component: function SettingsPage() {
    return (
      <div className="flex flex-col h-full">
        <div className="flex items-center gap-3 px-5 py-3.5 border-b shrink-0">
          <Link
            to="/knowledge"
            className="flex items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground transition-colors"
          >
            <ArrowLeft className="h-4 w-4" />
            <span>Knowledge</span>
          </Link>
          <span className="text-muted-foreground">/</span>
          <h1 className="text-lg font-semibold flex items-center gap-2">
            <Settings className="h-5 w-5" />
            Settings
          </h1>
        </div>
        <div className="flex-1 overflow-y-auto">
          <KnowledgeSettings />
        </div>
      </div>
    )
  },
})
