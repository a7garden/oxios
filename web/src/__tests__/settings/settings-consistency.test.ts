import { describe, expect, it } from 'vitest'
import { NEW_SECTIONS, SECTION_META, SETTINGS_GROUPS } from '@/components/settings/field-defs'

/**
 * Structural invariants for the settings navigation.
 *
 * The settings UI derives its rail/tabs from THREE sources that must
 * stay perfectly aligned:
 *
 *   1. `SETTINGS_GROUPS`  — which groups + section ids appear in the nav
 *   2. `SECTION_META`     — metadata (label/icon/group) for each section
 *   3. `renderActiveSection` (routes/settings.tsx) — the actual renderer
 *
 * The `advanced` orphan bug (8 sections declared in SETTINGS_GROUPS but
 * absent from SECTION_META, silently hidden by a `.filter()`) was caused
 * by the lack of these invariants. These tests lock the contract down.
 */

// ── Source 3: the set of section ids that renderActiveSection can render ──
//
// Kept in sync manually with `routes/settings.tsx::renderActiveSection`.
// If you add a new section, append its id here AND wire its renderer;
// the test below will fail until both halves land together.
const RENDERABLE_SECTION_IDS = new Set<string>([
  // Custom renderers (dedicated components)
  'engine',
  'update',
  'secrets',
  'notifications',
  'host-tools',
  'memory',
  'channels.telegram',
  // Declarative new sections (driven by NEW_SECTIONS)
  ...NEW_SECTIONS.map((s) => s.key),
  // Legacy field-based sections (legacyFieldsBySection)
  'kernel',
  'orchestrator',
  'context',
  'gateway',
  'session',
  'logging',
])

describe('settings navigation consistency', () => {
  it('every SETTINGS_GROUPS sectionKey exists in SECTION_META', () => {
    const metaIds = new Set(SECTION_META.map((m) => m.id))
    const orphans: string[] = []
    for (const group of SETTINGS_GROUPS) {
      for (const key of group.sectionKeys) {
        if (!metaIds.has(key)) orphans.push(`${group.id}.${key}`)
      }
    }
    expect(orphans, `sections without SECTION_META: ${orphans.join(', ')}`).toEqual([])
  })

  it('every SETTINGS_GROUPS sectionKey is renderable', () => {
    const orphans = SETTINGS_GROUPS.flatMap((g) => g.sectionKeys).filter(
      (key) => !RENDERABLE_SECTION_IDS.has(key),
    )
    expect(orphans, `nav items without a renderer: ${orphans.join(', ')}`).toEqual([])
  })

  it('every SECTION_META entry belongs to a SETTINGS_GROUPS group', () => {
    const groupIds = new Set(SETTINGS_GROUPS.map((g) => g.id))
    const orphans = SECTION_META.filter((m) => !groupIds.has(m.groupId)).map((m) => m.id)
    expect(orphans, `meta entries with unknown groupId: ${orphans.join(', ')}`).toEqual([])
  })

  it('every SECTION_META entry is renderable', () => {
    const orphans = SECTION_META.filter((m) => !RENDERABLE_SECTION_IDS.has(m.id)).map((m) => m.id)
    expect(orphans, `meta entries without a renderer: ${orphans.join(', ')}`).toEqual([])
  })

  it('every SECTION_META entry has a labelKey + descriptionKey + iconKey', () => {
    for (const m of SECTION_META) {
      expect(m.labelKey, `${m.id}.labelKey`).toBeTruthy()
      expect(m.descriptionKey, `${m.id}.descriptionKey`).toBeTruthy()
      expect(m.iconKey, `${m.id}.iconKey`).toBeTruthy()
    }
  })

  it('no duplicate SECTION_META ids', () => {
    const ids = SECTION_META.map((m) => m.id)
    expect(new Set(ids).size, `duplicates: ${findDuplicates(ids).join(', ')}`).toBe(ids.length)
  })

  it('SETTINGS_GROUPS ids are unique', () => {
    const ids = SETTINGS_GROUPS.map((g) => g.id)
    expect(new Set(ids).size).toBe(ids.length)
  })
})

describe('hot-reload single source of truth', () => {
  it('NEW_SECTIONS fields must not carry hotReload (backend is sole source)', () => {
    for (const section of NEW_SECTIONS) {
      for (const field of section.fields) {
        expect(
          (field as unknown as Record<string, unknown>).hotReload,
          `${section.key}.${field.key}`,
        ).toBeUndefined()
      }
    }
  })

  it('section definitions must not carry hotReload', () => {
    for (const section of NEW_SECTIONS) {
      expect((section as unknown as Record<string, unknown>).hotReload, section.key).toBeUndefined()
    }
  })
})

function findDuplicates(arr: string[]): string[] {
  const seen = new Set<string>()
  const dupes = new Set<string>()
  for (const x of arr) {
    if (seen.has(x)) dupes.add(x)
    seen.add(x)
  }
  return [...dupes]
}
