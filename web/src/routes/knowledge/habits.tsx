import { createFileRoute, Link } from '@tanstack/react-router'
import { Activity, ArrowLeft } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Habits } from '@/components/knowledge/habits'

export const Route = createFileRoute('/knowledge/habits')({
  component: function HabitsPage() {
    const { t } = useTranslation()
    return (
      <div className="flex flex-col h-full">
        <div className="flex items-center gap-3 px-5 py-3.5 border-b shrink-0">
          <Link
            to="/knowledge"
            className="flex items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground transition-colors"
          >
            <ArrowLeft className="h-4 w-4" />
            <span>{t('knowledge.title')}</span>
          </Link>
          <span className="text-muted-foreground">/</span>
          <h1 className="text-lg font-semibold flex items-center gap-2">
            <Activity className="h-5 w-5" />
            {t('knowledge.habitsTitle')}
          </h1>
        </div>
        <div className="flex-1 overflow-y-auto">
          <Habits />
        </div>
      </div>
    )
  },
})
