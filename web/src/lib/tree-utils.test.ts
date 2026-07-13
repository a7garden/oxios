import { describe, expect, it } from 'vitest'
import {
  countFilesRecursive,
  fileTint,
  flattenTree,
  generateUniqueName,
  indentStyle,
  isCircularMove,
} from './tree-utils'
import type { KnowledgeTreeNode } from '@/types/knowledge'

// Minimal tree fixture used across tests. Layout:
//
//   brain/
//     Rust.md
//     rust/
//       Ownership.md
//   journal/
//     2026-07.md
function makeTree(): KnowledgeTreeNode[] {
  return [
    {
      name: 'brain',
      path: 'brain',
      is_dir: true,
      ctime: 0,
      display_name: 'brain',
      has_content: false,
      oxios_quality: null,
      children: [
        {
          name: 'Rust.md',
          path: 'brain/Rust.md',
          is_dir: false,
          ctime: 0,
          display_name: 'Rust',
          has_content: true,
          oxios_quality: null,
          children: [],
        },
        {
          name: 'rust',
          path: 'brain/rust',
          is_dir: true,
          ctime: 0,
          display_name: 'rust',
          has_content: false,
          oxios_quality: null,
          children: [
            {
              name: 'Ownership.md',
              path: 'brain/rust/Ownership.md',
              is_dir: false,
              ctime: 0,
              display_name: 'Ownership',
              has_content: true,
              oxios_quality: 'curated',
              children: [],
            },
          ],
        },
      ],
    },
    {
      name: 'journal',
      path: 'journal',
      is_dir: true,
      ctime: 0,
      display_name: 'journal',
      has_content: false,
      oxios_quality: null,
      children: [
        {
          name: '2026-07.md',
          path: 'journal/2026-07.md',
          is_dir: false,
          ctime: 0,
          display_name: '2026-07',
          has_content: true,
          oxios_quality: null,
          children: [],
        },
      ],
    },
    {
      name: 'New file.md',
      path: 'New file.md',
      is_dir: false,
      ctime: 0,
      display_name: 'New file',
      has_content: true,
      oxios_quality: null,
      children: [],
    },
  ]
}

describe('indentStyle', () => {
  it('uses depth * 16 + 8 for padding-left (unified with workspace tree)', () => {
    expect(indentStyle(0)).toEqual({ paddingLeft: '8px' })
    expect(indentStyle(1)).toEqual({ paddingLeft: '24px' })
    expect(indentStyle(2)).toEqual({ paddingLeft: '40px' })
  })
})

describe('fileTint', () => {
  it('maps md/txt to amber, code to blue, images to pink, falls back to muted', () => {
    expect(fileTint('notes.md')).toBe('text-amber-500')
    expect(fileTint('README.txt')).toBe('text-amber-500')
    expect(fileTint('main.ts')).toBe('text-blue-500')
    expect(fileTint('script.py')).toBe('text-blue-500')
    expect(fileTint('logo.png')).toBe('text-pink-500')
    expect(fileTint('archive.zip')).toBe('text-muted-foreground')
  })

  it('uses the extension, ignoring the dot files and case', () => {
    expect(fileTint('CHANGELOG.MD')).toBe('text-amber-500')
    expect(fileTint('.hidden.md')).toBe('text-amber-500')
  })

  it('returns the default for files with no extension', () => {
    expect(fileTint('Makefile')).toBe('text-muted-foreground')
  })
})

describe('countFilesRecursive', () => {
  it('returns 0 for a file node', () => {
    const file = makeTree()[2]!
    expect(countFilesRecursive(file)).toBe(0)
  })

  it('counts only files inside a leaf folder, not the folder itself', () => {
    const leaf = makeTree()[0]!.children![1]! // brain/rust
    expect(countFilesRecursive(leaf)).toBe(1) // Ownership.md
  })

  it('recurses into nested folders to count every descendant file', () => {
    const brain = makeTree()[0]! // brain/
    expect(countFilesRecursive(brain)).toBe(2) // Rust.md + brain/rust/Ownership.md
  })
})

