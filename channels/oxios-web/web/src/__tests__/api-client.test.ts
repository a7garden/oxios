import { describe, it, expect } from 'vitest'
import { ApiError } from '@/lib/api-client'

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
