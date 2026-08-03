/**
 * TerminalView — one xterm.js pane bound to a single backend PTY session.
 *
 * Lifecycle:
 *   1. Lazy-load xterm + addons via `import()` (keeps the initial bundle small).
 *   2. Read theme colors from `--color-surface-sunken` / `--color-text` on
 *      `document.documentElement` so the terminal tracks the current theme.
 *   3. Open the terminal in the container <div>, attach FitAddon, fit once.
 *   4. Open the WebSocket from `codeApi.terminalWsUrl(tid)`.
 *   5. Pump WS messages into the terminal and forward keystrokes back.
 *   6. On PTY resize, send `{ type: "resize", cols, rows }` over the WS so the
 *      backend matches the viewport. A ResizeObserver on the container catches
 *      panel/window resizes; visibility flips call `fit()` directly.
 *   7. On unmount: close the WS, dispose the terminal + addons, ask the
 *      daemon to delete the PTY (idempotent — we ignore the result).
 *
 * Hidden tabs (display:none) stay mounted so scrollback & the WS survive tab
 * switching; we just re-fit when a tab becomes visible again.
 */
import type { FitAddon as FitAddonType } from '@xterm/addon-fit'
import type { IDisposable, Terminal } from '@xterm/xterm'
import { useEffect, useRef, useState } from 'react'
import '@xterm/xterm/css/xterm.css'
import { codeApi } from '@/lib/code-api'

interface TerminalViewProps {
  /** Backend PTY id from `codeApi.createTerminal`. */
  terminalId: string
  /** When false the pane is hidden — we still keep the WS alive, but the
   *  FitAddon only re-runs once the pane becomes visible again. */
  active: boolean
  /** Optional shell override forwarded to `codeApi.createTerminal`. Only
   *  honoured when this terminal is *newly created*; re-renders of an
   *  already-open WS ignore it. */
  shell?: string
}

/**
 * Pull the current value of a CSS custom property off the root element.
 * Trims whitespace and returns `undefined` for empty/missing values so callers
 * can fall back to a safe default rather than passing "" into xterm.
 */
function readCssVar(name: string): string | undefined {
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  return raw ? raw : undefined
}

/** Build the xterm theme object from current oxi design tokens. */
function readTheme() {
  return {
    background: readCssVar('--color-surface-sunken'),
    foreground: readCssVar('--color-text'),
    cursor: readCssVar('--color-text'),
    selectionBackground: readCssVar('--color-text-muted'),
  }
}

export function TerminalView({ terminalId, active }: TerminalViewProps) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const termRef = useRef<Terminal | null>(null)
  const fitRef = useRef<FitAddonType | null>(null)
  const wsRef = useRef<WebSocket | null>(null)
  const disposeDataRef = useRef<IDisposable | null>(null)
  const disposeResizeRef = useRef<IDisposable | null>(null)
  const resizeObserverRef = useRef<ResizeObserver | null>(null)

  // Keep `status` so the chrome can show a connecting/closed badge later.
  // Today nothing visible consumes it, but the WS lifecycle still drives it.
  const [, setStatus] = useState<'connecting' | 'open' | 'closed' | 'error'>('connecting')

  // Connect / tear down when the terminal id changes (i.e. on mount, and
  // when the user closes one tab and opens another in the same slot).
  useEffect(() => {
    if (!containerRef.current) return
    let cancelled = false

    async function boot() {
      const [{ Terminal: TerminalCtor }, { FitAddon: FitAddonCtor }, { WebLinksAddon }] =
        await Promise.all([
          import('@xterm/xterm'),
          import('@xterm/addon-fit'),
          import('@xterm/addon-web-links'),
        ])
      if (cancelled || !containerRef.current) return

      const term = new TerminalCtor({
        cursorBlink: true,
        convertEol: true,
        fontFamily:
          'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace',
        fontSize: 12,
        lineHeight: 1.2,
        theme: readTheme(),
        allowProposedApi: true,
        scrollback: 5000,
      })

      const fit = new FitAddonCtor()
      term.loadAddon(fit)
      term.loadAddon(new WebLinksAddon())
      term.open(containerRef.current)

      const sendResize = () => {
        const ws = wsRef.current
        if (!ws || ws.readyState !== WebSocket.OPEN) return
        try {
          ws.send(JSON.stringify({ type: 'resize', cols: term.cols, rows: term.rows }))
        } catch {
          /* closing socket will reject the next send anyway */
        }
      }

      const safeFit = () => {
        try {
          fit.fit()
          sendResize()
        } catch {
          /* container transiently has zero width during layout */
        }
      }

      termRef.current = term
      fitRef.current = fit

      // Mirror xterm's own resize event (font load, options change) onto the WS.
      disposeResizeRef.current = term.onResize(sendResize)

      // Forward keystrokes; persist the disposer so cleanup can unhook.
      disposeDataRef.current = term.onData((data) => {
        const ws = wsRef.current
        if (ws && ws.readyState === WebSocket.OPEN) ws.send(data)
      })

      // Container size changes (panel drag, window resize, sibling panels
      // appearing) — fit then push the new dimensions to the PTY.
      const ro = new ResizeObserver(() => safeFit())
      ro.observe(containerRef.current!)
      resizeObserverRef.current = ro

      // Open the WS AFTER the terminal is mounted so the first PTY output
      // renders as soon as it arrives.
      const ws = new WebSocket(codeApi.terminalWsUrl(terminalId))
      wsRef.current = ws

      ws.addEventListener('open', () => {
        if (cancelled) return
        setStatus('open')
        // Tell the PTY how big the viewport is now that the canvas is laid out.
        requestAnimationFrame(safeFit)
      })

      ws.addEventListener('message', (ev) => {
        if (typeof ev.data === 'string') term.write(ev.data)
      })

      ws.addEventListener('error', () => {
        if (cancelled) return
        setStatus('error')
      })

      ws.addEventListener('close', () => {
        if (cancelled) return
        setStatus('closed')
      })
    }

    void boot()

    return () => {
      cancelled = true
      // Detach listeners FIRST so any in-flight `onResize` can't fire against
      // a half-disposed terminal.
      disposeDataRef.current?.dispose()
      disposeResizeRef.current?.dispose()
      resizeObserverRef.current?.disconnect()
      disposeDataRef.current = null
      disposeResizeRef.current = null
      resizeObserverRef.current = null

      const ws = wsRef.current
      wsRef.current = null
      if (ws && ws.readyState <= WebSocket.OPEN) ws.close()

      const fit = fitRef.current
      const term = termRef.current
      fitRef.current = null
      termRef.current = null
      try {
        fit?.dispose()
      } catch {
        /* addon already torn down */
      }
      try {
        term?.dispose()
      } catch {
        /* terminal already torn down */
      }

      // Best-effort backend cleanup. The daemon also reaps orphans, so a
      // failure here (e.g. socket already gone) is safe to ignore.
      void codeApi.deleteTerminal(terminalId).catch(() => undefined)
    }
  }, [terminalId])

  // Re-fit when the pane becomes visible after being hidden — xterm caches
  // its canvas size and would otherwise show a blank grid until resize.
  useEffect(() => {
    if (!active) return
    try {
      fitRef.current?.fit()
    } catch {
      /* container not yet sized */
    }
    termRef.current?.focus()
  }, [active])

  return (
    <div
      ref={containerRef}
      role="tabpanel"
      aria-hidden={!active}
      className="h-full w-full bg-surface-sunken"
      // Hide rather than unmount so the WS / scrollback survive tab switching.
      // When `active` flips back we re-fit in the effect above.
      style={{ display: active ? 'block' : 'none' }}
    />
  )
}
