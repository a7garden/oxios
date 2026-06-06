import { useQuery } from '@tanstack/react-query'
import { ArrowUpCircle } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { api } from '@/lib/api-client'

export interface SkillUpdate {
  slug: string
  currentVersion: string
  latestVersion: string
  changelog?: string
}

/** Hook: check for available skill updates via ClawHub. */
export function useSkillUpdates() {
  return useQuery({
    queryKey: ['marketplace', 'updates'],
    queryFn: async () => {
      const r = await api.get<SkillUpdate[]>('/api/marketplace/updates')
      return r ?? []
    },
    refetchInterval: 60_000 * 5, // 5 min
    refetchOnWindowFocus: false,
  })
}

/** Small badge showing the number of available updates. */
export function UpdateBadge({ count }: { count: number }) {
  if (count <= 0) return null
  return (
    <Badge variant="outline" className="text-xs gap-1 border-warning/50 text-warning">
      <ArrowUpCircle className="h-3 w-3" />
      {count}
    </Badge>
  )
}
