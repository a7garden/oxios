import type { Meta, StoryObj } from '@storybook/tanstack-react'
import { Badge } from '@/components/ui/badge'

const meta: Meta<typeof Badge> = {
  title: 'UI/Badge',
  component: Badge,
  args: {
    children: 'Badge',
  },
  argTypes: {
    variant: {
      control: 'select',
      options: ['default', 'secondary', 'outline', 'destructive'],
    },
  },
}

export default meta
type Story = StoryObj<typeof Badge>

// ── Dimension 1: Variants ────────────────────────────────

export const Default: Story = { args: { variant: 'default' } }
export const Secondary: Story = { args: { variant: 'secondary' } }
export const Outline: Story = { args: { variant: 'outline' } }
export const Destructive: Story = { args: { variant: 'destructive' } }

export const AllVariants: Story = {
  render: () => (
    <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap', alignItems: 'center' }}>
      <Badge variant="default">Default</Badge>
      <Badge variant="secondary">Secondary</Badge>
      <Badge variant="outline">Outline</Badge>
      <Badge variant="destructive">Destructive</Badge>
    </div>
  ),
}

// ── Status badges (custom usage) ─────────────────────────

export const StatusBadges: Story = {
  render: () => (
    <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap' }}>
      <Badge className="bg-success-subtle text-success border-success-subtle-border">Running</Badge>
      <Badge className="bg-warning-subtle text-warning border-warning-subtle-border">Pending</Badge>
      <Badge className="bg-error-subtle text-error border-error-subtle-border">Failed</Badge>
      <Badge className="bg-info-subtle text-info border-info-subtle-border">Info</Badge>
    </div>
  ),
}
