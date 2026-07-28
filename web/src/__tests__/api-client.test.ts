import { HttpResponse, http } from 'msw'
import { afterEach, describe, expect, it } from 'vitest'
import { ApiError, api, apiClient } from '@/lib/api-client'
import { useAuthStore } from '@/stores/auth'
import { server } from './msw/server'

// ---------------------------------------------------------------------------
// ApiError — a real instance is a thrown error carrying status, statusText,
// and the response body verbatim (callers pattern-match on statusText / body
// to differentiate retriable vs permanent failures).
// ---------------------------------------------------------------------------
describe('ApiError', () => {
  it('creates with status and message', () => {
    const err = new ApiError(404, 'Not Found', '{"error":"not found"}')
    expect(err.status).toBe(404)
    expect(err.statusText).toBe('Not Found')
    expect(err.body).toBe('{"error":"not found"}')
    expect(err.message).toBe('API Error 404: Not Found')
    expect(err.name).toBe('ApiError')
    expect(err).toBeInstanceOf(Error)
    expect(err).toBeInstanceOf(ApiError)
  })

  it('works without body', () => {
    const err = new ApiError(500, 'Internal Server Error')
    expect(err.body).toBeUndefined()
    expect(err.message).toBe('API Error 500: Internal Server Error')
  })
})

