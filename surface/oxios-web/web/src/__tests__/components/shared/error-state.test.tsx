import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { ErrorState } from '@/components/shared/error-state'

// Mock i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

describe('ErrorState', () => {
  it('renders default title from i18n when none provided', () => {
    render(<ErrorState />)

    expect(screen.getByText('common.errorFailedToLoad')).toBeInTheDocument()
  })

  it('renders default message from i18n when none provided', () => {
    render(<ErrorState />)

    expect(screen.getByText('common.errorSomethingWrong')).toBeInTheDocument()
  })

  it('renders custom title', () => {
    render(<ErrorState title="Load failed" />)

    expect(screen.getByText('Load failed')).toBeInTheDocument()
  })

  it('renders custom message', () => {
    render(<ErrorState message="Network error occurred" />)

    expect(screen.getByText('Network error occurred')).toBeInTheDocument()
  })

  it('renders retry button when onRetry is provided', () => {
    render(<ErrorState onRetry={vi.fn()} />)

    const retryBtn = screen.getByRole('button', { name: 'common.retry' })
    expect(retryBtn).toBeInTheDocument()
  })

  it('does not render retry button when onRetry is not provided', () => {
    render(<ErrorState />)

    expect(screen.queryByRole('button')).not.toBeInTheDocument()
  })

  it('calls onRetry when retry button is clicked', async () => {
    const onRetry = vi.fn()
    render(<ErrorState onRetry={onRetry} />)

    const retryBtn = screen.getByRole('button', { name: 'common.retry' })
    await userEvent.click(retryBtn)

    expect(onRetry).toHaveBeenCalledOnce()
  })

  it('has role="alert"', () => {
    render(<ErrorState />)

    expect(screen.getByRole('alert')).toBeInTheDocument()
  })

  it('applies custom className', () => {
    render(<ErrorState className="my-error-class" />)

    const alert = screen.getByRole('alert')
    expect(alert.className).toContain('my-error-class')
  })
})
