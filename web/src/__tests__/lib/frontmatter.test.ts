import {
  findFrontmatterRange,
  parseFlowArray,
  parseFrontmatter,
  parseFrontmatterBlock,
} from '@/lib/frontmatter'

describe('findFrontmatterRange', () => {
  it('locates a flat frontmatter block including the closing newline', () => {
    const doc = '---\ntitle: Foo\ntags: [a]\n---\n\n# Body\n'
    const range = findFrontmatterRange(doc)
    expect(range).toEqual({ from: 0, to: 29 })
    // Range covers `---...---\n`; the next char is the blank line before the body.
    expect(doc[29]).toBe('\n')
    expect(doc.slice(range!.to)).toBe('\n# Body\n')
  })

  it('returns null when the doc does not start with ---', () => {
    expect(findFrontmatterRange('# No fm\nbody')).toBeNull()
    expect(findFrontmatterRange('prose\n---\n---\n')).toBeNull()
  })

  it('returns null for an unclosed block', () => {
    expect(findFrontmatterRange('---\ntitle: Foo\n')).toBeNull()
  })

  it('rejects a first line that is not exactly ---', () => {
    expect(findFrontmatterRange('---foo\ntitle: x\n---\n')).toBeNull()
  })

  it('accepts ... as a closing delimiter', () => {
    const doc = '---\nkey: val\n...\nbody'
    expect(findFrontmatterRange(doc)).not.toBeNull()
  })

  it('handles the closing delimiter being the last line (no trailing newline)', () => {
    const doc = '---\nkey: val\n---'
    expect(findFrontmatterRange(doc)).toEqual({ from: 0, to: doc.length })
  })

  it('finds the body start so extractH1 can skip frontmatter', () => {
    const doc = '---\nkey: val\n---\n# Title\n'
    const range = findFrontmatterRange(doc)!
    expect(doc.slice(range.to).startsWith('# Title')).toBe(true)
  })
})

describe('parseFrontmatter', () => {
  it('parses flat scalar entries', () => {
    const entries = parseFrontmatter('title: Foo\nauthor: me\n')!
    expect(entries).toHaveLength(2)
    expect(entries[0]!).toMatchObject({ key: 'title', valueText: 'Foo', kind: 'scalar' })
    expect(entries[1]!).toMatchObject({ key: 'author', valueText: 'me', kind: 'scalar' })
  })

  it('parses inline flow arrays as kind array', () => {
    const entries = parseFrontmatter('tags: [a, b, c]\n')!
    expect(entries[0]!).toMatchObject({ key: 'tags', kind: 'array' })
  })

  it('preserves a nested oxios block as kind nested with raw text', () => {
    const yaml = 'oxios:\n  author: agent\n  source: Hook\n'
    const entries = parseFrontmatter(yaml)!
    expect(entries).toHaveLength(1)
    const e = entries[0]!
    expect(e.key).toBe('oxios')
    expect(e.kind).toBe('nested')
    expect(e.raw).toContain('author: agent')
    expect(e.raw).toContain('source: Hook')
  })

  it('skips blank and comment lines', () => {
    const entries = parseFrontmatter('# a comment\n\ntitle: Foo\n')!
    expect(entries).toHaveLength(1)
    expect(entries[0]!.key).toBe('title')
  })

  it('returns null for an indented line with no owning key', () => {
    expect(parseFrontmatter('  orphan: line\n')).toBeNull()
  })

  it('returns null for a non-key line at column 0', () => {
    expect(parseFrontmatter('just prose\n')).toBeNull()
  })

  it('returns null for an empty body', () => {
    expect(parseFrontmatter('')).toBeNull()
  })
})

describe('parseFlowArray', () => {
  it('splits a simple flow array', () => {
    expect(parseFlowArray('[a, b, c]')).toEqual(['a', 'b', 'c'])
  })

  it('strips surrounding quotes from items', () => {
    expect(parseFlowArray('["a b", \'c d\']')).toEqual(['a b', 'c d'])
  })

  it('returns [] for an empty array', () => {
    expect(parseFlowArray('[]')).toEqual([])
  })
})

describe('parseFrontmatterBlock', () => {
  it('parses a full block including delimiters', () => {
    const raw = '---\ntitle: Foo\ntags: [a, b]\n---\n'
    const entries = parseFrontmatterBlock(raw)!
    expect(entries).toHaveLength(2)
    expect(entries[0]!).toMatchObject({ key: 'title', kind: 'scalar' })
    expect(entries[1]!).toMatchObject({ key: 'tags', kind: 'array' })
  })

  it('preserves a nested oxios block from a full block', () => {
    const raw = '---\noxios:\n  author: agent\n  source: Hook\n---\n'
    const entries = parseFrontmatterBlock(raw)!
    const e = entries[0]!
    expect(e).toMatchObject({ key: 'oxios', kind: 'nested' })
    expect(e.raw).toContain('author: agent')
  })
})
