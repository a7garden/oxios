import { render } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { SettingsShellGroup, SettingsShellSection } from '@/components/settings/settings-shell'
import { SettingsShell } from '@/components/settings/settings-shell'

// Mock i18next — SettingsShell renders labels via `t()`; identity is enough
// to assert on class strings.
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}))

// jsdom doesn't implement scrollIntoView; the rail calls it on the active
// item. Stub it so the component mounts under test.
Element.prototype.scrollIntoView = () => {}

/**
 * Locks in the responsive breakpoint contract from the design spec (§5).
 *
 * The rail and field-row widths are deliberately multi-tiered so that the
 * layout adapts across phone → tablet → laptop → desktop. These tests
 * guard against accidental collapse back to a single width.
 */

const groups: SettingsShellGroup[] = [
  { id: 'system', labelKey: 'g.system' },
  { id: 'security', labelKey: 'g.security' },
]
const sections: SettingsShellSection[] = [
  { id: 'kernel', labelKey: 's.kernel', groupId: 'system' },
  { id: 'exec', labelKey: 's.exec', groupId: 'system' },
  { id: 'security', labelKey: 's.security', groupId: 'security' },
]

describe('SettingsShell responsive breakpoints (spec §5)', () => {
  it('rail is visible from md and widens across 3 tiers (200/240/280)', () => {
    const { container } = render(
      <SettingsShell
        groups={groups}
        sections={sections}
        activeId="kernel"
        onNavigate={() => {}}
        unsavedBySection={{}}
      >
        <div />
      </SettingsShell>,
    )
    const aside = container.querySelector('aside')
    expect(aside).not.toBeNull()
    const cls = aside!.className
    // Visible from md (not lg) — tablet shows the rail, not just a drawer.
    expect(cls).toContain('md:block')
    expect(cls).not.toContain('lg:block')
    // Three width tiers.
    expect(cls).toContain('w-[200px]')
    expect(cls).toContain('lg:w-[240px]')
    expect(cls).toContain('xl:w-[280px]')
  })

  it('mobile drawer trigger only appears below md', () => {
    const { container } = render(
      <SettingsShell
        groups={groups}
        sections={sections}
        activeId="kernel"
        onNavigate={() => {}}
        unsavedBySection={{}}
      >
        <div />
      </SettingsShell>,
    )
    // The mobile trigger wrapper is `md:hidden`.
    const mobileWrapper = container.querySelector('.md\\:hidden')
    expect(mobileWrapper).not.toBeNull()
  })
})
