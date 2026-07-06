import { describe, expect, it } from 'vitest'
import {
  csvToPayload,
  numbersToPayload,
  tagsToPayload,
} from '@/components/settings/array-transforms'
import { diffConfigs } from '@/hooks/use-config'

/**
 * Regression tests for the array-field round-trip bug.
 *
 * Root cause: the settings load useEffect coerced arrays to strings
 * (`String(["read","write"])` → `"read,write"`), and buildPayload's
 * `numbers` case did `String([12345,67890]).split("\n")` →
 * `["12345,67890"]` → `[NaN]` → `[]`.  Both wiped data on save/reload.
 *
 * These tests lock the contract: a value loaded from the server
 * (always a JSON array for tags/csv/numbers fields) must survive the
 * buildPayload transformation unchanged.
 */
describe('array field round-trip', () => {
  // ── tags ──

  it('tagsToPayload preserves a string[] from the server', () => {
    expect(tagsToPayload(['read', 'write', 'edit'])).toEqual(['read', 'write', 'edit'])
  })

  it('tagsToPayload parses a comma-separated string from the editor', () => {
    expect(tagsToPayload('read, write')).toEqual(['read', 'write'])
  })

  it('tagsToPayload handles empty values', () => {
    expect(tagsToPayload([])).toEqual([])
    expect(tagsToPayload('')).toEqual([])
    expect(tagsToPayload(undefined)).toEqual([])
  })

  // ── csv ──

  it('csvToPayload preserves a string[] from the server', () => {
    expect(csvToPayload(['http://localhost:3000', 'http://localhost:4200'])).toEqual([
      'http://localhost:3000',
      'http://localhost:4200',
    ])
  })

  it('csvToPayload parses a comma-separated string', () => {
    expect(csvToPayload('a, b, c')).toEqual(['a', 'b', 'c'])
  })

  // ── numbers ──

  it('numbersToPayload preserves a number[] from the server', () => {
    expect(numbersToPayload([12345, 67890])).toEqual([12345, 67890])
  })

  it('numbersToPayload parses a newline-separated string from the editor', () => {
    expect(numbersToPayload('5\n10\n15')).toEqual([5, 10, 15])
  })

  it('numbersToPayload handles empty values', () => {
    expect(numbersToPayload([])).toEqual([])
    expect(numbersToPayload('')).toEqual([])
  })

  // ── diff round-trip ──
  //
  // When the user hasn't touched a field, the form value (loaded from
  // server) passes through buildPayload and must produce NO diff.

  it('tags array unchanged produces no diff', () => {
    const server = { security: { allowed_tools: ['read', 'write'] } }
    const payload = { security: { allowed_tools: tagsToPayload(['read', 'write']) } }
    expect(diffConfigs(server, payload)).toEqual([])
  })

  it('numbers array unchanged produces no diff', () => {
    const server = { 'channels.telegram': { allowed_users: [12345, 67890] } }
    const payload = {
      'channels.telegram': { allowed_users: numbersToPayload([12345, 67890]) },
    }
    expect(diffConfigs(server, payload)).toEqual([])
  })
})