describe('flattenTree', () => {
  it('returns every node (files + directories) in pre-order', () => {
    const flat = flattenTree(makeTree())
    const names = flat.map((n) => n.name)
    expect(names).toContain('brain')
    expect(names).toContain('Rust.md')
    expect(names).toContain('rust')
    expect(names).toContain('Ownership.md')
    expect(names).toContain('journal')
    expect(names).toContain('2026-07.md')
    expect(names).toContain('New file.md')
    expect(flat.length).toBe(7)
  })

  it('places a directory before its descendants', () => {
    const flat = flattenTree(makeTree())
    const brainIdx = flat.findIndex((n) => n.name === 'brain')
    const rustIdx = flat.findIndex((n) => n.name === 'rust')
    const ownershipIdx = flat.findIndex((n) => n.name === 'Ownership.md')
    expect(brainIdx).toBeGreaterThanOrEqual(0)
    expect(rustIdx).toBeGreaterThan(brainIdx)
    expect(ownershipIdx).toBeGreaterThan(rustIdx)
  })
  it('returns an empty array for empty input', () => {
    expect(flattenTree<KnowledgeTreeNode>([])).toEqual([])
  })
})

describe('generateUniqueName', () => {
  it('returns the default name when there is no collision', () => {
    expect(generateUniqueName(makeTree(), '', 'Ideas.md')).toBe('Ideas.md')
  })

  it('suffixes " 2" when the default is taken at root level', () => {
    // 'New file.md' is already present in the fixture.
    expect(generateUniqueName(makeTree(), '', 'New file.md')).toBe('New file 2.md')
  })

  it('keeps incrementing the suffix while each candidate is taken', () => {
    const tree: KnowledgeTreeNode[] = [
      {
        name: 'Note.md',
        path: 'Note.md',
        is_dir: false,
        ctime: 0,
        display_name: 'Note',
        has_content: true,
        oxios_quality: null,
        children: [],
      },
    ]
    // Note.md → Note 2.md → Note 3.md
    expect(generateUniqueName(tree, '', 'Note.md')).toBe('Note 2.md')
    expect(
      generateUniqueName([...tree, { ...tree[0], path: 'Note 2.md' }], '', 'Note.md'),
    ).toBe('Note 3.md')
  })

  it('preserves the extension when the default name has one (C5 regression)', () => {
    const taken: KnowledgeTreeNode[] = [
      {
        name: 'Plan.md',
        path: 'Plan.md',
        is_dir: false,
        ctime: 0,
        display_name: 'Plan',
        has_content: true,
        oxios_quality: null,
        children: [],
      },
    ]
    expect(generateUniqueName(taken, '', 'Plan.md')).toBe('Plan 2.md')
    // C5: a stem like "Notes" (no extension) should also suffix cleanly.
    expect(generateUniqueName(taken, '', 'Notes')).toBe('Notes')
  })

  it('uses the basePath prefix for nested targets', () => {
    // No collision exists under brain/ — the default should pass through.
    expect(generateUniqueName(makeTree(), 'brain', 'new.md')).toBe('new.md')
    // But 'Rust.md' IS under brain/, so the next candidate must avoid it.
    expect(generateUniqueName(makeTree(), 'brain', 'Rust.md')).toBe('Rust 2.md')
  })
})

describe('isCircularMove', () => {
  it('blocks moving a folder into itself', () => {
    expect(isCircularMove('brain', 'brain')).toBe(true)
  })

  it('blocks moving a folder into its direct child', () => {
    expect(isCircularMove('brain', 'brain/rust')).toBe(true)
  })

  it('blocks moving a folder into a deeply nested descendant', () => {
    expect(isCircularMove('brain', 'brain/rust/notes/deep')).toBe(true)
  })

  it('allows moving a file into a parent folder', () => {
    // A file like "brain/rust/Ownership.md" being moved to "brain" —
    // the source is a file, not a folder, so there's no cycle.
    expect(isCircularMove('brain/rust/Ownership.md', 'brain')).toBe(false)
  })

  it('allows moving between unrelated folders', () => {
    expect(isCircularMove('brain', 'journal')).toBe(false)
    expect(isCircularMove('brain/rust', 'projects')).toBe(false)
  })

  it('does not false-positive on prefix-similar names', () => {
    // "brain" vs "brain-trust" — the `/` guard prevents this.
    expect(isCircularMove('brain', 'brain-trust')).toBe(false)
  })
})
