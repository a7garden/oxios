import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { Progress } from '@/components/ui/progress'

describe('GroupProgress - Progress bar patterns', () => {
  it('renders progress bar at 50%', () => {
    render(<Progress value={50} />)

    const progressBar = screen.getByRole('progressbar')
    expect(progressBar).toBeInTheDocument()
    // Radix Progress uses CSS transform on the indicator, not aria-valuenow
    const indicator = progressBar.querySelector('[data-slot="progress-indicator"]')
    expect(indicator).toHaveStyle({ transform: 'translateX(-50%)' })
  })

  it('renders progress bar at 100%', () => {
    render(<Progress value={100} />)

    const progressBar = screen.getByRole('progressbar')
    expect(progressBar).toBeInTheDocument()
    const indicator = progressBar.querySelector('[data-slot="progress-indicator"]')
    expect(indicator).toHaveStyle({ transform: 'translateX(-0%)' })
  })

  it('renders progress bar at 0%', () => {
    render(<Progress value={0} />)

    const progressBar = screen.getByRole('progressbar')
    expect(progressBar).toBeInTheDocument()
    const indicator = progressBar.querySelector('[data-slot="progress-indicator"]')
    expect(indicator).toHaveStyle({ transform: 'translateX(-100%)' })
  })

  it('renders progress with custom class for completed state', () => {
    render(<Progress value={100} className="bg-success" />)

    const progressBar = screen.getByRole('progressbar')
    expect(progressBar).toBeInTheDocument()
  })

  it('renders progress with custom class for running state', () => {
    render(<Progress value={75} className="bg-info" />)

    const progressBar = screen.getByRole('progressbar')
    expect(progressBar).toBeInTheDocument()
  })
})
