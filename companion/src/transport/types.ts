export type ConnectionPath = 'lan' | 'tailscale'
export type ConnectionState = 'connecting' | 'handshaking' | 'connected' | 'disconnected' | 'reconnecting' | 'auth-failed'

export interface DirectEndpoint { url: string; kind: ConnectionPath }
export interface HostProfile {
  id: string
  name: string
  endpoint: string
  deviceId: string
  deviceToken?: string
  publicKeyB64: string
  lastConnected: number
  endpoints?: DirectEndpoint[]
}
export interface PairingOffer {
  v: number
  endpoint: string
  device_id: string
  public_key_b64: string
  endpoints?: Array<string | DirectEndpoint>
  scope?: string | string[]
}
export interface RpcRequest { jsonrpc: '2.0'; id: string | number; method: string; params?: unknown }
export interface RpcSuccess { jsonrpc: '2.0'; id: string | number; result: unknown }
export interface RpcError { jsonrpc: '2.0'; id: string | number; error: { code: number; message: string; data?: unknown } }
export type RpcResponse = RpcSuccess | RpcError
export interface RpcNotification { jsonrpc: '2.0'; method: string; params?: unknown }
export interface ConnectionHealth {
  verdict: 'connected' | 'connecting' | 'disconnected'
  label: string
  path?: ConnectionPath
  hint?: string
}

export interface PhysicalRpcTransport {
  getState(): ConnectionState
  onStateChange(listener: (state: ConnectionState) => void): () => void
  request(method: string, params?: unknown): Promise<unknown>
  subscribe(method: string, params: unknown, listener: (params: unknown) => void): () => void
  close(): void
}
