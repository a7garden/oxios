// ============================================================================
// Oxios Design System — Public API
// ============================================================================
//
// This barrel export provides type-safe access to all design tokens.
// Components import from here — never from individual token files.
//
// Usage:
//   import { semanticColors, typography, spacing } from '@/design-system'
//   // or
//   import { cn } from '@/design-system'
// ============================================================================

// Re-export the cn() utility for consumers
export { cn } from '../lib/utils'
export {
  breakpoints,
  primitiveColors,
  radius,
  semanticColors,
  shadows,
  spacing,
  typography,
  zIndex,
} from './tokens/index'
