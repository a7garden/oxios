import type { Meta, StoryObj } from '@storybook/tanstack-react'
import { type RailGroup, SettingsRail } from '@/components/settings/settings-rail'
import { i18nDecorator } from '../../../.storybook/i18n-mock'

/** Realistic rail layout: 3 groups spanning System / Security / Memory. */
const groups: RailGroup[] = [
  {
    id: 'system',
    labelKey: 'settings.groupSystem',
    items: [
      { id: 'kernel', labelKey: 'settings.sectionKernel', badge: 2, status: 'modified' },
      { id: 'exec', labelKey: 'settings.sectionExec', badge: 5, status: 'modified' },
      { id: 'orchestrator', labelKey: 'settings.sectionOrchestrator' },
      { id: 'gateway', labelKey: 'settings.sectionGateway' },
    ],
  },
  {
    id: 'security',
    labelKey: 'settings.groupSecurity',
    items: [
      { id: 'security', labelKey: 'settings.sectionSecurity', badge: 1, status: 'modified' },
      { id: 'audit', labelKey: 'settings.sectionAudit' },
    ],
  },
  {
    id: 'memory',
    labelKey: 'settings.groupMemory',
    items: [{ id: 'memory', labelKey: 'settings.sectionMemory' }],
  },
]

const meta: Meta<typeof SettingsRail> = {
  title: 'Settings/SettingsRail',
  component: SettingsRail,
  decorators: [i18nDecorator],
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component:
          'Left rail navigation: a search box on top followed by grouped items. ' +
          'Items can carry an unsaved-changes badge and a "modified" status dot.',
      },
    },
  },
  args: {
    groups,
    activeId: 'exec',
    onNavigate: () => {},
    searchQuery: '',
    onSearchChange: () => {},
  },
  render: (args) => (
    // The rail is full-height; constrain it so the canvas stays tidy.
    <div className="mx-auto h-[520px] w-full max-w-[280px] rounded-lg border bg-background p-2">
      <SettingsRail {...args} />
    </div>
  ),
}

export default meta
type Story = StoryObj<typeof SettingsRail>

// ── Default: middle item active, a few modified ────────────

export const Default: Story = {}

// ── First item active ──────────────────────────────────────

export const FirstActive: Story = {
  args: { activeId: 'kernel' },
}

// ── Active item is inside the Memory group ─────────────────

export const MemoryActive: Story = {
  args: { activeId: 'memory' },
}

// ── Filtered by search query ───────────────────────────────

export const SearchFiltered: Story = {
  args: { searchQuery: 'sec' },
}

// ── No matches → empty state ───────────────────────────────

export const NoMatches: Story = {
  args: { searchQuery: 'zzz' },
}
