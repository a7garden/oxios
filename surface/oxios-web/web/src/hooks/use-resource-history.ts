import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type { ResourceSnapshot } from '@/types'

/**
 * Resource history (CPU / memory / disk) for a given window.
 *
 * Default window is 30 samples — enough for a small sparkline. The
 * dashboard calls this once with a small window per stat card so the
 * fetches can be cached separately from the full Resources page.
 */
export function useResourceHistory(lastN = 30, refetchInterval = 10_000) {
  return useQuery({
    queryKey: ['resources', 'history', lastN],
    queryFn: async () => {
      const res = await api.get<{ snapshots: ResourceSnapshot[]; count: number }>(
        `/api/resources/history?last_n=${lastN}`,
      )
      return res.snapshots ?? []
    },
    refetchInterval,
  })
}

/**
 * Extract a numeric series (e.g. CPU%) from a snapshot list.
 * Returns numbers in chronological order (oldest → newest).
 */
export function seriesFromSnapshots(
  snapshots: ResourceSnapshot[],
  key: keyof ResourceSnapshot,
): number[] {
  return snapshots.map((s) => Number(s[key] ?? 0))
}

/**
 * Compute the % delta between the first and last value in a series.
 * Returns 0 for empty / single-point series.
 */
export function computeDelta(series: number[]): number {
  if (series.length < 2) return 0
  const first = series[0] ?? 0
  const last = series[series.length - 1] ?? 0
  if (first === 0) return last === 0 ? 0 : 100
  return ((last - first) / first) * 100
}
