/**
 * Single source of truth for the companion app's dark monochrome palette.
 * Inherited from the Oxios web surface: pure-black surfaces, off-white type,
 * stepped elevations via subtle borders (no shadows — the platform is the OS).
 */

export const colors = {
  bg: '#000000',
  surface: '#0A0A0A',
  surfaceElev: '#121212',
  border: '#1F1F1F',
  borderStrong: '#2A2A2A',
  text: '#F5F5F5',
  textMuted: '#9A9A9A',
  textDim: '#6A6A6A',
  textInverse: '#000000',
  accent: '#F5F5F5',
  accentBg: '#F5F5F5',
  accentText: '#000000',
  danger: '#E5484D',
  warn: '#E5C07B',
  success: '#7FB069',
  overlay: 'rgba(0,0,0,0.7)'
} as const;

export const space = {
  xs: 4,
  sm: 8,
  md: 12,
  lg: 16,
  xl: 24,
  xxl: 32
} as const;

export const radius = {
  sm: 6,
  md: 10,
  lg: 16,
  pill: 999
} as const;

export const font = {
  mono: 'Menlo',
  sans: 'System'
} as const;

export const text = {
  display: { fontSize: 28, fontWeight: '700' as const, color: colors.text },
  title: { fontSize: 20, fontWeight: '600' as const, color: colors.text },
  body: { fontSize: 15, color: colors.text },
  bodyMuted: { fontSize: 15, color: colors.textMuted },
  caption: { fontSize: 13, color: colors.textMuted },
  micro: { fontSize: 11, color: colors.textDim, letterSpacing: 0.5 }
};
