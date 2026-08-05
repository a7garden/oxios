/**
 * Domain types re-exported from the transport module so screens can
 * reference `HostProfile`, `SessionSummary`, `Persona`, and
 * `PairingOffer` from one place without leaking internal paths.
 *
 * Anything that originates from the wire belongs to the transport
 * module; the screens import only via this barrel.
 */

export type {
  HostProfile,
  PairingOffer,
  DirectEndpoint,
  ConnectionPath,
  ConnectionState,
  ConnectionHealth,
  RpcRequest,
  RpcSuccess,
  RpcError,
  RpcResponse,
  RpcNotification,
  PhysicalRpcTransport
} from '@/transport';

export type {
  StatusResult,
  SessionSummary,
  Persona
} from '@/transport/rpc-client';

/* ------------------------------------------------------------------ */
/* Streaming event shapes we surface to the UI layer.                */
/*                                                                    */
/* The wire format is JSON-RPC 2.0 envelopes; the daemon's            */
/* `chat.subscribe` server-streaming emits a stream of these. The     */
/* transport module passes the raw `unknown` to subscribers. The      */
/* chat screen normalises these into the discriminated union below    */
/* for rendering — every other field is intentionally elided.         */
/* ------------------------------------------------------------------ */

export type ChatRole = 'user' | 'assistant' | 'system';

export type ChatMessage = {
  id: string;
  role: ChatRole;
  content: string;
  created_at: string;
};

export type AgentStatus = {
  state: 'idle' | 'thinking' | 'acting' | 'waiting' | 'error';
  detail?: string;
};

export type SubscribeEvent =
  | { kind: 'message'; session_id: string; message: ChatMessage }
  | { kind: 'agent.status'; session_id?: string; state: AgentStatus['state']; detail?: string }
  | { kind: 'pong'; ts: number }
  | { kind: 'unknown' };
