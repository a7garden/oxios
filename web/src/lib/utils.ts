import { type ClassValue, clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/**
 * Resolve a CSS custom property (e.g. '--color-info') to an RGB string
 * suitable for SVG attributes in Recharts. CSS variables are NOT
 * natively supported as SVG presentation attributes.
 *
 * Returns 'rgb(0 0 0)' as fallback when the variable cannot be resolved.
 */
export function cssVarToRgb(varName: string): string {
  if (typeof document === 'undefined') return 'rgb(0 0 0)'
  const value = getComputedStyle(document.documentElement).getPropertyValue(varName).trim()
  if (!value) return 'rgb(0 0 0)'
  // Already an rgb() value
  if (value.startsWith('rgb')) return value
  // OKLCH or other — create a temporary element to resolve
  const el = document.createElement('div')
  el.style.color = value
  document.body.appendChild(el)
  const resolved = getComputedStyle(el).color
  document.body.removeChild(el)
  return resolved || 'rgb(0 0 0)'
}

export function formatBytes(bytes: number, decimals = 2): string {
  if (bytes === 0) return '0 Bytes'
  const k = 1024
  const dm = decimals < 0 ? 0 : decimals
  const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB']
  const i = Math.floor(Math.log(bytes) / Math.log(k))
  return `${parseFloat((bytes / k ** i).toFixed(dm))} ${sizes[i]}`
}

export function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`
  return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`
}

/**
 * Format a USD amount with adaptive precision so that micro-costs
 * ($0.0001) and bulk costs ($1234.57) both render with sensible
 * significant digits rather than a fixed 4 decimals.
 *
 * Rules:
 *  - < 0.01 → 4 significant digits (e.g. $0.000123)
 *  - < 1    → 3 significant digits (e.g. $0.123)
 *  - < 1000 → 2 decimals   (e.g. $12.34)
 *  - else   → 2 decimals + thousands separator (e.g. $1,234.57)
 */
export function formatUsd(value: number): string {
  if (!Number.isFinite(value)) return '$0'
  const abs = Math.abs(value)
  if (abs === 0) return '$0'
  if (abs < 0.01) return `$${value.toPrecision(4)}`
  if (abs < 1) return `$${value.toPrecision(3)}`
  return `$${value.toLocaleString(undefined, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function formatRelativeTime(date: string | Date, t?: (...args: any[]) => any): string {
  const d = typeof date === 'string' ? new Date(date) : date
  const diff = Date.now() - d.getTime()
  if (diff < 60000) return t ? t('common.justNow', 'just now') : 'just now'
  if (diff < 3600000) {
    const mins = Math.floor(diff / 60000)
    return t ? t('common.minutesAgo', { count: mins }) : `${mins}m ago`
  }
  if (diff < 86400000) {
    const hrs = Math.floor(diff / 3600000)
    return t ? t('common.hoursAgo', { count: hrs }) : `${hrs}h ago`
  }
  const days = Math.floor(diff / 86400000)
  return t ? t('common.daysAgo', { count: days }) : `${days}d ago`
}

/**
 * Format a date as a relative time string, supporting both past and future.
 * Uses compact notation suitable for dashboard cards.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function formatRelativeDate(date: string | Date, t?: (...args: any[]) => any): string {
  const d = typeof date === 'string' ? new Date(date) : date
  const diffMs = Date.now() - d.getTime()
  const absDiff = Math.abs(diffMs)
  const isFuture = diffMs < 0

  if (absDiff < 60_000) return t ? t('common.justNow', 'just now') : '방금 전'

  const mins = Math.floor(absDiff / 60_000)
  const hours = Math.floor(mins / 60)
  const days = Math.floor(hours / 24)

  if (mins < 60) {
    if (t)
      return isFuture
        ? t('common.minutesLater', { count: mins })
        : t('common.minutesAgo', { count: mins })
    return `${mins}분 ${isFuture ? '후' : '전'}`
  }
  if (hours < 24) {
    if (t)
      return isFuture
        ? t('common.hoursLater', { count: hours })
        : t('common.hoursAgo', { count: hours })
    return `${hours}시간 ${isFuture ? '후' : '전'}`
  }
  if (t)
    return isFuture ? t('common.daysLater', { count: days }) : t('common.daysAgo', { count: days })
  return `${days}일 ${isFuture ? '후' : '전'}`
}
