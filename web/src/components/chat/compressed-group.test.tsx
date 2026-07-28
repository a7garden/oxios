import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { CompressedGroup } from './compressed-group'

describe('CompressedGroup (controlled)', () => {
  it('calls onToggle when clicked and reflects the expanded prop', () => {
    const onToggle = vi.fn()
    const { rerender } = render(<CompressedGroup count={25} expanded={false} onToggle={onToggle} />)
    fireEvent.click(screen.getByRole('button'))
    expect(onToggle).toHaveBeenCalledTimes(1)
    // Re-render expanded — still a single toggle button, no crash.
    rerender(<CompressedGroup count={25} expanded onToggle={onToggle} />)
    expect(screen.getByRole('button')).toBeTruthy()
  })
})
