/**
 * Pure transformation helpers for array-typed settings fields.
 *
 * These are shared between `routes/settings.tsx` (buildPayload + the
 * load useEffect) and the regression test suite so the round-trip
 * contract — *a value loaded from the server must survive a save
 * without mutation* — is unit-testable without rendering the component.
 *
 * The original bug: the load path did `String(raw ?? '')` which turned
 * `["read","write"]` into `"read,write"`, and buildPayload's `numbers`
 * case did `String([12345,67890]).split("\n")` → `["12345,67890"]` →
 * `[NaN]` → `[]`.  Both are fixed by the `Array.isArray` guards below.
 */

/** Normalise a `tags` field value (string[] | string) to string[]. */
export function tagsToPayload(raw: unknown): string[] {
  if (Array.isArray(raw)) return raw
  if (raw == null || raw === '') return []
  return String(raw)
    .split(/[\s,]+/)
    .filter(Boolean)
}

/** Normalise a `csv` field value (string[] | string) to string[]. */
export function csvToPayload(raw: unknown): string[] {
  if (Array.isArray(raw)) return raw.map((s) => String(s).trim()).filter(Boolean)
  if (raw == null || raw === '') return []
  return String(raw)
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean)
}

/** Normalise a `numbers` field value (number[] | string) to number[]. */
export function numbersToPayload(raw: unknown): number[] {
  if (Array.isArray(raw)) return raw.map(Number).filter((n) => !Number.isNaN(n))
  if (raw == null || raw === '') return []
  return String(raw)
    .split('\n')
    .map((s) => Number(s.trim()))
    .filter((n) => !Number.isNaN(n))
}
