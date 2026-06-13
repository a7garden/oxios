import {
  forceCenter,
  forceCollide,
  forceLink,
  forceManyBody,
  forceSimulation,
  forceX,
  forceY,
  type Simulation,
  type SimulationLinkDatum,
  type SimulationNodeDatum,
} from 'd3-force'
import { select } from 'd3-selection'
import { type ZoomBehavior, zoom, zoomIdentity } from 'd3-zoom'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { MemoryMapEntry, MemoryMapNeighbor } from '@/types/memory'

/**
 * Render a memory-embedding scatter plot on a single HTMLCanvasElement.
 *
 * Why canvas (not SVG)?
 * - 500+ nodes: SVG starts dropping frames around 200 nodes in our
 *   rendering style. Canvas keeps 60 fps at 1000.
 * - d3-force still drives the layout; we just draw the resulting
 *   positions via Canvas 2D context every animation tick.
 *
 * Why a custom d3-zoom (vs the SVG pattern)?
 * - We translate the canvas transform in `draw()`, which means the
 *   underlying simulation positions never change. The simulation
 *   runs in screen-space-equivalent coordinates centred at (0, 0),
 *   spanning roughly [-1, 1] after the backend's `normalize_to_unit_square`.
 */

export interface EmbeddingCanvasProps {
  entries: MemoryMapEntry[]
  /** Highlight one node (sets its full opacity; dims the rest). */
  selectedId?: string | null
  /** Hover a node (does not change selection). */
  onHover?: (id: string | null) => void
  /** Click a node. */
  onSelect?: (id: string) => void
  /** Optionally fly-to a node (used by search). */
  flyToId?: string | null
  /** Disable simulation/animations for big datasets. */
  animate?: boolean
  /** Show neighbour edges above this similarity (0.0–1.0). */
  edgeThreshold?: number
}

// d3-force node extends the entry with runtime fields.
type SimNode = MemoryMapEntry &
  SimulationNodeDatum & {
    /** Pre-computed target position from the backend (anchors the simulation). */
    tx: number
    ty: number
  }

type SimLink = SimulationLinkDatum<SimNode> & { similarity: number }

// Tier colour map. Matches the rest of the memory UI (emerald/amber/zinc
// with OKLCH-leaning values for dark mode).
const TIER_FILL: Record<string, string> = {
  hot: '#10b981', // emerald-500
  warm: '#f59e0b', // amber-500
  cold: '#71717a', // zinc-500
}

// Memory-type shape mapping. We draw distinct shapes for the four
// most common types so the map is still readable in grayscale.
function shapePath(ctx: CanvasRenderingContext2D, memType: string, r: number) {
  switch (memType) {
    case 'fact':
      ctx.beginPath()
      ctx.arc(0, 0, r, 0, Math.PI * 2)
      ctx.fill()
      break
    case 'episode':
      ctx.beginPath()
      ctx.moveTo(0, -r)
      ctx.lineTo(r, r)
      ctx.lineTo(-r, r)
      ctx.closePath()
      ctx.fill()
      break
    case 'decision':
      ctx.beginPath()
      ctx.rect(-r, -r, r * 2, r * 2)
      ctx.fill()
      break
    case 'skill':
      // 4-pointed star
      ctx.beginPath()
      for (let i = 0; i < 8; i += 1) {
        const a = (i / 8) * Math.PI * 2 - Math.PI / 2
        const radius = i % 2 === 0 ? r : r * 0.45
        const x = Math.cos(a) * radius
        const y = Math.sin(a) * radius
        if (i === 0) ctx.moveTo(x, y)
        else ctx.lineTo(x, y)
      }
      ctx.closePath()
      ctx.fill()
      break
    default:
      // Other types: a smaller circle
      ctx.beginPath()
      ctx.arc(0, 0, r * 0.85, 0, Math.PI * 2)
      ctx.fill()
  }
}

function nodeRadius(entry: MemoryMapEntry): number {
  // Recency: 0..1 based on access_count (log scale, capped).
  const recencyBoost = Math.min(1, Math.log1p(entry.access_count) / 3)
  // Base 5px + up to +5px from access count.
  return 5 + recencyBoost * 5
}

