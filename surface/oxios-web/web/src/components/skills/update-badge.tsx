import { useQuery } from '@tanstack/react-query'
import { RefreshCw } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { api } from '@/lib/api-client'

interface SkillUpdate {
  slug: string
  currentVersion: string
  latestVersion: string
  changelog?: string
}

export function useSkillUpdates() {
  return useQuery({
    queryKey: ['marketplace', 'updates'],
    queryFn: async () => {
      const res = await api.get<SkillUpdate[]>('/api/marketplace/updates')
      return res ?? []
    },
    refetchInterval: 60_000 * 5, // every 5 minutes
    refetchOnWindowFocus: false,
  })
}

export function UpdateBadge({ count }: { count: number }) {
  if (count === 0) return null
  return (
    <Badge variant="destructive" className="text-xs px-1.5 py-0 h-5 min-w-5 flex items-center justify-center">
      {count}
    </Badge>
  )
}

export function SkillUpdateIndicator({ slug }: { slug: string }) {
  const { t } = useTranslation()
  const { data: updates } = useSkillUpdates()
  const hasUpdate = updates?.some((u) => u.slug === slug)
  if (!hasUpdate) return null
  return (
    <Badge variant="outline" className="text-xs gap-1 border-amber-500/50 text-amber-600 dark:text-amber-400">
      <RefreshCw className="h-3 w-3" />
      {t('skills.updateAvailable')}
    </Badge>
  )
}
