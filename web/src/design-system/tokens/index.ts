// ============================================================================
// Oxios Design System — Token Architecture
// ============================================================================
//
// Three-tier token hierarchy following DTCG standard:
//   Tier 1: Primitive (raw OKLCH values, 11-step palettes)
//   Tier 2: Semantic   (purpose-driven aliases — components consume these)
//   Tier 3: Component  (component-specific compositions)
//
// Rule: Components NEVER consume primitive tokens directly.
//       Always go through semantic → component tokens.
// ============================================================================

// ── Tier 1: Primitive Color Palettes ────────────────────────────────────────
// 11-step OKLCH palettes per hue, evenly distributed lightness.
// These are the raw building blocks — do NOT use in components.

export const primitiveColors = {
  zinc: {
    50: 'oklch(0.985 0 0)',
    100: 'oklch(0.967 0.001 286.375)',
    200: 'oklch(0.92 0.004 286.32)',
    300: 'oklch(0.705 0.015 286.067)',
    400: 'oklch(0.552 0.016 285.938)',
    500: 'oklch(0.405 0.012 286)',
    600: 'oklch(0.274 0.006 286.033)',
    700: 'oklch(0.21 0.006 285.885)',
    800: 'oklch(0.178 0.008 285.89)',
    900: 'oklch(0.15 0.006 286)',
    950: 'oklch(0.141 0.005 285.823)',
  },
  red: {
    50: 'oklch(0.97 0.014 27)',
    100: 'oklch(0.91 0.05 27)',
    200: 'oklch(0.81 0.10 27)',
    300: 'oklch(0.704 0.15 27)',
    400: 'oklch(0.645 0.20 27)',
    500: 'oklch(0.577 0.245 27.325)',
    600: 'oklch(0.48 0.22 27)',
    700: 'oklch(0.39 0.18 27)',
    800: 'oklch(0.30 0.13 27)',
    900: 'oklch(0.23 0.09 27)',
    950: 'oklch(0.18 0.06 27)',
  },
  emerald: {
    50: 'oklch(0.97 0.014 163)',
    100: 'oklch(0.91 0.05 163)',
    200: 'oklch(0.81 0.10 163)',
    300: 'oklch(0.704 0.13 163)',
    400: 'oklch(0.65 0.16 163)',
    500: 'oklch(0.596 0.145 163)',
    600: 'oklch(0.50 0.13 163)',
    700: 'oklch(0.42 0.10 163)',
    800: 'oklch(0.33 0.07 163)',
    900: 'oklch(0.26 0.05 163)',
    950: 'oklch(0.20 0.04 163)',
  },
  amber: {
    50: 'oklch(0.97 0.014 70)',
    100: 'oklch(0.91 0.05 70)',
    200: 'oklch(0.81 0.10 70)',
    300: 'oklch(0.769 0.12 70)',
    400: 'oklch(0.70 0.15 70)',
    500: 'oklch(0.669 0.162 70)',
    600: 'oklch(0.57 0.15 70)',
    700: 'oklch(0.48 0.12 70)',
    800: 'oklch(0.38 0.09 70)',
    900: 'oklch(0.30 0.07 70)',
    950: 'oklch(0.23 0.05 70)',
  },
  blue: {
    50: 'oklch(0.97 0.014 259)',
    100: 'oklch(0.91 0.05 259)',
    200: 'oklch(0.81 0.10 259)',
    300: 'oklch(0.704 0.13 259)',
    400: 'oklch(0.66 0.17 259)',
    500: 'oklch(0.623 0.214 259.815)',
    600: 'oklch(0.52 0.19 259)',
    700: 'oklch(0.43 0.15 259)',
    800: 'oklch(0.34 0.11 259)',
    900: 'oklch(0.27 0.08 259)',
    950: 'oklch(0.20 0.06 259)',
  },
  violet: {
    50: 'oklch(0.97 0.018 303)',
    100: 'oklch(0.91 0.06 303)',
    200: 'oklch(0.81 0.12 303)',
    300: 'oklch(0.72 0.17 303)',
    400: 'oklch(0.65 0.22 303)',
    500: 'oklch(0.627 0.265 303.9)',
    600: 'oklch(0.53 0.23 303)',
    700: 'oklch(0.44 0.18 303)',
    800: 'oklch(0.35 0.14 303)',
    900: 'oklch(0.27 0.10 303)',
    950: 'oklch(0.21 0.07 303)',
  },
} as const

// ── Tier 2: Semantic Color Tokens ───────────────────────────────────────────
// Purpose-driven aliases. Components consume these.
// These map to CSS custom properties in index.css.

