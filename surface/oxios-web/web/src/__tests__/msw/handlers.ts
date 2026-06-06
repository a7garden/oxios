import { HttpResponse, http } from 'msw'

export const handlers = [
  http.get('/api/budget', () =>
    HttpResponse.json({
      items: [],
      total: 0,
      page: 1,
      limit: 100,
    }),
  ),
  http.get('/api/agent-groups', () =>
    HttpResponse.json({
      items: [],
      total: 0,
      page: 1,
      limit: 100,
    }),
  ),
  http.get('/api/a2a/agents', () => HttpResponse.json({ agents: [] })),
  http.get('/api/a2a/messages', () => HttpResponse.json({ messages: [] })),
  http.get('/api/a2a/topology', () => HttpResponse.json({ nodes: [], edges: [] })),
  http.get('/api/skills', () => HttpResponse.json({ skills: [] })),
  // Memory map (RFC-T1-B) — default empty response. Tests that need
  // a populated map should override this handler in their setup.
  http.get('/api/memory/map', () =>
    HttpResponse.json({
      count: 0,
      epoch: 0,
      entries: [],
    }),
  ),
]
