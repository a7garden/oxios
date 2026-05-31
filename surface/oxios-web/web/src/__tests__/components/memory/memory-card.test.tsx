import { describe, expect, it, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { MemoryCard } from '@/components/memory/memory-card'
import type { MemoryDetail } from '@/types/memory'

// Mock i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

const baseMemory: MemoryDetail = {
  id: 'mem-1',
  key: 'test-key',
  tier: 'hot',
  memory_type: 'fact',
  content: 'This is a test memory content for the card display.',
  summary: null,
  project_ids: [],
  created_at: '2025-01-15T10:30:00Z',
  updated_at: '2025-01-15T10:30:00Z',
  last_accessed: null,
  access_count: 5,
  pinned: false,
  protected: false,
  protection_reason: null,
  tags: [],
  metadata: {},
}

describe('MemoryCard', () => {
  it('renders memory content text', () => {
    render(<MemoryCard memory={baseMemory} onClick={vi.fn()} />)

    expect(screen.getByText('This is a test memory content for the card display.')).toBeInTheDocument()
  })

  it('renders type badge', () => {
    render(<MemoryCard memory={baseMemory} onClick={vi.fn()} />)

    // TypeBadge renders memory.fact for 'fact' type
    expect(screen.getByText('memory.fact')).toBeInTheDocument()
  })

  it('renders tier badge', () => {
    render(<MemoryCard memory={baseMemory} onClick={vi.fn()} />)

    // TierBadge renders memory.hot for 'hot' tier
    expect(screen.getByText('memory.hot')).toBeInTheDocument()
  })

  it('renders created date', () => {
    render(<MemoryCard memory={baseMemory} onClick={vi.fn()} />)

    expect(screen.getByText(new Date('2025-01-15T10:30:00Z').toLocaleDateString())).toBeInTheDocument()
  })

  it('renders access count', () => {
    render(<MemoryCard memory={baseMemory} onClick={vi.fn()} />)

    expect(screen.getByText('memory.appearances: 5')).toBeInTheDocument()
  })

  it('renders pin icon when memory is pinned', () => {
    const pinnedMemory = { ...baseMemory, pinned: true }
    render(<MemoryCard memory={pinnedMemory} onClick={vi.fn()} />)

    // Pin icon is an svg with lucide class — check via aria-hidden or svg
    const pinIcon = document.querySelector('.lucide-pin')
    expect(pinIcon).toBeInTheDocument()
  })

  it('does not render pin icon when not pinned', () => {
    render(<MemoryCard memory={baseMemory} onClick={vi.fn()} />)

    const pinIcon = document.querySelector('.lucide-pin')
    expect(pinIcon).not.toBeInTheDocument()
  })

  it('falls back to key when content is empty', () => {
    const noContent = { ...baseMemory, content: '' }
    render(<MemoryCard memory={noContent} onClick={vi.fn()} />)

    expect(screen.getByText('test-key')).toBeInTheDocument()
  })

  it('truncates long content to 120 characters', () => {
    const longContent = { ...baseMemory, content: 'A'.repeat(200) }
    render(<MemoryCard memory={longContent} onClick={vi.fn()} />)

    // Content is sliced to 120 chars via .slice(0, 120)
    const displayed = screen.getByText('A'.repeat(120))
    expect(displayed).toBeInTheDocument()
  })

  it('calls onClick when clicked', async () => {
    const onClick = vi.fn()
    render(<MemoryCard memory={baseMemory} onClick={onClick} />)

    const card = screen.getByText('This is a test memory content for the card display.').closest('[class*="cursor-pointer"]') as HTMLElement
    card!.click()

    expect(onClick).toHaveBeenCalledOnce()
  })

  it('renders different type badges for different memory types', () => {
    const episodeMemory = { ...baseMemory, memory_type: 'episode' }
    render(<MemoryCard memory={episodeMemory} onClick={vi.fn()} />)

    expect(screen.getByText('memory.episode')).toBeInTheDocument()
  })
})
