import { describe, expect, it } from 'vitest'
import type { KnowledgeTreeNode } from '@/types/knowledge'
import { buildWikilinkIndex, resolveWikilink } from './wikilink-resolve'

// Fixture (paths matter, other fields are filler for the type):
//
//   brain/
//     Rust.md
//     Ownership.md
//   lang/
//     Rust.md            ← same stem as brain/Rust.md (collision)
//   Notes.md
function makeTree(): KnowledgeTreeNode[] {
  const leaf = (path: string): KnowledgeTreeNode => ({
    name: path.split('/').pop()!,
    path,
    is_dir: false,
    ctime: 0,
    display_name: '',
    has_content: true,
    oxios_quality: null,
    children: [],
  })
  const dir = (path: string, children: KnowledgeTreeNode[]): KnowledgeTreeNode => ({
    name: path,
    path,
    is_dir: true,
    ctime: 0,
    display_name: '',
    has_content: false,
    oxios_quality: null,
    children,
  })
  return [
    dir('brain', [leaf('brain/Rust.md'), leaf('brain/Ownership.md')]),
    dir('lang', [leaf('lang/Rust.md')]),
    leaf('Notes.md'),
  ]
}

describe('buildWikilinkIndex', () => {
  it('maps lowercase stems to their full paths and skips directories', () => {
    const idx = buildWikilinkIndex(makeTree())
    expect(idx.get('rust')).toEqual(['brain/Rust.md', 'lang/Rust.md'])
    expect(idx.get('ownership')).toEqual(['brain/Ownership.md'])
    expect(idx.get('notes')).toEqual(['Notes.md'])
    // Directory nodes are not indexed as stems.
    expect(idx.has('brain')).toBe(false)
  })
})

describe('resolveWikilink', () => {
  const idx = buildWikilinkIndex(makeTree())

  it('full path with .md resolves by exact match', () => {
    expect(resolveWikilink('brain/Rust.md', null, idx)).toBe('brain/Rust.md')
    expect(resolveWikilink('brain/Missing.md', null, idx)).toBeNull()
  })

  it('path without extension resolves by appending .md', () => {
    expect(resolveWikilink('brain/Ownership', null, idx)).toBe('brain/Ownership.md')
    expect(resolveWikilink('brain/Missing', null, idx)).toBeNull()
  })

  it('bare stem resolves uniquely when there is one match', () => {
    expect(resolveWikilink('Ownership', null, idx)).toBe('brain/Ownership.md')
    expect(resolveWikilink('Notes', null, idx)).toBe('Notes.md')
  })

  it('bare stem is case-insensitive', () => {
    expect(resolveWikilink('ownership', null, idx)).toBe('brain/Ownership.md')
    expect(resolveWikilink('NOTES', null, idx)).toBe('Notes.md')
  })

  it('ambiguous bare stem resolves to the same-dir match when source disambiguates', () => {
    expect(resolveWikilink('Rust', 'brain/Ownership.md', idx)).toBe('brain/Rust.md')
    expect(resolveWikilink('Rust', 'lang/Other.md', idx)).toBe('lang/Rust.md')
  })

  it('ambiguous bare stem with no source, or source in a third dir, is unresolved', () => {
    expect(resolveWikilink('Rust', null, idx)).toBeNull()
    expect(resolveWikilink('Rust', 'Notes.md', idx)).toBeNull()
  })

  it('empty / whitespace target is unresolved', () => {
    expect(resolveWikilink('', null, idx)).toBeNull()
    expect(resolveWikilink('   ', null, idx)).toBeNull()
  })

  it('unknown stem is unresolved', () => {
    expect(resolveWikilink('Nowhere', null, idx)).toBeNull()
  })
})
