import { render } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { EmbeddingCanvas } from '@/components/memory/embedding-canvas'
import type { MemoryMapEntry } from '@/types/memory'

// ---- Mocks ----
//
// We need to control the d3-force simulation lifecycle to assert that
// the Animate toggle restarts/stops it. d3-zoom and ResizeObserver are
// stubbed because jsdom has no layout, and we provide a no-op canvas
// getContext so the draw loop does not blow up.

type SimInstance = {
  force: ReturnType<typeof vi.fn>
  alpha: ReturnType<typeof vi.fn>
  alphaDecay: ReturnType<typeof vi.fn>
  restart: ReturnType<typeof vi.fn>
  stop: ReturnType<typeof vi.fn>
  on: ReturnType<typeof vi.fn>
}

function makeFakeSim(): SimInstance {
  const sim: SimInstance = {
    force: vi.fn().mockReturnThis(),
    alpha: vi.fn(function (this: SimInstance, _v: number) {
      return this
    }),
    alphaDecay: vi.fn().mockReturnThis(),
    restart: vi.fn(function (this: SimInstance) {
      return this
    }),
    stop: vi.fn(),
    on: vi.fn(function (this: SimInstance, _event: string, _cb: () => void) {
      return this
    }),
  }
  return sim
}

const lastSim = vi.hoisted(() => ({ current: null as SimInstance | null }))

vi.mock('d3-force', () => {
  const chainable = () => {
    const obj: Record<string, unknown> = {}
    ;(Object.keys({ id: 1, distance: 1, strength: 1, radius: 1 }) as string[]).forEach((k) => {
      obj[k] = vi.fn(() => obj)
    })
    return obj
  }
  return {
    forceCenter: vi.fn(() => chainable()),
    forceCollide: vi.fn(() => chainable()),
    forceLink: vi.fn(() => chainable()),
    forceManyBody: vi.fn(() => chainable()),
    forceSimulation: vi.fn(() => {
      const s = makeFakeSim()
      lastSim.current = s
      return s
    }),
    forceX: vi.fn(() => chainable()),
    forceY: vi.fn(() => chainable()),
  }
})

vi.mock('d3-selection', () => {
  const noopSel = {
    call: vi.fn().mockReturnThis(),
    on: vi.fn().mockReturnThis(),
  }
  return { select: vi.fn(() => noopSel) }
})

vi.mock('d3-zoom', () => {
  const noopZoom = {
    scaleExtent: vi.fn().mockReturnThis(),
    on: vi.fn().mockReturnThis(),
    transform: vi.fn(),
  }
  return {
    zoom: vi.fn(() => noopZoom),
    zoomIdentity: { translate: vi.fn().mockReturnThis(), scale: vi.fn().mockReturnThis() },
  }
})

// ResizeObserver is not in jsdom.
class ResizeObserverMock {
  observe = vi.fn()
  unobserve = vi.fn()
  disconnect = vi.fn()
}
globalThis.ResizeObserver = ResizeObserverMock as unknown as typeof ResizeObserver

// jsdom does not implement canvas 2D context.
const fakeContext = {
  setTransform: vi.fn(),
  clearRect: vi.fn(),
  translate: vi.fn(),
  scale: vi.fn(),
  beginPath: vi.fn(),
  arc: vi.fn(),
  rect: vi.fn(),
  moveTo: vi.fn(),
  lineTo: vi.fn(),
  closePath: vi.fn(),
  fill: vi.fn(),
  stroke: vi.fn(),
  save: vi.fn(),
  restore: vi.fn(),
}
HTMLCanvasElement.prototype.getContext = vi.fn(
  () => fakeContext,
) as unknown as typeof HTMLCanvasElement.prototype.getContext

// We don't mock requestAnimationFrame; jsdom polyfills it as a
// microtask, so the canvas component's draw scheduling still runs.
beforeEach(() => {
  lastSim.current = null
})

afterEach(() => {
  vi.clearAllMocks()
})

const sampleEntry = (id: string, x = 0.1, y = 0.2): MemoryMapEntry => ({
  id,
  tier: 'hot',
  mem_type: 'fact',
  content_preview: `preview-${id}`,
  created_at: '2026-06-04T00:00:00Z',
  access_count: 1,
  coords_2d: [x, y],
  top_neighbors: [],
})

const ENTRIES: MemoryMapEntry[] = [
  sampleEntry('a', 0.1, 0.2),
  sampleEntry('b', -0.3, 0.4),
  sampleEntry('c', 0.0, -0.5),
]

describe('EmbeddingCanvas — Animate toggle (P0-2)', () => {
  it('starts the simulation with non-zero alpha on first mount', () => {
    render(<EmbeddingCanvas entries={ENTRIES} animate={true} />)
    expect(lastSim.current).not.toBeNull()
    // alpha() must have been called with 0.6 at least once during the
    // graph build (animate=true).
    expect(lastSim.current?.alpha).toHaveBeenCalled()
    const alphas = (lastSim.current?.alpha.mock.calls ?? []).map((c) => c[0])
    expect(alphas).toContain(0.6)
  })

  it('restart() is called when animate flips from false to true', () => {
    const { rerender } = render(<EmbeddingCanvas entries={ENTRIES} animate={false} />)
    expect(lastSim.current).not.toBeNull()
    const sim = lastSim.current!
    // After mount with animate=false, the sim should be stopped (not
    // restarted). The .alpha(0.001) used at graph-build time is fine.
    expect(sim.stop).toHaveBeenCalled()
    sim.stop.mockClear()
    sim.restart.mockClear()
    sim.alpha.mockClear()

    rerender(<EmbeddingCanvas entries={ENTRIES} animate={true} />)

    // The Animate effect must call sim.alpha(0.6).restart().
    expect(sim.alpha).toHaveBeenCalledWith(0.6)
    expect(sim.restart).toHaveBeenCalled()
  })

  it('stop() is called when animate flips from true to false', () => {
    const { rerender } = render(<EmbeddingCanvas entries={ENTRIES} animate={true} />)
    expect(lastSim.current).not.toBeNull()
    const sim = lastSim.current!
    sim.stop.mockClear()

    rerender(<EmbeddingCanvas entries={ENTRIES} animate={false} />)
    expect(sim.stop).toHaveBeenCalled()
  })
})
