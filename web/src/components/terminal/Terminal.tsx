// Terminal UI component (RFC-038 §10.3 + §10.7).
//
// Renders a ghostty-web Terminal on a div, wires it to the PTY WebSocket
// hook, and exposes header status / kill / reconnect controls.

import { Terminal as GhosttyTerminal, FitAddon, init as ghosttyInit, TextDecoder as GhosttyTextDecoder } from 'ghostty-web';
import { PowerOff, RotateCw } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { Button } from '@/components/ui/button';
import { useTerminalSocket } from './useTerminalSocket';

const WS_PATH = '/api/terminal/stream';

function utf8Encode(input: string): Uint8Array {
  return new TextEncoder().encode(input);
}

export type TerminalProps = {
  /** Re-attach to an existing session id (null = open a new one). */
  sessionId?: string | null;
  /** Default shell to spawn. Empty → server default. */
  shell?: string;
};

export function Terminal({ sessionId: initialSessionId = null, shell }: TerminalProps): JSX.Element {
  const { t } = useTranslation();
  const containerRef = useRef<HTMLDivElement | null>(null);
  const termRef = useRef<GhosttyTerminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const [ready, setReady] = useState(false);
  const [resolvedSessionId, setResolvedSessionId] = useState<string | null>(initialSessionId);
  const [shellName, setShellName] = useState<string | null>(shell ?? null);

  const { status, errorMessage, sessionId, shell: socketShell, handle } = useTerminalSocket({
    path: WS_PATH,
    cols: 80,
    rows: 24,
    ...(shell ? { shell } : {}),
    onEvent: (event) => {
      const term = termRef.current;
      if (!term) return;
      if (event.kind === 'binary') {
        term.write(event.bytes);
        return;
      }
      // Control frames: Opened already routed through onEvent in the hook,
      // here we only react to Exit visually.
      if (event.frame.type === 'exit') {
        term.write('\r\n[process exited]\r\n');
      }
    },
  });

  // Initialize ghostty-web + FitAddon once on mount.
  useEffect(() => {
    let disposed = false;
    let ro: ResizeObserver | null = null;
    (async () => {
      await ghosttyInit();
      if (disposed || !containerRef.current) return;
      const term = new GhosttyTerminal({
        cols: 80,
        rows: 24,
        fontFamily: 'Menlo, Monaco, Consolas, monospace',
        fontSize: 13,
        scrollback: 5000,
      });
      const fit = new FitAddon();
      term.loadAddon(fit);
      term.open(containerRef.current);
      try {
        fit.fit();
      } catch {
        // best-effort; container may not have measurable dimensions yet
      }
      term.onData((data: string) => {
        handle.sendBytes(utf8Encode(data));
      });
      term.onResize(({ cols, rows }: { cols: number; rows: number }) => {
        handle.resize(cols, rows);
      });
      termRef.current = term;
      fitRef.current = fit;
      setReady(true);
      ro = new ResizeObserver(() => {
        try {
          fit.fit();
        } catch {
          // container may be detached mid-fit
        }
      });
      ro.observe(containerRef.current);
    })();
    return () => {
      disposed = true;
      if (ro) ro.disconnect();
      if (termRef.current) {
        try {
          termRef.current.dispose();
        } catch {
          // ghostty may have already torn down
        }
        termRef.current = null;
      }
      fitRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Reflect socket state into component state.
  useEffect(() => {
    if (sessionId) setResolvedSessionId(sessionId);
    if (socketShell) setShellName(socketShell);
  }, [sessionId, socketShell]);

  const headerLabel = (() => {
    if (status === 'connecting') return t('terminal.connecting', 'Connecting…');
    if (status === 'exited') return t('terminal.exited', 'Process exited');
    if (status === 'error') return errorMessage ?? t('terminal.error', 'Error');
    if (status === 'detached') return t('terminal.detached', 'Detached (reconnect?)');
    if (status === 'disabled') return t('terminal.disabled', 'Terminal is disabled');
    if (status === 'permission-denied')
      return t('terminal.permissionDenied', 'Not authorized to open a terminal');
    return resolvedSessionId ?? t('terminal.attached', 'Connected');
  })();

  const onReconnect = (): void => {
    handle.close();
    window.location.reload();
  };
  const onKill = (): void => {
    handle.close();
  };

  return (
    <div className="flex h-full w-full flex-col bg-background">
      <div className="flex items-center justify-between border-b bg-card px-3 py-1.5 text-xs">
        <div className="flex items-center gap-2">
          <span className="font-mono text-muted-foreground">
            {shellName ?? '/bin/zsh'}
          </span>
          <span className="text-muted-foreground">·</span>
          <span className="text-muted-foreground">{headerLabel}</span>
          {resolvedSessionId ? (
            <>
              <span className="text-muted-foreground">·</span>
              <span className="font-mono text-[10px] text-muted-foreground">
                {resolvedSessionId.slice(0, 12)}
              </span>
            </>
          ) : null}
        </div>
        <div className="flex items-center gap-1">
          {status === 'detached' && ready ? (
            <Button
              variant="ghost"
              size="sm"
              onClick={onReconnect}
              className="h-7 gap-1 px-2 text-xs"
            >
              <RotateCw className="h-3 w-3" />
              {t('terminal.reconnectButton', 'Reconnect')}
            </Button>
          ) : null}
          {ready ? (
            <Button
              variant="ghost"
              size="sm"
              onClick={onKill}
              className="h-7 gap-1 px-2 text-xs"
            >
              <PowerOff className="h-3 w-3" />
              {t('terminal.killButton', 'Kill')}
            </Button>
          ) : null}
        </div>
      </div>
      <div ref={containerRef} className="flex-1 overflow-hidden bg-black p-2" />
    </div>
  );
}

// Suppress unused import warning for GhosttyTextDecoder (kept for future use
// when we wire incremental UTF-8 decoding through binary frames).
void GhosttyTextDecoder;