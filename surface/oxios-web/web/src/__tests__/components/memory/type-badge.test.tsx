import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import { TypeBadge } from '@/components/memory/type-badge'

// Mock i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

describe('TypeBadge', () => {
  it('renders fact type badge', () => {
    render(<TypeBadge type="fact" />)

    const badge = screen.getByText('memory.fact')
    expect(badge).toBeInTheDocument()
    expect(badge.closest('[class*="text-xs"]')).toBeInTheDocument()
  })

  it('renders episode type badge', () => {
    render(<TypeBadge type="episode" />)

    expect(screen.getByText('memory.episode')).toBeInTheDocument()
  })

  it('renders knowledge type badge', () => {
    render(<TypeBadge type="knowledge" />)

    expect(screen.getByText('memory.knowledge')).toBeInTheDocument()
  })

  it('renders session type badge', () => {
    render(<TypeBadge type="session" />)

    expect(screen.getByText('memory.session')).toBeInTheDocument()
  })

  it('renders with secondary variant', () => {
    render(<TypeBadge type="fact" />)

    const badge = screen.getByText('memory.fact')
    // Badge has variant="secondary"
    expect(badge.closest('[class*="rounded"]')).toBeInTheDocument()
  })
})
