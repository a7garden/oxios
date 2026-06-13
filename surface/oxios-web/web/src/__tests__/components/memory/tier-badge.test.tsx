import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { TierBadge } from '@/components/memory/tier-badge'

// Mock i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

describe('TierBadge', () => {
  it('renders hot tier badge', () => {
    render(<TierBadge tier="hot" />)

    const badge = screen.getByText('memory.hot')
    expect(badge).toBeInTheDocument()
    expect(badge.closest('[class*="bg-error-subtle"]')).toBeInTheDocument()
  })

  it('renders warm tier badge', () => {
    render(<TierBadge tier="warm" />)

    const badge = screen.getByText('memory.warm')
    expect(badge).toBeInTheDocument()
    expect(badge.closest('[class*="bg-warning-subtle"]')).toBeInTheDocument()
  })

  it('renders cold tier badge', () => {
    render(<TierBadge tier="cold" />)

    const badge = screen.getByText('memory.cold')
    expect(badge).toBeInTheDocument()
    expect(badge.closest('[class*="bg-info-subtle"]')).toBeInTheDocument()
  })

  it('renders unknown tier with fallback text', () => {
    render(<TierBadge tier="unknown" />)

    // The t function returns the key, and the fallback is the tier string
    // Since t('memory.unknown', 'unknown') returns 'memory.unknown' (our mock returns key)
    expect(screen.getByText('memory.unknown')).toBeInTheDocument()
  })
})
