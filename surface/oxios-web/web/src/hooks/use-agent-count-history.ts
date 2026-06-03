import { useEffect, useState } from 'react'

/**
 * Ring buffer of agent counts over time, used by the dashboard KPI
 * cards for Total Agents and Running Agents sparklines.
 *
 * The dashboard's status/agents queries are polled at 5–10s intervals.
 * Every time the count changes, we push the new value into a fixed-size
 * ring buffer (default 30 samples). Components downstream render the
 * ring as a sparkline.
 *
 * Two return values: `totalSeries` and `runningSeries` — the two
 * separate KPIs. The history depth defaults to 30 samples, which at
 * 5s polling is ~2.5 minutes of history.
 */
const DEFAULT_CAPACITY = 30

export interface AgentCountHistory {
  totalSeries: number[]
  runningSeries: number[]
}

function pushRing(buffer: number[], value: number, capacity: number): number[] {
  const next = [...buffer, value]
  return next.length > capacity ? next.slice(-capacity) : next
}

export function useAgentCountHistory(
  total: number | null,
  running: number,
  capacity: number = DEFAULT_CAPACITY,
): AgentCountHistory {
  const [totalBuf, setTotalBuf] = useState<number[]>([])
  const [runningBuf, setRunningBuf] = useState<number[]>([])

  // Track total. Skip the push when the field is null (status missing)
  // so the sparkline doesn't show a misleading 0 placeholder.
  useEffect(() => {
    if (total === null || total === undefined) return
    setTotalBuf((h) => pushRing(h, total, capacity))
  }, [total, capacity])

  // Track running count.
  useEffect(() => {
    setRunningBuf((h) => pushRing(h, running, capacity))
  }, [running, capacity])

  return { totalSeries: totalBuf, runningSeries: runningBuf }
}
