import { describe, expect, it } from 'vitest'
import { desiredRenamePath, extractH1, isProtectedPath, sanitizeFilenameStem } from './note-rename'

describe('extractH1', () => {
  it('extracts a normal H1', () => {
    expect(extractH1('# My Note\n\nbody')).toBe('My Note')
  })
  it('trims trailing whitespace', () => {
    expect(extractH1('# My Note   \nbody')).toBe('My Note')
  })
  it('returns null when first line is not a heading', () => {
    expect(extractH1('just prose\n# late heading')).toBeNull()
  })
  it('returns null for an empty/whitespace heading', () => {
    expect(extractH1('#    \nbody')).toBeNull()
    expect(extractH1('')).toBeNull()
  })
  it('ignores content after the first line', () => {
    expect(extractH1('# Title\n# Other')).toBe('Title')
  })
})

describe('sanitizeFilenameStem', () => {
  it('strips path separators and reserved chars', () => {
    expect(sanitizeFilenameStem('a/b\\c:d*e?f"g<h>i|j')).toBe('abcdefghij')
  })
  it('collapses internal whitespace runs', () => {
    expect(sanitizeFilenameStem('My   Cool   Note')).toBe('My Cool Note')
  })
  it('trims leading/trailing whitespace', () => {
    expect(sanitizeFilenameStem('  Trimmed  ')).toBe('Trimmed')
  })
  it('returns empty when nothing usable remains', () => {
    expect(sanitizeFilenameStem('   ')).toBe('')
    expect(sanitizeFilenameStem('///')).toBe('')
  })
  it('caps length at 100 chars', () => {
    const long = 'x'.repeat(250)
    expect(sanitizeFilenameStem(long).length).toBe(100)
  })
})

describe('isProtectedPath', () => {
  it.each([
    'Chat.md',
    'Later.md',
    'Done.md',
    'Shop.md',
    'Watch.md',
    'Read.md',
    'journal/2026-07.md',
    'habits/Exercise.md',
    'archive/Old.md',
    'media/image.png',
    'notes.txt',
  ])('protects %s', (p) => {
    expect(isProtectedPath(p)).toBe(true)
  })
  it.each(['brain/Rust.md', 'My Note.md', 'new file.md', 'deep/nested/Note.md'])(
    'allows %s',
    (p) => {
      expect(isProtectedPath(p)).toBe(false)
    },
  )
})

describe('desiredRenamePath', () => {
  it('returns null when path is protected', () => {
    expect(desiredRenamePath('Chat.md', 'Anything')).toBeNull()
    expect(desiredRenamePath('journal/2026.md', 'New Title')).toBeNull()
  })
  it('returns null when H1 is empty', () => {
    expect(desiredRenamePath('brain/Rust.md', null)).toBeNull()
    expect(desiredRenamePath('brain/Rust.md', '')).toBeNull()
  })
  it('returns null when the stem already matches (no-op)', () => {
    expect(desiredRenamePath('brain/Rust.md', 'Rust')).toBeNull()
    expect(desiredRenamePath('My Note.md', 'My Note')).toBeNull()
  })
  it('computes the target path preserving the directory', () => {
    expect(desiredRenamePath('brain/rust.md', 'Rust Lang')).toBe('brain/Rust Lang.md')
    expect(desiredRenamePath('new file.md', 'My Fresh Title')).toBe('My Fresh Title.md')
  })
  it('sanitizes the H1 before computing the target', () => {
    expect(desiredRenamePath('n.md', 'A/B:Cool')).toBe('ABCool.md')
  })
  it('is case-sensitive on the stem so case-fixes still rename', () => {
    expect(desiredRenamePath('rust.md', 'Rust')).toBe('Rust.md')
  })
})
