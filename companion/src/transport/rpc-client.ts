import type { PhysicalRpcTransport } from './types'

export interface StatusResult {
  protocol_version: string
  min_client_version: string
  device_id: string
  paired_count: number
}

export interface SessionSummary {
  id: string
  title: string
  project_id: string | null
  message_count: number
  created_at: string
  updated_at: string
}

export interface Persona {
  id: string
  name: string
  role: string
  description: string
  model: string
  enabled: boolean
  capabilities: string[]
  is_active: boolean
}

export class RpcClient {
  constructor(private transport: PhysicalRpcTransport) {}

  authVerify(device_id: string, token: string) {
    return this.call<{ verified: boolean; device_id: string }>('auth.verify', { device_id, token })
  }

  statusGet() {
    return this.call<StatusResult>('status.get')
  }

  sessionList() {
    return this.call<{ sessions: SessionSummary[] }>('session.list')
  }

  sessionCreate(params: { persona_id?: string; project_id?: string } = {}) {
    return this.call<{
      id: string
      active_persona_id: string | null
      project_id: string | null
      created_at: string
    }>('session.create', params)
  }

  personaList() {
    return this.call<{ personas: Persona[] }>('persona.list')
  }

  personaActivate(persona_id: string) {
    return this.call<{ active_persona_id: string }>('persona.activate', { persona_id })
  }

  chatSend(params: { content: string; session_id?: string; persona_id?: string }) {
    return this.call<{ response: string }>('chat.send', params)
  }

  chatSubscribe(session_id: string, listener: (event: unknown) => void) {
    return this.transport.subscribe('chat.subscribe', { session_id }, listener)
  }

  agentStatus(listener: (event: unknown) => void) {
    return this.transport.subscribe('agent.status', {}, listener)
  }

  getState() {
    return this.transport.getState()
  }

  onStateChange(listener: Parameters<PhysicalRpcTransport['onStateChange']>[0]) {
    return this.transport.onStateChange(listener)
  }

  close() {
    this.transport.close()
  }

  private async call<T>(method: string, params?: unknown) {
    return (await this.transport.request(method, params)) as T
  }
}
