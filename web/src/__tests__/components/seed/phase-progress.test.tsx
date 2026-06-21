import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { PhaseProgress } from '@/components/seed/phase-progress'

// Mock i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

describe('PhaseProgress', () => {
  it('renders all 5 phases', () => {
    render(<PhaseProgress phaseReached="interview" />)

    expect(screen.getByText('seeds.interview')).toBeInTheDocument()
    expect(screen.getByText('seeds.seed')).toBeInTheDocument()
    expect(screen.getByText('seeds.execute')).toBeInTheDocument()
    expect(screen.getByText('seeds.evaluate')).toBeInTheDocument()
    expect(screen.getByText('seeds.evolve')).toBeInTheDocument()
  })

  it('highlights interview as current phase', () => {
    render(<PhaseProgress phaseReached="interview" />)

    const currentPill = screen.getByText('seeds.interview').closest('div[class*="rounded-full"]')
    expect(currentPill?.className).toContain('bg-primary')
    expect(currentPill?.className).toContain('text-primary-foreground')
  })

  it('marks interview and seed as complete when execute is reached', () => {
    render(<PhaseProgress phaseReached="execute" />)

    const interviewPill = screen.getByText('seeds.interview').closest('div[class*="rounded-full"]')
    const seedPill = screen.getByText('seeds.seed').closest('div[class*="rounded-full"]')

    // Complete phases use bg-primary/10
    expect(interviewPill?.className).toContain('bg-primary/10')
    expect(seedPill?.className).toContain('bg-primary/10')
  })

  it('shows evolve as muted when interview is current', () => {
    render(<PhaseProgress phaseReached="interview" />)

    const evolvePill = screen.getByText('seeds.evolve').closest('div[class*="rounded-full"]')
    expect(evolvePill?.className).toContain('bg-muted')
    expect(evolvePill?.className).toContain('text-muted-foreground')
  })

  it('marks all phases complete except evolve when evolve is reached', () => {
    render(<PhaseProgress phaseReached="evolve" />)

    const interviewPill = screen.getByText('seeds.interview').closest('div[class*="rounded-full"]')
    const evaluatePill = screen.getByText('seeds.evaluate').closest('div[class*="rounded-full"]')

    // interview through evaluate should be complete
    expect(interviewPill?.className).toContain('bg-primary/10')
    expect(evaluatePill?.className).toContain('bg-primary/10')
  })
})
