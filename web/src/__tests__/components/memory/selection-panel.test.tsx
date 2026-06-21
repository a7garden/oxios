import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { SelectionPanel } from '@/components/memory/selection-panel'
import type { MemoryMapEntry } from '@/types/memory'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      typeof opts?.count === 'number' ? `${key}:${opts.count}` : key,
    i18n: { language: 'en' },
  }),
}))

const baseEntry: MemoryMapEntry = {
  id: 'mem-1',
  tier: 'hot',
  mem_type: 'fact',
  content_preview: 'Sample content for the selected node',
  created_at: '2026-06-04T00:00:00Z',
  access_count: 5,
  coords_2d: [0.1, 0.2],
  top_neighbors: [
    { id: 'mem-2', similarity: 0.91 },
    { id: 'mem-3', similarity: 0.74 },
  ],
}

describe('SelectionPanel', () => {
  it('shows the empty-state prompt when nothing is selected', () => {
    render(<SelectionPanel selected={null} allEntries={[]} onOpenDetail={vi.fn()} />)
    expect(screen.getByTestId('selection-panel-empty')).toBeInTheDocument()
    expect(screen.getByText('memory.mapSelectPrompt')).toBeInTheDocument()
  })

  it('renders the selected node preview and related neighbours', () => {
    const all: MemoryMapEntry[] = [
      baseEntry,
      { ...baseEntry, id: 'mem-2', content_preview: 'related A', tier: 'warm' },
      { ...baseEntry, id: 'mem-3', content_preview: 'related B', tier: 'cold' },
    ]
    render(<SelectionPanel selected={baseEntry} allEntries={all} onOpenDetail={vi.fn()} />)
    expect(screen.getByTestId('selection-panel')).toBeInTheDocument()
    expect(screen.getByText('Sample content for the selected node')).toBeInTheDocument()
    // Related list shows both neighbour previews.
    expect(screen.getByText('related A')).toBeInTheDocument()
    expect(screen.getByText('related B')).toBeInTheDocument()
  })

  it('falls back to neighbour id when the entry is not in the dataset', () => {
    render(<SelectionPanel selected={baseEntry} allEntries={[baseEntry]} onOpenDetail={vi.fn()} />)
    // mem-2 / mem-3 are neighbours but not in `allEntries`; the panel
    // should still render the neighbour label by id.
    expect(screen.getByText('mem-2')).toBeInTheDocument()
    expect(screen.getByText('mem-3')).toBeInTheDocument()
  })

  it('invokes the open-detail handler when the action button is clicked', () => {
    const onOpenDetail = vi.fn()
    render(
      <SelectionPanel selected={baseEntry} allEntries={[baseEntry]} onOpenDetail={onOpenDetail} />,
    )
    screen.getByTestId('selection-open-detail').click()
    expect(onOpenDetail).toHaveBeenCalledWith('mem-1')
  })
})
