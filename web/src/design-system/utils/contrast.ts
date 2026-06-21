// ============================================================================
// Oxios Design System — APCA Contrast Utilities
// ============================================================================
//
// APCA (Accessible Perceptual Contrast Algorithm) contrast checking.
// Working threshold: Lc 60 for body text (14px+).
// Full threshold table in operational-notes.md.
//
// ⚠️ APCA is part of WCAG 3.0 Working Draft, not yet a W3C Recommendation.
// For legal compliance (ADA, EN 301 549), also verify WCAG 2.x 4.5:1.
// ============================================================================

/**
 * Calculate APCA contrast between two OKLCH lightness values.
 * Returns a signed value: positive = dark text on light bg,
 * negative = light text on dark bg.
 *
 * Simplified APCA formula (full spec: https://github.com/Myndex/apca-w3)
 */
export function apcaContrast(fgL: number, bgL: number): number {
  // APCA uses a non-linear perceptual mapping
  const sigma = 0.022
  const scale = 1.625

  const lBg = bgL >= sigma ? bgL : bgL + (sigma - bgL) ** 2 / (4 * sigma)
  const lFg = fgL >= sigma ? fgL : fgL + (sigma - fgL) ** 2 / (4 * sigma)

  const delta = lBg - lFg

  let contrast: number
  if (Math.abs(delta) < 0.005) {
    return 0
  }

  if (delta > 0) {
    // Dark text on light background
    contrast = (lBg ** 0.56 - lFg ** 0.56) * scale
  } else {
    // Light text on dark background
    contrast = (lBg ** 0.65 - lFg ** 0.65) * scale
  }

  // Clamp to [-108, 106] (APCA bounds)
  return Math.max(-108, Math.min(106, contrast * 100))
}

/**
 * Check if contrast meets minimum threshold.
 * @param level 'pass' = Lc 45 (large text), 'preferred' = Lc 60 (body text), 'aaa' = Lc 75
 */
export function meetsContrastThreshold(
  fgL: number,
  bgL: number,
  level: 'pass' | 'preferred' | 'aaa' = 'preferred',
): boolean {
  const thresholds = { pass: 45, preferred: 60, aaa: 75 }
  const minContrast = thresholds[level]
  const contrast = Math.abs(apcaContrast(fgL, bgL))
  return contrast >= minContrast
}

/**
 * Suggest a foreground lightness that meets the minimum contrast threshold
 * for a given background lightness.
 */
export function suggestForegroundL(bgL: number, minContrast: number = 60): number {
  // If background is light, suggest dark foreground
  if (bgL > 0.5) {
    // Binary search for darkest acceptable foreground
    let lo = 0,
      hi = bgL - 0.1
    for (let i = 0; i < 20; i++) {
      const mid = (lo + hi) / 2
      if (Math.abs(apcaContrast(mid, bgL)) >= minContrast) {
        hi = mid // Can be lighter
      } else {
        lo = mid // Need darker
      }
    }
    return Math.round(((lo + hi) / 2) * 1000) / 1000
  }
  // If background is dark, suggest light foreground
  let lo = bgL + 0.1,
    hi = 1
  for (let i = 0; i < 20; i++) {
    const mid = (lo + hi) / 2
    if (Math.abs(apcaContrast(mid, bgL)) >= minContrast) {
      lo = mid // Can be darker
    } else {
      hi = mid // Need lighter
    }
  }
  return Math.round(((lo + hi) / 2) * 1000) / 1000
}