/**
 * Invisible padded hit-test ring (CSS px). WCAG 2.5.5 requires a touch
 * target of at least 24×24 CSS px. The visual node radius is 5–10 px,
 * so we extend the hit area by `HIT_TEST_PADDING` without changing
 * the visual size.
 */
const HIT_TEST_PADDING = 12

export function EmbeddingCanvas({
  entries,
  selectedId = null,
  onHover,
  onSelect,
  flyToId = null,
  animate = true,
  edgeThreshold = 0.7,
}: EmbeddingCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  const containerRef = useRef<HTMLDivElement | null>(null)
  const simRef = useRef<Simulation<SimNode, SimLink> | null>(null)
  const nodesRef = useRef<SimNode[]>([])
  const linksRef = useRef<SimLink[]>([])
  // We keep transform state in a ref so d3-zoom and the draw loop agree.
  const transformRef = useRef<{ k: number; x: number; y: number }>({ k: 1, x: 0, y: 0 })
  // Adjacency map for fast hover → neighbours lookup.
  const neighboursRef = useRef<Map<string, string[]>>(new Map())
  // The id of the node currently under the cursor.
  const hoverIdRef = useRef<string | null>(null)
  // Initial-fit flag — we want to fit-once, not on every prop change.
  const hasFittedRef = useRef(false)

  const [size, setSize] = useState({ width: 600, height: 400 })

  // Hold the d3-zoom behavior in a ref so resize/edge-threshold changes
  // do not detach and re-attach the gesture handler (which would reset
  // d3-zoom's internal transform). The handler is attached once in the
  // mount-only effect below; subsequent resize/zoom changes just update
  // the behaviour's extent or call `.transform` on it.
  const zoomRef = useRef<ZoomBehavior<HTMLCanvasElement, unknown> | null>(null)

  // Build the d3-force graph whenever entries change. The simulation
  // is anchored to the backend's PCA coordinates via `fx`/`fy` so the
  // layout ends up near the semantic structure the kernel produced.
  useEffect(() => {
    if (entries.length === 0) {
      nodesRef.current = []
      linksRef.current = []
      if (simRef.current) {
        simRef.current.stop()
        simRef.current = null
      }
      requestDraw()
      return
    }

    // Build nodes, anchoring to (tx, ty) = (coords_2d.x, coords_2d.y).
    // We keep `tx`/`ty` private so we can still call forceX/forceY
    // as soft pulls in case the user wants to perturb the layout.
    const oldNodeById = new Map(nodesRef.current.map((n) => [n.id, n]))
    const nodes: SimNode[] = entries.map((e) => {
      const [x, y] = e.coords_2d
      const prev = oldNodeById.get(e.id)
      return {
        ...e,
        x: prev?.x ?? x,
        y: prev?.y ?? y,
        vx: prev?.vx ?? 0,
        vy: prev?.vy ?? 0,
        tx: x,
        ty: y,
      }
    })
    nodesRef.current = nodes

    // Build links from top_neighbors. We deduplicate so each pair has
    // at most one edge in either direction.
    const linkSet = new Set<string>()
    const links: SimLink[] = []
    for (const node of nodes) {
      for (const n of node.top_neighbors as MemoryMapNeighbor[]) {
        if (!nodes.find((m) => m.id === n.id)) continue
        if (n.similarity < edgeThreshold) continue
        const key = node.id < n.id ? `${node.id}|${n.id}` : `${n.id}|${node.id}`
        if (linkSet.has(key)) continue
        linkSet.add(key)
        links.push({ source: node.id, target: n.id, similarity: n.similarity })
      }
    }
    linksRef.current = links

    // Adjacency for hover highlighting.
    const nbrs = new Map<string, string[]>()
    for (const link of links) {
      const s = typeof link.source === 'string' ? link.source : (link.source as SimNode).id
      const t = typeof link.target === 'string' ? link.target : (link.target as SimNode).id
      nbrs.set(s, [...(nbrs.get(s) ?? []), t])
      nbrs.set(t, [...(nbrs.get(t) ?? []), s])
    }
    neighboursRef.current = nbrs

    if (simRef.current) {
      simRef.current.stop()
    }

    const sim = forceSimulation<SimNode>(nodes)
      .force(
        'link',
        forceLink<SimNode, SimLink>(links)
          .id((d) => d.id)
          .distance(0.05)
          .strength(0.3),
      )
      .force('charge', forceManyBody().strength(-0.02))
      .force('collide', forceCollide<SimNode>().radius(0.04).strength(0.7))
      .force('x', forceX<SimNode>((d) => d.tx).strength(0.6))
      .force('y', forceY<SimNode>((d) => d.ty).strength(0.6))
      .force('center', forceCenter(0, 0).strength(0.05))
      .alpha(animate ? 0.6 : 0.001)
      .alphaDecay(0.05)
      .on('tick', requestDraw)

    simRef.current = sim
    requestDraw()
    return () => {
      sim.stop()
    }
    // We intentionally re-build the graph when the entries identity
    // changes; the `animate` flag is handled in a separate effect
    // below so toggling it does not rebuild the graph. The
    // `edgeThreshold` filter is also part of the graph build so it is
    // intentionally in the dep array.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [entries, edgeThreshold])

  // P0-2: Animate toggle. Restart or stop the running simulation
  // without rebuilding the graph. The `animate` flag was previously
  // only read at graph-build time, so flipping the switch was a no-op.
  useEffect(() => {
    const sim = simRef.current
    if (!sim) return
    if (animate) {
      sim.alpha(0.6).restart()
    } else {
      sim.stop()
    }
  }, [animate])

  // P1-4: Reset the initial-fit flag whenever the entries identity
  // changes (filter, data refresh, etc). Without this, a filter change
  // re-builds the graph but leaves the camera at the old fit, so the
  // new node positions may be off-screen.
  useEffect(() => {
    hasFittedRef.current = false
  }, [entries])

  // Resize observer — keep canvas crisp on container resize.
  useEffect(() => {
    const container = containerRef.current
    if (!container) return
    const ro = new ResizeObserver(([entry]) => {
      const w = Math.max(1, Math.floor(entry?.contentRect.width ?? 0))
      const h = Math.max(1, Math.floor(entry?.contentRect.height ?? 0))
      setSize({ width: w, height: h })
    })
    ro.observe(container)
    return () => ro.disconnect()
  }, [])

  // P1-5: Wheel + drag zoom/pan via d3-zoom, attached ONCE on mount.
  // We keep the zoom behavior stable across resizes so d3-zoom's
  // internal transform is preserved; subsequent size changes only
  // update the scale extent / transform through the existing behavior.
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const z: ZoomBehavior<HTMLCanvasElement, unknown> = zoom<HTMLCanvasElement, unknown>()
      .scaleExtent([0.2, 8])
      .on('zoom', (event) => {
        transformRef.current = { k: event.transform.k, x: event.transform.x, y: event.transform.y }
        requestDraw()
      })
    const sel = select(canvas)
    sel.call(z)
    // Disable d3's default double-click zoom — we use dblclick for node open.
    sel.on('dblclick.zoom', null)
    zoomRef.current = z
    return () => {
      sel.on('.zoom', null)
      zoomRef.current = null
    }
  }, [])

  // P1-5 (cont): When the canvas size changes and the camera is still
  // at the default identity (i.e. the user has not panned/zoomed),
  // re-fit the view. If the user has already moved the camera, leave
  // it alone so we do not stomp their transform.
  useEffect(() => {
    const z = zoomRef.current
    const canvas = canvasRef.current
    if (!z || !canvas) return
    if (size.width < 2 || size.height < 2) return
    // Has the user pan/zoomed away from identity?
    const t = transformRef.current
    const isIdentity = t.k === 1 && t.x === 0 && t.y === 0
    if (!isIdentity) return
    hasFittedRef.current = false
  }, [size.width, size.height])

  // Fit-to-view on first entries arrival (and again after the entries
  // identity changes if the camera is still at identity — see above).
  useEffect(() => {
    if (hasFittedRef.current) return
    if (entries.length === 0) return
    if (size.width < 2 || size.height < 2) return
    const z = zoomRef.current
    const canvas = canvasRef.current
    if (!z || !canvas) return
    // Reserve a margin so nodes are not flush against the edges.
    const margin = 40
    const scale = Math.min((size.width - margin * 2) / 2, (size.height - margin * 2) / 2)
    const cx = size.width / 2
    const cy = size.height / 2
    select(canvas).call(z.transform, zoomIdentity.translate(cx, cy).scale(scale))
    transformRef.current = { k: scale, x: cx, y: cy }
    hasFittedRef.current = true
    requestDraw()
  }, [entries, size.width, size.height])

  // P0-3: Fly-to a specific node when the parent asks (e.g. search).
  //
  // The previous version called `sel.call(z.transform, ...)` before
  // capturing the start transform, which fired d3-zoom's `zoom` event
  // and updated `transformRef.current` to the destination — so the
  // rAF interpolation became a no-op (start = end). The fix is to
  // capture the start *first*, then drive the transform solely from
  // the rAF tick, with no synchronous zoom call.
  useEffect(() => {
    if (!flyToId) return
    const node = nodesRef.current.find((n) => n.id === flyToId)
    if (!node || node.x == null || node.y == null) return
    const targetScale = Math.max(transformRef.current.k, 2.5)
    const endK = targetScale
    const endX = size.width / 2 - node.x * endK
    const endY = size.height / 2 - node.y * endK
    // Capture the previous transform BEFORE any zoom call. This is
    // the start of the easing curve.
    const startK = transformRef.current.k
    const startX = transformRef.current.x
    const startY = transformRef.current.y
    const t0 = performance.now()
    // Ease-out cubic: 1 - (1 - t)^3.
    const ease = (t: number) => 1 - (1 - t) ** 3
    const tick = (now: number) => {
      const p = Math.min(1, (now - t0) / 450)
      const e = ease(p)
      const kk = startK + (endK - startK) * e
      const xx = startX + (endX - startX) * e
      const yy = startY + (endY - startY) * e
      transformRef.current = { k: kk, x: xx, y: yy }
      requestDraw()
      if (p < 1) requestAnimationFrame(tick)
    }
    requestAnimationFrame(tick)
  }, [flyToId, size.width, size.height])

  // Drawing loop. We schedule a single rAF per state change.
  const drawScheduledRef = useRef(false)
  const requestDraw = useCallback(() => {
    if (drawScheduledRef.current) return
    drawScheduledRef.current = true
    requestAnimationFrame(() => {
      drawScheduledRef.current = false
      draw()
    })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedId, hoverIdRef.current])

  // Track the hover id in a ref so the next draw picks it up.
  useEffect(() => {
    hoverIdRef.current = selectedId
    requestDraw()
  }, [selectedId, requestDraw])

  const draw = useCallback(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    const dpr = window.devicePixelRatio || 1
    const cssW = size.width
    const cssH = size.height
    if (canvas.width !== Math.floor(cssW * dpr) || canvas.height !== Math.floor(cssH * dpr)) {
      canvas.width = Math.floor(cssW * dpr)
      canvas.height = Math.floor(cssH * dpr)
      canvas.style.width = `${cssW}px`
      canvas.style.height = `${cssH}px`
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    ctx.clearRect(0, 0, cssW, cssH)

    const { k, x, y } = transformRef.current
    // Apply the d3-zoom transform.
    ctx.translate(x, y)
    ctx.scale(k, k)

    const nodes = nodesRef.current
    const links = linksRef.current
    const sel = selectedId
    const neighbourSet = sel != null ? new Set(neighboursRef.current.get(sel) ?? []) : null

    // Edges first so they sit under the nodes.
    if (links.length > 0) {
      ctx.lineWidth = 1 / k
      for (const link of links) {
        const s = link.source as SimNode
        const t = link.target as SimNode
        if (s.x == null || s.y == null || t.x == null || t.y == null) continue
        const sim = (link as SimLink & { similarity?: number }).similarity ?? 0.7
        let opacity = 0.05 + (sim - 0.7) * 0.5
        if (neighbourSet) {
          const isHighlighted =
            (s.id === sel && neighbourSet.has(t.id)) || (t.id === sel && neighbourSet.has(s.id))
          opacity = isHighlighted ? Math.min(0.85, opacity * 4) : 0.02
        }
        ctx.strokeStyle = `rgba(120, 120, 140, ${Math.max(0.02, opacity)})`
        ctx.beginPath()
        ctx.moveTo(s.x, s.y)
        ctx.lineTo(t.x, t.y)
        ctx.stroke()
      }
    }

    // Nodes.
    for (const n of nodes) {
      if (n.x == null || n.y == null) continue
      const r = nodeRadius(n)
      const baseFill: string = TIER_FILL[n.tier] ?? TIER_FILL.warm ?? '#f59e0b'
      let alpha = 1
      if (sel != null) {
        if (n.id === sel) alpha = 1
        else if (neighbourSet?.has(n.id)) alpha = 0.95
        else alpha = 0.2
      } else if (hoverIdRef.current && hoverIdRef.current !== n.id) {
        const hoverNeighbours = Array.isArray(neighboursRef.current.get(hoverIdRef.current))
          ? neighboursRef.current.get(hoverIdRef.current)!
          : []
        alpha = hoverNeighbours.includes(n.id) ? 0.95 : 0.25
      }
      ctx.globalAlpha = alpha
      ctx.fillStyle = baseFill
      ctx.save()
      ctx.translate(n.x, n.y)
      shapePath(ctx, n.mem_type, r / k)
      ctx.restore()
      if (n.id === sel) {
        ctx.globalAlpha = 1
        ctx.strokeStyle = 'rgba(255, 255, 255, 0.9)'
        ctx.lineWidth = 2 / k
        ctx.beginPath()
        ctx.arc(n.x, n.y, r / k + 2 / k, 0, Math.PI * 2)
        ctx.stroke()
      }
    }
    ctx.globalAlpha = 1
  }, [selectedId, size.width, size.height])

  // Mouse → node hit-test. We re-use the same screen↔world transform
  // the draw loop uses (inverted) so accuracy stays exact.
  const handlePointer = useCallback(
    (e: React.PointerEvent<HTMLCanvasElement>) => {
      const rect = (e.currentTarget as HTMLCanvasElement).getBoundingClientRect()
      const px = e.clientX - rect.left
      const py = e.clientY - rect.top
      const { k, x, y } = transformRef.current
      const wx = (px - x) / k
      const wy = (py - y) / k
      let best: SimNode | null = null
      let bestD = Infinity
      for (const n of nodesRef.current) {
        if (n.x == null || n.y == null) continue
        // Hit area = visual radius + HIT_TEST_PADDING, converted to
        // world units by dividing by the current zoom scale `k`.
        const r = (nodeRadius(n) + HIT_TEST_PADDING) / k
        const dx = n.x - wx
        const dy = n.y - wy
        const d2 = dx * dx + dy * dy
        if (d2 < r * r && d2 < bestD) {
          best = n
          bestD = d2
        }
      }
      const newId = best?.id ?? null
      if (newId !== hoverIdRef.current) {
        hoverIdRef.current = newId
        onHover?.(newId)
        requestDraw()
      }
    },
    [onHover, requestDraw],
  )

  const handleClick = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      const rect = (e.currentTarget as HTMLCanvasElement).getBoundingClientRect()
      const px = e.clientX - rect.left
      const py = e.clientY - rect.top
      const { k, x, y } = transformRef.current
      const wx = (px - x) / k
      const wy = (py - y) / k
      let best: SimNode | null = null
      let bestD = Infinity
      for (const n of nodesRef.current) {
        if (n.x == null || n.y == null) continue
        // Hit area = visual radius + HIT_TEST_PADDING (world units).
        const r = (nodeRadius(n) + HIT_TEST_PADDING) / k
        const dx = n.x - wx
        const dy = n.y - wy
        const d2 = dx * dx + dy * dy
        if (d2 < r * r && d2 < bestD) {
          best = n
          bestD = d2
        }
      }
      if (best) onSelect?.(best.id)
    },
    [onSelect],
  )

  // Stash transform/zoom in `useMemo`-stable refs so the draw loop
  // closure stays current without re-creating the effect.
  const dprAwareSize = useMemo(
    () => ({ width: size.width, height: size.height }),
    [size.width, size.height],
  )

  return (
    <div
      ref={containerRef}
      className="relative h-full w-full overflow-hidden rounded-md border bg-background"
      data-testid="embedding-canvas"
      style={{ minHeight: 360 }}
    >
      <canvas
        ref={canvasRef}
        width={dprAwareSize.width}
        height={dprAwareSize.height}
        onPointerMove={handlePointer}
        onPointerLeave={() => {
          if (hoverIdRef.current != null) {
            hoverIdRef.current = null
            onHover?.(null)
            requestDraw()
          }
        }}
        onClick={handleClick}
        style={{ display: 'block', width: '100%', height: '100%', cursor: 'grab' }}
      />
    </div>
  )
}
