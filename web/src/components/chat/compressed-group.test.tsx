import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { CompressedGroup } from './compressed-group'

describe('CompressedGroup (controlled)', () => {
  it('calls onToggle when clicked and reflects the expanded prop', () => {
    const onToggle = vi.fn()
    render(
      <CompressedGroup
        count={25}
        expanded={false}
        onToggle={onToggle}
        foldedMessages={[]}
        compression={null}
      />,
    )
    fireEvent.click(screen.getByRole('button'))
    expect(onToggle).toHaveBeenCalledTimes(1)
    // Re-render expanded — toggle button is still the first button.
    const { container: container2 } = render(
      <CompressedGroup
        count={25}
        expanded
        onToggle={onToggle}
        foldedMessages={[]}
        compression={null}
      />,
    )
    expect(container2.querySelectorAll('button').length).toBeGreaterThan(0)
  })
})

it('renders tabbed panel when expanded', () => {
  render(
    <CompressedGroup
      count={25}
      expanded
      onToggle={vi.fn()}
      foldedMessages={[{ id: 'm1', role: 'user', content: 'hi', timestamp: '' }]}
      compression={null}
    />,
  )
  // When expanded, the tabbed panel with Summary/History should be present.
  expect(screen.getAllByRole('button')).toHaveLength(3) // toggle + 2 tabs
})
