import type { Meta, StoryObj } from '@storybook/tanstack-react'
import { Cpu, Terminal } from 'lucide-react'
import { SectionCard } from '@/components/settings/section-card'
import { Switch } from '@/components/ui/switch'
import { i18nDecorator } from '../../../.storybook/i18n-mock'

/** A couple of lightweight mock rows so the card body looks realistic. */
function MockRow({ label, hint, on }: { label: string; hint: string; on?: boolean }) {
  return (
    <div className="flex items-start justify-between gap-4 py-3 first:pt-0 last:pb-0">
      <div className="min-w-0">
        <p className="text-sm font-medium text-foreground">{label}</p>
        <p className="mt-1 text-xs text-muted-foreground leading-relaxed">{hint}</p>
      </div>
      <Switch defaultChecked={on} className="mt-1" />
    </div>
  )
}

const meta: Meta<typeof SectionCard> = {
  title: 'Settings/SectionCard',
  component: SectionCard,
  decorators: [i18nDecorator],
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component:
          'Unified section container for every settings section. Header carries title, ' +
          'icon, description, a field-count badge, and an optional reset button ' +
          'that appears only when modified. Restart-required info is deferred to ' +
          'the DiffPreview at save time.',
      },
    },
  },
  args: {
    title: 'Execution',
    description: 'How agents run tools on the host.',
    sectionId: 'exec',
    fieldCount: 4,
    modified: false,
  },
  render: (args) => (
    <div className="mx-auto w-full max-w-2xl">
      <SectionCard {...args}>
        <MockRow
          label="Default Mode"
          hint="Structured is safer — shell mode requires explicit opt-in."
          on
        />
        <MockRow
          label="Allow Shell Mode"
          hint="Enable bash -c execution. Dangerous in production."
        />
      </SectionCard>
    </div>
  ),
}

export default meta
type Story = StoryObj<typeof SectionCard>

// ── Default: clean, no changes ─────────────────────────────

export const Default: Story = {}

// ── With icon ──────────────────────────────────────────────

export const WithIcon: Story = {
  args: {
    title: 'Kernel',
    description: 'Core runtime tunables: workspace, concurrency, event bus.',
    sectionId: 'kernel',
    icon: <Cpu className="h-4 w-4" />,
    fieldCount: 6,
  },
}

// ── Modified: ring accent + reset button enabled ───────────

export const Modified: Story = {
  args: {
    modified: true,
    onReset: () => {},
  },
}

// ── No metadata row (minimal header) ───────────────────────

export const Minimal: Story = {
  args: {
    title: 'Session',
    description: undefined,
    fieldCount: undefined,
    icon: <Terminal className="h-4 w-4" />,
  },
  render: (args) => (
    <div className="mx-auto w-full max-w-2xl">
      <SectionCard {...args}>
        <MockRow label="Session Timeout" hint="Idle session expiry in minutes." on />
      </SectionCard>
    </div>
  ),
}

// ── Modified with icon ─────────────────────────────────────

export const ModifiedWithIcon: Story = {
  args: {
    title: 'Kernel',
    description: 'Core runtime tunables.',
    icon: <Cpu className="h-4 w-4" />,
    fieldCount: 6,
    modified: true,
    onReset: () => {},
  },
}
