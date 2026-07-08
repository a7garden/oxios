// Terminal WebSocket hook (RFC-038 §10.3).
//
// Layered on top of `useWebSocketTransport` (web/src/lib/ws-client.ts).
// Handles: PTY Open frame + control frames, binary PTY bytes,
// xterm.js-compatible render events, resize, re-attach, error states.

import { z } from 'zod/v4';

import { useCallback, useEffect, useRef, useState } from 'react';

import { connectWs, type WsController } from '@/lib/ws-client';

export type TerminalStatus =
  | 'connecting'
  | 'attached'
  | 'detached'
  | 'exited'
  | 'disabled'
  | 'permission-denied'
  | 'error';

const ControlFrame = z.discriminatedUnion('type', [
  z.object({
    type: z.literal('open'),
    session_id: z.string().optional(),
    shell: z.string().optional(),
    cols: z.number(),
    rows: z.number(),
  }),
  z.object({
    type: z.literal('opened'),
    session_id: z.string(),
    shell: z.string(),
    cols: z.number(),
    rows: z.number(),
  }),
  z.object({ type: z.literal('resize'), cols: z.number(), rows: z.number() }),
  z.object({ type: z.literal('close'), reason: z.string().optional() }),
  z.object({
    type: z.literal('exit'),
    code: z.number().optional(),
    signal: z.number().optional(),
  }),
  z.object({ type: z.literal('error'), message: z.string() }),
]);
type TerminalControlFrame = z.infer<typeof ControlFrame>;

export type TerminalEvent =
  | { kind: 'control'; frame: TerminalControlFrame }
  | { kind: 'binary'; bytes: Uint8Array };

export type UseTerminalSocketOpts = {
  path: string;
  cols: number;
  rows: number;
  shell?: string;
  onEvent: (event: TerminalEvent) => void;
};

export type TerminalSocketHandle = {
  sendBytes: (data: Uint8Array) => void;
  sendText: (data: string) => void;
  resize: (cols: number, rows: number) => void;
  close: () => void;
};

function toUint8Array(raw: ArrayBuffer | Uint8Array): Uint8Array {
  if (raw instanceof ArrayBuffer) return new Uint8Array(raw);
  const copy = new Uint8Array(raw.byteLength);
  copy.set(raw);
  return copy;
}

function parseControlFrame(text: string): TerminalControlFrame | null {
  const parsed = z
    .unknown()
    .pipe(ControlFrame)
    .safeParse(JSON.parse(text));
  return parsed.success ? parsed.data : null;
}

export function useTerminalSocket(opts: UseTerminalSocketOpts): {
  status: TerminalStatus;
  errorMessage: string | null;
  sessionId: string | null;
  shell: string | null;
  handle: TerminalSocketHandle;
} {
  const [status, setStatus] = useState<TerminalStatus>('connecting');
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [shell, setShell] = useState<string | null>(null);

  const ctrlRef = useRef<WsController | null>(null);
  const onEventRef = useRef(opts.onEvent);
  onEventRef.current = opts.onEvent;

  const sessionIdRef = useRef<string | null>(null);
  const initialShellRef = useRef(opts.shell);
  const initialColsRef = useRef(opts.cols);
  const initialRowsRef = useRef(opts.rows);

  useEffect(() => {
    let cancelled = false;
    const ctrl = connectWs(opts.path, {
      onOpen: () => {
        if (cancelled) return;
        const open: TerminalControlFrame = {
          type: 'open',
          ...(sessionIdRef.current ? { session_id: sessionIdRef.current } : {}),
          ...(initialShellRef.current ? { shell: initialShellRef.current } : {}),
          cols: initialColsRef.current,
          rows: initialRowsRef.current,
        };
        ctrl.send(JSON.stringify(open));
        setStatus('connecting');
      },
      onMessage: (msg) => {
        if (cancelled) return;
        if (typeof msg === 'string') {
          const frame = parseControlFrame(msg);
          if (!frame) return;
          if (frame.type === 'opened') {
            sessionIdRef.current = frame.session_id;
            setSessionId(frame.session_id);
            setShell(frame.shell);
            setStatus('attached');
          } else if (frame.type === 'exit') {
            setStatus('exited');
          } else if (frame.type === 'error') {
            setStatus('error');
            setErrorMessage(frame.message);
          }
          onEventRef.current({ kind: 'control', frame });
          return;
        }
        onEventRef.current({ kind: 'binary', bytes: toUint8Array(msg) });
      },
      onClose: () => {
        if (cancelled) return;
        if (sessionIdRef.current) setStatus('detached');
      },
      onError: (err) => {
        if (cancelled) return;
        setStatus('error');
        setErrorMessage(err instanceof Error ? err.message : String(err));
      },
    });
    ctrlRef.current = ctrl;
    return () => {
      cancelled = true;
      ctrl.close();
      ctrlRef.current = null;
    };
  }, [opts.path]);

  const sendText = useCallback((data: string) => {
    const ctrl = ctrlRef.current;
    if (!ctrl) return;
    ctrl.send(data);
  }, []);

  const sendBytes = useCallback((data: Uint8Array) => {
    const ctrl = ctrlRef.current;
    if (!ctrl) return;
    ctrl.send(data);
  }, []);

  const resize = useCallback((cols: number, rows: number) => {
    const ctrl = ctrlRef.current;
    if (!ctrl) return;
    const frame: TerminalControlFrame = { type: 'resize', cols, rows };
    ctrl.send(JSON.stringify(frame));
  }, []);

  const close = useCallback(() => {
    const ctrl = ctrlRef.current;
    if (!ctrl) return;
    const frame: TerminalControlFrame = { type: 'close' };
    ctrl.send(JSON.stringify(frame));
    ctrl.close();
  }, []);

  return {
    status,
    errorMessage,
    sessionId,
    shell,
    handle: { sendBytes, sendText, resize, close },
  };
}