import { useAuthStore } from '@/stores/auth'

const API_BASE = import.meta.env.VITE_API_BASE || ''

export class ApiError extends Error {
  constructor(
    public status: number,
    public statusText: string,
    public body?: string,
  ) {
    super(`API Error ${status}: ${statusText}`)
    this.name = 'ApiError'
  }
}

interface RequestOptions {
  method?: string
  body?: unknown
  params?: Record<string, string>
  headers?: Record<string, string>
  /** Skip JSON encoding — send body as-is (for file PUT with raw markdown) */
  rawBody?: boolean
}

export async function apiClient<T>(path: string, options?: RequestOptions): Promise<T> {
  const url = new URL(`${API_BASE}${path}`, window.location.origin)
  if (options?.params) {
    for (const [k, v] of Object.entries(options.params)) {
      url.searchParams.set(k, v)
    }
  }

  const token = useAuthStore.getState().token
  const isRawBody = options?.rawBody === true
  const res = await fetch(url.toString(), {
    method: options?.method ?? 'GET',
    headers: {
      'Content-Type': isRawBody ? 'text/markdown' : 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...options?.headers,
    },
    body: isRawBody
      ? ((options.body as string) ?? undefined)
      : options?.body
        ? JSON.stringify(options.body)
        : undefined,
  })

  if (!res.ok) {
    const text = await res.text().catch(() => '')
    throw new ApiError(res.status, res.statusText, text)
  }

  if (res.status === 204) return undefined as T
  const contentType = res.headers.get('content-type') ?? ''
  if (contentType.includes('application/json')) {
    return res.json() as Promise<T>
  }
  if (contentType.includes('text/') || contentType.includes('application/toml')) {
    return res.text() as Promise<T>
  }
  return res.json() as Promise<T>
}

export const api = {
  get: <T>(path: string, params?: Record<string, string>) => apiClient<T>(path, { params }),
  post: <T>(path: string, body?: unknown) => apiClient<T>(path, { method: 'POST', body }),
  put: <T>(path: string, body?: unknown, rawBody?: boolean) =>
    apiClient<T>(path, { method: 'PUT', body, rawBody }),
  patch: <T>(path: string, body?: unknown) => apiClient<T>(path, { method: 'PATCH', body }),
  delete: <T>(path: string) => apiClient<T>(path, { method: 'DELETE' }),
}