// ---------------------------------------------------------------------------
// apiClient() — exercised against the real MSW node server (no live network).
// Pins the observable contract the rest of the app depends on: query
// encoding, Authorization header, JSON/text/toml/204 parsing, JSON request
// bodies, raw markdown PUT, FormData content-type suppression, ApiError
// payload capture, and the 401 → logout side-effect.
// ---------------------------------------------------------------------------
describe('apiClient', () => {
  afterEach(() => {
    server.resetHandlers()
    useAuthStore.getState().setToken(null)
    sessionStorage.clear()
  })

  it('encodes params into the query string', async () => {
    const seen: { url: string }[] = []
    server.use(
      http.get('/api/echo', ({ request }) => {
        seen.push({ url: request.url })
        return HttpResponse.json({ ok: true })
      }),
    )
    const data = await apiClient<{ ok: boolean }>('/api/echo', {
      params: { q: 'hello world', tag: 'a&b' },
    })
    expect(data).toEqual({ ok: true })
    expect(seen[0]?.url).toContain('/api/echo')
    // URLSearchParams encodes spaces as `+` and `&` as `%26`.
    expect(seen[0]?.url).toContain('q=hello+world')
    expect(seen[0]?.url).toContain('tag=a%26b')
  })

  it('sends Authorization: Bearer <token> when the store holds one', async () => {
    useAuthStore.getState().setToken('secret-key')
    let auth: string | null = null
    server.use(
      http.get('/api/who', ({ request }) => {
        auth = request.headers.get('authorization')
        return HttpResponse.json({ ok: true })
      }),
    )
    await apiClient('/api/who')
    expect(auth).toBe('Bearer secret-key')
  })

  it('omits the Authorization header when no token is set', async () => {
    let auth: string | null = 'sentinel'
    server.use(
      http.get('/api/who', ({ request }) => {
        auth = request.headers.get('authorization')
        return HttpResponse.json({ ok: true })
      }),
    )
    await apiClient('/api/who')
    expect(auth).toBeNull()
  })

  it('parses JSON responses', async () => {
    server.use(http.get('/api/json', () => HttpResponse.json({ hello: 'world' })))
    const out = await apiClient<{ hello: string }>('/api/json')
    expect(out).toEqual({ hello: 'world' })
  })

  it('parses text/* responses as raw text', async () => {
    server.use(
      http.get(
        '/api/raw',
        () =>
          new HttpResponse('# Markdown', {
            headers: { 'content-type': 'text/markdown; charset=utf-8' },
          }),
      ),
    )
    const out = await apiClient<string>('/api/raw')
    expect(out).toBe('# Markdown')
  })

  it('parses application/toml responses as text', async () => {
    server.use(
      http.get(
        '/api/config',
        () => new HttpResponse('title = "hi"', { headers: { 'content-type': 'application/toml' } }),
      ),
    )
    const out = await apiClient<string>('/api/config')
    expect(out).toBe('title = "hi"')
  })

  it('returns undefined for 204 No Content', async () => {
    server.use(http.delete('/api/items/1', () => new HttpResponse(null, { status: 204 })))
    const out = await apiClient<void>('/api/items/1', { method: 'DELETE' })
    expect(out).toBeUndefined()
  })

  it('JSON-encodes request bodies and sets Content-Type for POST', async () => {
    let body: unknown = null
    let contentType: string | null = null
    server.use(
      http.post('/api/echo', async ({ request }) => {
        contentType = request.headers.get('content-type')
        body = await request.json()
        return HttpResponse.json({ ok: true })
      }),
    )
    await api.post('/api/echo', { name: 'oxios' })
    expect(contentType).toBe('application/json')
    expect(body).toEqual({ name: 'oxios' })
  })

  it('JSON-encodes bodies for PATCH', async () => {
    let body: unknown = null
    let contentType: string | null = null
    server.use(
      http.patch('/api/echo', async ({ request }) => {
        contentType = request.headers.get('content-type')
        body = await request.json()
        return HttpResponse.json({ ok: true })
      }),
    )
    await api.patch('/api/echo', { v: 1 })
    expect(contentType).toBe('application/json')
    expect(body).toEqual({ v: 1 })
  })

  it('raw PUT sends the body verbatim with text/markdown Content-Type', async () => {
    let body = ''
    let contentType: string | null = null
    server.use(
      http.put('/api/file.md', async ({ request }) => {
        contentType = request.headers.get('content-type')
        body = await request.text()
        return new HttpResponse(null, { status: 204 })
      }),
    )
    await api.put('/api/file.md', '# Hello', true)
    expect(contentType).toBe('text/markdown')
    expect(body).toBe('# Hello')
  })

  it('FormData POST lets the browser set Content-Type (no application/json)', async () => {
    let contentType: string | null = null
    let boundary: string | null = null
    server.use(
      http.post('/api/upload', ({ request }) => {
        contentType = request.headers.get('content-type')
        boundary = contentType?.match(/boundary=(.+)/)?.[1] ?? null
        return HttpResponse.json({ ok: true })
      }),
    )
    const fd = new FormData()
    fd.append('file', new Blob(['x']), 'a.txt')
    await api.upload('/api/upload', fd)
    // The browser sets a multipart Content-Type with a generated boundary.
    expect(contentType).not.toBe('application/json')
    expect(contentType).toMatch(/^multipart\/form-data; boundary=/)
    expect(boundary).toBeTruthy()
  })

  it('throws ApiError carrying status, statusText, and body for non-2xx', async () => {
    server.use(
      http.get(
        '/api/boom',
        () =>
          new HttpResponse('{"error":"nope"}', {
            status: 404,
            statusText: 'Not Found',
            headers: { 'content-type': 'application/json' },
          }),
      ),
    )
    await expect(apiClient('/api/boom')).rejects.toMatchObject({
      status: 404,
      statusText: 'Not Found',
      body: '{"error":"nope"}',
      name: 'ApiError',
    })
  })

  it('calls useAuthStore.logout() on 401 responses', async () => {
    useAuthStore.getState().setToken('will-be-revoked')
    server.use(
      http.get(
        '/api/expired',
        () => new HttpResponse(null, { status: 401, statusText: 'Unauthorized' }),
      ),
    )
    await expect(apiClient('/api/expired')).rejects.toBeInstanceOf(ApiError)
    expect(useAuthStore.getState().token).toBeNull()
    expect(useAuthStore.getState().isAuthenticated).toBe(false)
  })
})
