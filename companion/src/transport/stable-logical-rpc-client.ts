import type { ConnectionPath, ConnectionState, PhysicalRpcTransport } from './types'

export class LogicalClientCutoverError extends Error {
  constructor() {
    super('RPC interrupted by connection migration')
  }
}

type Sub = {
  method: string
  params: unknown
  listener: (p: unknown) => void
  dispose: (() => void) | null
  cancelled: boolean
}

export interface StableLogicalRpcClient extends PhysicalRpcTransport {
  migrateTo(next: PhysicalRpcTransport, path: ConnectionPath, timeoutMs?: number): Promise<void>
  getActivePath(): ConnectionPath
  getGeneration(): number
}

function waitConnected(c: PhysicalRpcTransport, ms: number) {
  if (c.getState() === 'connected') return Promise.resolve()
  return new Promise<void>((res, rej) => {
    const timer = setTimeout(
      () => finish(() => rej(new Error('replacement authentication timed out'))),
      ms,
    )
    const off = c.onStateChange((s) => {
      if (s === 'connected') finish(res)
      else if (s === 'auth-failed' || s === 'disconnected') finish(() => rej(new Error(`replacement ${s}`)))
    })
    let done = false
    function finish(cb: () => void) {
      if (done) return
      done = true
      clearTimeout(timer)
      off()
      cb()
    }
  })
}

export function createStableLogicalRpcClient(
  initial: PhysicalRpcTransport,
  path: ConnectionPath,
): StableLogicalRpcClient {
  let active = initial
  let generation = 1
  let closed = false
  let state = initial.getState()
  let offState: () => void = () => {}
  const listeners = new Set<(s: ConnectionState) => void>()
  const subs = new Set<Sub>()

  const bind = () => {
    offState()
    const g = generation
    const session = active
    offState = session.onStateChange((s) => {
      if (!closed && g === generation && session === active) {
        state = s
        for (const l of listeners) l(s)
      }
    })
  }
  bind()

  return {
    getState: () => state,
    onStateChange(l) {
      listeners.add(l)
      return () => {
        listeners.delete(l)
      }
    },
    async request(m, p) {
      if (closed) throw new Error('Client closed')
      const g = generation
      const result = await active.request(m, p)
      if (g !== generation) throw new LogicalClientCutoverError()
      return result
    },
    subscribe(method, params, listener) {
      const r: Sub = { method, params, listener, dispose: null, cancelled: false }
      subs.add(r)
      const attach = (session: PhysicalRpcTransport, g: number) =>
        session.subscribe(method, params, (v) => {
          if (!r.cancelled && g === generation) listener(v)
        })
      r.dispose = attach(active, generation)
      return () => {
        r.cancelled = true
        r.dispose?.()
        subs.delete(r)
      }
    },
    close() {
      if (closed) return
      closed = true
      offState()
      for (const s of subs) s.dispose?.()
      subs.clear()
      active.close()
      state = 'disconnected'
    },
    async migrateTo(next, nextPath, ms = 12_000) {
      try {
        await waitConnected(next, ms)
      } catch (e) {
        next.close()
        throw e
      }
      if (closed) {
        next.close()
        throw new Error('Client closed')
      }
      const old = active
      const nextGeneration = generation + 1
      for (const s of subs) {
        const dispose = s.dispose
        s.dispose = next.subscribe(s.method, s.params, (v) => {
          if (!s.cancelled && generation === nextGeneration) s.listener(v)
        })
        dispose?.()
      }
      generation = nextGeneration
      active = next
      path = nextPath
      bind()
      state = next.getState()
      for (const l of listeners) l(state)
      old.close()
    },
    getActivePath: () => path,
    getGeneration: () => generation,
  }
}
