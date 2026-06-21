import type { Meta, StoryObj } from '@storybook/tanstack-react'
import { SaveDock } from '@/components/settings/save-dock'
import { i18nDecorator } from '../../../.storybook/i18n-mock'

const meta: Meta<typeof SaveDock> = {
  title: 'Settings/SaveDock',
  component: SaveDock,
  decorators: [i18nDecorator],
  parameters: {
    // The dock is `position: fixed`, so it pins to the viewport's
    // bottom-right. Give each story some height so the canvas is
    // scrollable and the dock is clearly visible.
    layout: 'fullscreen',
    docs: {
      description: {
        component:
          'Floating "save dock" pinned to the bottom-right of the settings page. ' +
          'Renders aggregate change state and forwards Review / Discard actions.',
      },
    },
  },
  args: {
    totalChanges: 1,
    restartRequired: 0,
    applyLive: 1,
    isPending: false,
    visible: true,
    onReview: () => {},
    onDiscard: () => {},
  },
  render: (args) => (
    <div className="relative flex h-[420px] w-full items-center justify-center text-sm text-muted-foreground">
      The dock floats over the settings viewport →
      <SaveDock {...args} />
    </div>
  ),
}

export default meta
type Story = StoryObj<typeof SaveDock>

// ── Single change (hot-reloadable) ──────────────────────────

export const Default: Story = {
  args: {
    totalChanges: 1,
    applyLive: 1,
    restartRequired: 0,
  },
}

// ── Mixed changes: some live, some need restart ────────────

export const MixedChanges: Story = {
  args: {
    totalChanges: 5,
    applyLive: 3,
    restartRequired: 2,
  },
}

// ── Every change needs a daemon restart ────────────────────

export const RestartRequired: Story = {
  args: {
    totalChanges: 2,
    applyLive: 0,
    restartRequired: 2,
  },
}

// ── Save mutation in flight (buttons disabled) ─────────────

export const Applying: Story = {
  args: {
    totalChanges: 3,
    applyLive: 2,
    restartRequired: 1,
    isPending: true,
  },
}

// ── Many changes (3-digit count, stress test layout) ───────

export const ManyChanges: Story = {
  args: {
    totalChanges: 128,
    applyLive: 100,
    restartRequired: 28,
  },
}

// ── Hidden state: renders nothing ──────────────────────────

export const Hidden: Story = {
  args: {
    totalChanges: 0,
    applyLive: 0,
    restartRequired: 0,
    visible: true,
  },
  render: (args) => (
    <div className="flex h-[420px] w-full items-center justify-center text-sm text-muted-foreground">
      totalChanges = 0 → the dock renders nothing.
      <SaveDock {...args} />
    </div>
  ),
}
