/**
 * Validate a CORS origin URL.
 *
 * Returns `null` if the value is a valid CORS origin,
 * or an i18n key string describing the error.
 */
export function validateCorsOrigin(value: string): string | null {
  const trimmed = value.trim()
  if (!trimmed) return 'settings.corsErrorsEmpty'
  try {
    const u = new URL(trimmed)
    if (u.protocol !== 'http:' && u.protocol !== 'https:') {
      return 'settings.corsErrorsInvalidProtocol'
    }
    if (u.pathname !== '/' || u.search || u.hash) {
      return 'settings.corsErrorsPathNotAllowed'
    }
    return null
  } catch {
    return 'settings.corsErrorsInvalidUrl'
  }
}
