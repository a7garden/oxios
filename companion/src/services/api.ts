import { decodePairOfferUrl } from '@/pairing/decode-offer';
import {
  RpcClient,
  createStableLogicalRpcClient,
  directEndpointUrls,
  directPathForEndpoint,
  FrameType,
  NoiseXXInitiator,
  decodeFrame,
  encodeFrame,
  openAuthenticatedDirectEndpoint
} from '@/transport';
import type {
  ConnectionPath,
  ConnectionState,
  HostProfile,
  PairingOffer,
  PhysicalRpcTransport,
  SessionSummary,
  Persona,
  StatusResult
} from '@/transport';
import {
  deleteHostProfile,
  listHostProfiles,
  loadHostProfile,
  saveDeviceToken,
  saveHostProfile
} from '@/keychain/host-store';

export type { HostProfile, PairingOffer };
export type ClientPath = 'Direct · LAN' | 'Direct · Tailscale';

export type ClientHandle = {
  authVerify(
    deviceId: string,
    token: string
  ): Promise<{ verified: boolean; device_id: string }>;
  statusGet(): Promise<StatusResult>;
  sessionList(): Promise<{ sessions: SessionSummary[] }>;
  sessionCreate(params?: {
    persona_id?: string;
    project_id?: string;
  }): Promise<{
    id: string;
    active_persona_id: string | null;
    project_id: string | null;
    created_at: string;
  }>;
  personaList(): Promise<{ personas: Persona[] }>;
  personaActivate(
    persona_id: string
  ): Promise<{ active_persona_id: string }>;
  chatSend(params: {
    content: string;
    session_id?: string;
    persona_id?: string;
  }): Promise<{ response: string }>;

  chatSubscribe(
    session_id: string,
    listener: (event: unknown) => void
  ): () => void;
  agentStatus(listener: (event: unknown) => void): () => void;

  getState(): ConnectionState;
  onStateChange(listener: (state: ConnectionState) => void): () => void;
  close(): void;

  getPath(): ClientPath;
};

export const hosts = {
  list: (): Promise<HostProfile[]> => listHostProfiles(),
  load: (id: string): Promise<HostProfile | null> => loadHostProfile(id),
  save: (profile: HostProfile): Promise<void> => saveHostProfile(profile),
  saveDeviceToken: (hostId: string, token: string): Promise<void> =>
    saveDeviceToken(hostId, token),
  remove: (id: string): Promise<void> => deleteHostProfile(id)
};

export const pairing = {
  decodeOfferUrl: (url: string): PairingOffer => decodePairOfferUrl(url),
  completeHandshake: async (_offer: PairingOffer): Promise<{ deviceToken: string; deviceId: string }> => {
    throw new TransportOfflineError('completeHandshake: runtime WebSocket dial not wired');
  }
};

export class TransportOfflineError extends Error {
  readonly kind = 'transport-offline' as const;
}

export function isTransportOffline(err: unknown): err is TransportOfflineError {
  return err instanceof TransportOfflineError;
}

const clients = new Map<string, ClientHandle>();
const pending = new Map<string, Promise<ClientHandle>>();

export async function getClient(hostId: string): Promise<ClientHandle> {
  const cached = clients.get(hostId);
  if (cached) return cached;
  const inFlight = pending.get(hostId);
  if (inFlight) return inFlight;
  const promise = buildClientForHost(hostId);
  pending.set(hostId, promise);
  try {
    return await promise;
  } finally {
    pending.delete(hostId);
  }
}

export async function dropClient(hostId: string): Promise<void> {
  const client = clients.get(hostId);
  if (client) {
    client.close();
    clients.delete(hostId);
  }
}

async function buildClientForHost(hostId: string): Promise<ClientHandle> {
  const profile = await loadHostProfile(hostId);
  if (!profile) throw new Error(`host not found: ${hostId}`);
  if (!profile.deviceToken || !profile.deviceId) {
    throw new Error('host has no device token — re-pair required');
  }

  const opened = await openAuthenticatedDirectEndpoint(
    profile,
    (url) => openNoiseTransport(url, profile.publicKeyB64)
  );
  if (!opened) throw new Error('no direct endpoint reachable');

  const logical = createStableLogicalRpcClient(opened.client, opened.path);
  const rpc = new RpcClient(logical);
  const verify = await rpc.authVerify(profile.deviceId, profile.deviceToken);
  if (!verify.verified) {
    logical.close();
    throw new Error('device token rejected — re-pair required');
  }

  const path: ClientPath = opened.path === 'tailscale' ? 'Direct · Tailscale' : 'Direct · LAN';
  const handle = wrapHandle(rpc, logical, path);
  clients.set(hostId, handle);
  return handle;
}

function wrapHandle(
  rpc: RpcClient,
  logical: { close(): void },
  path: ClientPath
): ClientHandle {
  return {
    authVerify: rpc.authVerify.bind(rpc),
    statusGet: rpc.statusGet.bind(rpc),
    sessionList: rpc.sessionList.bind(rpc),
    sessionCreate: rpc.sessionCreate.bind(rpc),
    personaList: rpc.personaList.bind(rpc),
    personaActivate: rpc.personaActivate.bind(rpc),
    chatSend: rpc.chatSend.bind(rpc),
    chatSubscribe: rpc.chatSubscribe.bind(rpc),
    agentStatus: rpc.agentStatus.bind(rpc),
    getState: rpc.getState.bind(rpc),
    onStateChange: rpc.onStateChange.bind(rpc),
    close: () => {
      rpc.close();
      logical.close();
    },
    getPath: () => path
  };
}

function openNoiseTransport(
  endpoint: string,
  remoteStaticPublicKeyB64: string
): PhysicalRpcTransport {
  throw new TransportOfflineError(
    `noise transport adapter not initialised for ${endpoint}`
  );
}

export {
  decodeFrame,
  directEndpointUrls,
  directPathForEndpoint,
  encodeFrame,
  FrameType,
  NoiseXXInitiator
};
export type { ConnectionPath };
