import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { EvaluationCard } from '@/components/seed/evaluation-card'
import type { EvaluationResult } from '@/types/seed'

// Mock i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

describe('EvaluationCard', () => {
  it('renders nothing when evaluation is undefined', () => {
    const { container } = render(<EvaluationCard evaluation={undefined} />)
    expect(container.innerHTML).toBe('')
  })

  it('renders all three evaluation categories', () => {
    const evaluation: EvaluationResult = {
      mechanical: { passed: true, details: 'All checks passed' },
      semantic: { passed: true, score: 0.95, details: 'Semantic check passed' },
      consensus: { agreed: true, details: 'Consensus reached' },
      score: 0.92,
      all_passed: true,
    }

    render(<EvaluationCard evaluation={evaluation} />)

    expect(screen.getByText('seeds.mechanical')).toBeInTheDocument()
    expect(screen.getByText('seeds.semantic')).toBeInTheDocument()
    expect(screen.getByText('seeds.consensus')).toBeInTheDocument()
  })

  it('renders pass state with green check icon', () => {
    const evaluation: EvaluationResult = {
      mechanical: { passed: true, details: 'OK' },
      semantic: { passed: true, score: 1.0, details: 'OK' },
      score: 1.0,
      all_passed: true,
    }

    render(<EvaluationCard evaluation={evaluation} />)

    // Green check icon has text-green-500
    const matches = screen.getAllByText('seeds.mechanical')
    const el = matches[0]!
    const checkIcons =
      el.closest('div[class*="rounded-lg"]')?.querySelector('.text-green-500') ?? null
    expect(checkIcons).toBeInTheDocument()
  })

  it('renders fail state with red X icon', () => {
    const evaluation: EvaluationResult = {
      mechanical: { passed: false, details: 'Failed' },
      semantic: { passed: true, score: 0.5, details: 'Partial' },
      score: 0.5,
      all_passed: false,
    }

    render(<EvaluationCard evaluation={evaluation} />)

    const failRow = screen.getByText('seeds.mechanical').closest('div[class*="rounded-lg"]')
    const xIcon = failRow?.querySelector('.text-error')
    expect(xIcon).toBeInTheDocument()
  })

  it('renders score bar and badge', () => {
    const evaluation: EvaluationResult = {
      mechanical: { passed: true, details: 'OK' },
      semantic: { passed: true, score: 0.9, details: 'OK' },
      score: 0.75,
      all_passed: true,
    }

    render(<EvaluationCard evaluation={evaluation} />)

    expect(screen.getByText('seeds.score:')).toBeInTheDocument()
    // Badge shows score
    expect(screen.getByText('0.75 / 1.0')).toBeInTheDocument()
  })

  it('renders score of 0 correctly', () => {
    const evaluation: EvaluationResult = {
      mechanical: { passed: false, details: 'Fail' },
      semantic: { passed: false, score: 0, details: 'Fail' },
      score: 0,
      all_passed: false,
    }

    render(<EvaluationCard evaluation={evaluation} />)

    expect(screen.getByText('0.00 / 1.0')).toBeInTheDocument()
  })

  it('renders detail text when provided', () => {
    const evaluation: EvaluationResult = {
      mechanical: { passed: true, details: 'All constraints met' },
      semantic: { passed: true, score: 0.9, details: 'High similarity' },
      consensus: { agreed: true, details: 'Unanimous' },
      score: 0.9,
      all_passed: true,
    }

    render(<EvaluationCard evaluation={evaluation} />)

    expect(screen.getByText('All constraints met')).toBeInTheDocument()
    expect(screen.getByText('High similarity')).toBeInTheDocument()
    expect(screen.getByText('Unanimous')).toBeInTheDocument()
  })
})
