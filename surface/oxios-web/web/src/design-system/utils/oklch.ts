// ============================================================================
// Oxios Design System — OKLCH Color Utilities
// ============================================================================
//
// OKLCH perceptually uniform color space utilities.
// L: Lightness (0–1), C: Chroma (0–~0.4), H: Hue (0–360)
//
// Reference: https://oklch.com
// ============================================================================

/** Parse an oklch() CSS string into components */
export function parseOklch(
  value: string,
): { l: number; c: number; h: number; alpha: number } | null {
  const match = value.match(/oklch\(\s*([\d.]+)\s+([\d.]+)\s+([\d.]+)(?:\s*\/\s*([\d.]+%?))?\s*\)/)
  if (!match) return null
  const [, lStr, cStr, hStr, aStr] = match
  let alpha = 1
  if (aStr) {
    alpha = aStr.endsWith('%') ? parseFloat(aStr) / 100 : parseFloat(aStr)
  }
  return {
    l: parseFloat(lStr!),
    c: parseFloat(cStr!),
    h: parseFloat(hStr!),
    alpha,
  }
}

/** Format OKLCH components as a CSS string */
export function oklchToString(l: number, c: number, h: number, alpha?: number): string {
  if (alpha !== undefined && alpha < 1) {
    return `oklch(${l} ${c} ${h} / ${alpha})`
  }
  return `oklch(${l} ${c} ${h})`
}

/**
 * Generate an 11-step OKLCH palette from a base color.
 * Steps: 50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950
 * Lightness is evenly distributed; hue and chroma are constant.
 */
export function generatePalette(
  baseL: number,
  baseC: number,
  baseH: number,
  steps: number[] = [50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950],
): Record<number, string> {
  const delta = 0.4
  const lMin = Math.max(0.05, baseL - delta / 2)
  const lMax = Math.min(0.95, baseL + delta / 2)
  const result: Record<number, string> = {}

  for (const step of steps) {
    // Map step to lightness: 50 → lMax, 950 → lMin
    const t = (step - 50) / (950 - 50) // 0 to 1
    const l = lMax - t * (lMax - lMin)
    result[step] = oklchToString(
      Math.round(l * 1000) / 1000,
      Math.round(baseC * 1000) / 1000,
      Math.round(baseH * 1000) / 1000,
    )
  }

  return result
}

/**
 * Derive dark mode value by inverting lightness.
 * H and C are preserved. Alpha is preserved.
 */
export function invertForDark(value: string): string {
  const parsed = parseOklch(value)
  if (!parsed) return value
  const newL = 1 - parsed.l
  return oklchToString(
    Math.round(newL * 1000) / 1000,
    parsed.c,
    parsed.h,
    parsed.alpha < 1 ? parsed.alpha : undefined,
  )
}

/**
 * Simple sRGB gamut check — returns true if the OKLCH color
 * is likely within sRGB gamut (approximate).
 * For production use, consider the `culori` library.
 */
export function isInSrgbGamut(l: number, c: number, _h: number): boolean {
  // Approximate: high chroma at extreme lightness is usually out of gamut
  if (l > 0.95 || l < 0.05) return c < 0.03
  if (l > 0.85 || l < 0.15) return c < 0.15
  return c < 0.35
}
