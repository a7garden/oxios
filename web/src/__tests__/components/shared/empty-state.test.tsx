import { render, screen } from '@testing-library/react'
import { Bot } from 'lucide-react'
import { describe, expect, it } from 'vitest'
import { EmptyState } from '@/components/shared/empty-state'
import { Button } from '@/components/ui/button'

describe('EmptyState', () => {
  it('renders title', () => {
    render(<EmptyState title="No items found" />)

    expect(screen.getByText('No items found')).toBeInTheDocument()
  })

  it('renders description when provided', () => {
    render(<EmptyState title="No agents" description="Start a new agent to see it here." />)

    expect(screen.getByText('Start a new agent to see it here.')).toBeInTheDocument()
  })

  it('does not render description when not provided', () => {
    render(<EmptyState title="No items" />)

    const container = screen.getByRole('status')
    const paragraphs = container.querySelectorAll('p')
    expect(paragraphs).toHaveLength(0)
  })

  it('renders icon when provided', () => {
    render(
      <EmptyState
        icon={<Bot data-testid="empty-icon" className="h-10 w-10" />}
        title="No agents"
      />,
    )

    expect(screen.getByTestId('empty-icon')).toBeInTheDocument()
  })

  it('does not render icon section when not provided', () => {
    render(<EmptyState title="No items" />)

    // No element with aria-hidden="true" should exist
    const container = screen.getByRole('status')
    const hiddenEl = container.querySelector('[aria-hidden="true"]')
    expect(hiddenEl).not.toBeInTheDocument()
  })

  it('renders action when provided', () => {
    render(<EmptyState title="No agents" action={<Button>Create Agent</Button>} />)

    expect(screen.getByRole('button', { name: 'Create Agent' })).toBeInTheDocument()
  })

  it('has role="status"', () => {
    render(<EmptyState title="Empty" />)

    expect(screen.getByRole('status')).toBeInTheDocument()
  })

  it('applies custom className', () => {
    render(<EmptyState title="Empty" className="my-custom-class" />)

    const container = screen.getByRole('status')
    expect(container.className).toContain('my-custom-class')
  })
})
