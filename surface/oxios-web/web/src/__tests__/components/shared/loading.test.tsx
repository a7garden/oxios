import { describe, expect, it } from 'vitest'
import { render } from '@testing-library/react'
import { LoadingCards, LoadingTable } from '@/components/shared/loading'

describe('LoadingCards', () => {
  it('renders default 3 skeleton cards', () => {
    render(<LoadingCards />)

    const cards = document.querySelectorAll('.rounded-xl')
    expect(cards).toHaveLength(3)
  })

  it('renders specified number of skeleton cards', () => {
    render(<LoadingCards count={5} />)

    const cards = document.querySelectorAll('.rounded-xl')
    expect(cards).toHaveLength(5)
  })

  it('renders 1 card when count is 1', () => {
    render(<LoadingCards count={1} />)

    const cards = document.querySelectorAll('.rounded-xl')
    expect(cards).toHaveLength(1)
  })

  it('each card contains skeleton elements', () => {
    render(<LoadingCards count={2} />)

    const cards = document.querySelectorAll('.rounded-xl')
    cards.forEach((card) => {
      const skeletons = card.querySelectorAll('[class*="animate-pulse"]')
      expect(skeletons.length).toBeGreaterThan(0)
    })
  })
})

describe('LoadingTable', () => {
  it('renders default 5 skeleton rows', () => {
    render(<LoadingTable />)

    const rows = document.querySelectorAll('.flex.items-center')
    expect(rows).toHaveLength(5)
  })

  it('renders specified number of skeleton rows', () => {
    render(<LoadingTable rows={3} />)

    const rows = document.querySelectorAll('.flex.items-center')
    expect(rows).toHaveLength(3)
  })

  it('renders a header section', () => {
    render(<LoadingTable />)

    // The header has its own border-b and p-4
    const header = document.querySelector('.border-b.p-4')
    expect(header).toBeInTheDocument()
  })
})