export const semanticColors = {
  // Surface
  background: 'var(--background)',
  foreground: 'var(--foreground)',
  card: 'var(--card)',
  cardForeground: 'var(--card-foreground)',
  popover: 'var(--popover)',
  popoverForeground: 'var(--popover-foreground)',

  // Interactive
  primary: 'var(--primary)',
  primaryForeground: 'var(--primary-foreground)',
  secondary: 'var(--secondary)',
  secondaryForeground: 'var(--secondary-foreground)',
  muted: 'var(--muted)',
  mutedForeground: 'var(--muted-foreground)',
  accent: 'var(--accent)',
  accentForeground: 'var(--accent-foreground)',
  destructive: 'var(--destructive)',
  destructiveForeground: 'var(--destructive-foreground)',

  // Border & Input
  border: 'var(--border)',
  input: 'var(--input)',
  ring: 'var(--ring)',

  // Status
  success: 'var(--success)',
  successSubtle: 'var(--success-subtle)',
  successMuted: 'var(--success-muted)',
  warning: 'var(--warning)',
  warningSubtle: 'var(--warning-subtle)',
  warningMuted: 'var(--warning-muted)',
  error: 'var(--error)',
  errorSubtle: 'var(--error-subtle)',
  errorMuted: 'var(--error-muted)',
  info: 'var(--info)',
  infoSubtle: 'var(--info-subtle)',
  infoMuted: 'var(--info-muted)',

  // Sidebar
  sidebar: 'var(--sidebar)',
  sidebarForeground: 'var(--sidebar-foreground)',
  sidebarPrimary: 'var(--sidebar-primary)',
  sidebarPrimaryForeground: 'var(--sidebar-primary-foreground)',
  sidebarAccent: 'var(--sidebar-accent)',
  sidebarAccentForeground: 'var(--sidebar-accent-foreground)',
  sidebarBorder: 'var(--sidebar-border)',
  sidebarRing: 'var(--sidebar-ring)',
} as const

// ── Typography Tokens ───────────────────────────────────────────────────────

export const typography = {
  fontFamily: {
    sans: "'Geist', system-ui, -apple-system, sans-serif",
    mono: "'Geist Mono', ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, monospace",
  },
  fontSize: {
    '2xs': '10px', // Micro labels only (badges, metadata)
    xs: '12px', // Small text, helper hints
    sm: '14px', // Secondary body text
    base: '16px', // Primary body text
    lg: '18px', // Card titles
    xl: '20px', // Section headings
    '2xl': '24px', // Page titles
    '3xl': '30px', // Hero headings [GENERATED]
    '4xl': '36px', // Display [GENERATED]
  },
  fontWeight: {
    normal: 400,
    medium: 500,
    semibold: 600,
    bold: 700,
  },
  lineHeight: {
    none: 1,
    tight: 1.25,
    snug: 1.375,
    normal: 1.5,
    relaxed: 1.625,
  },
  letterSpacing: {
    tight: '-0.025em',
    normal: '0',
    wide: '0.025em',
    wider: '0.05em',
  },
} as const

// ── Spacing Scale (base-4) ──────────────────────────────────────────────────

export const spacing = {
  0: '0px',
  0.5: '2px',
  1: '4px',
  1.5: '6px',
  2: '8px',
  2.5: '10px',
  3: '12px',
  4: '16px',
  5: '20px',
  6: '24px',
  8: '32px',
  10: '40px',
  12: '48px',
  16: '64px',
  20: '80px',
  24: '96px',
} as const

// ── Border Radius Scale ─────────────────────────────────────────────────────

export const radius = {
  sm: 'calc(var(--radius) * 0.6)', // 6px
  md: 'calc(var(--radius) * 0.8)', // 8px
  lg: 'var(--radius)', // 10px
  xl: 'calc(var(--radius) * 1.4)', // 14px
  '2xl': 'calc(var(--radius) * 1.8)', // 18px
  '3xl': 'calc(var(--radius) * 2.2)', // 22px
  full: '9999px',
} as const

// ── Shadow Scale ────────────────────────────────────────────────────────────

export const shadows = {
  none: 'none',
  sm: '0 1px 3px oklch(0 0 0 / 0.06)',
  md: '0 4px 12px oklch(0 0 0 / 0.08)',
  lg: '0 8px 24px oklch(0 0 0 / 0.12)',
  xl: '0 16px 40px oklch(0 0 0 / 0.16)',
} as const

// ── Z-index Scale ───────────────────────────────────────────────────────────

export const zIndex = {
  base: 0,
  dropdown: 10,
  sticky: 40,
  overlay: 50,
  modal: 100,
  popover: 200,
  toast: 300,
} as const

// ── Breakpoints ─────────────────────────────────────────────────────────────

export const breakpoints = {
  sm: '640px',
  md: '768px',
  lg: '1024px',
  xl: '1280px',
  '2xl': '1536px',
} as const
