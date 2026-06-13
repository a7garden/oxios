import type { Meta, StoryObj } from '@storybook/tanstack-react'
import { ArrowRight, Loader2, Mail, Plus } from 'lucide-react'
import { Button } from '@/components/ui/button'

const meta: Meta<typeof Button> = {
  title: 'UI/Button',
  component: Button,
  args: {
    children: 'Button',
  },
  argTypes: {
    variant: {
      control: 'select',
      options: ['default', 'secondary', 'outline', 'ghost', 'destructive', 'link'],
    },
    size: {
      control: 'select',
      options: ['default', 'sm', 'lg', 'icon'],
    },
  },
}

export default meta
type Story = StoryObj<typeof Button>

// ── Dimension 1: Variants ────────────────────────────────

export const Default: Story = { args: { variant: 'default' } }
export const Secondary: Story = { args: { variant: 'secondary' } }
export const Outline: Story = { args: { variant: 'outline' } }
export const Ghost: Story = { args: { variant: 'ghost' } }
export const Destructive: Story = { args: { variant: 'destructive' } }
export const Link: Story = { args: { variant: 'link' } }

export const AllVariants: Story = {
  render: () => (
    <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap', alignItems: 'center' }}>
      <Button variant="default">Default</Button>
      <Button variant="secondary">Secondary</Button>
      <Button variant="outline">Outline</Button>
      <Button variant="ghost">Ghost</Button>
      <Button variant="destructive">Destructive</Button>
      <Button variant="link">Link</Button>
    </div>
  ),
}

// ── Dimension 2: Sizes ───────────────────────────────────

export const Small: Story = { args: { size: 'sm', children: 'Small' } }
export const DefaultSize: Story = { args: { size: 'default', children: 'Default' } }
export const Large: Story = { args: { size: 'lg', children: 'Large' } }
export const IconOnly: Story = {
  args: { size: 'icon', children: <Mail className="h-4 w-4" /> },
}

export const AllSizes: Story = {
  render: () => (
    <div style={{ display: 'flex', gap: '0.75rem', alignItems: 'center' }}>
      <Button size="sm">Small</Button>
      <Button size="default">Default</Button>
      <Button size="lg">Large</Button>
      <Button size="icon">
        <Mail className="h-4 w-4" />
      </Button>
    </div>
  ),
}

// ── Dimension 3: States ──────────────────────────────────

export const Disabled: Story = {
  args: { disabled: true, children: 'Disabled' },
}

// ── Dimension 7: Slot / Children ─────────────────────────

export const WithLeadingIcon: Story = {
  args: {
    children: (
      <>
        <Plus className="h-4 w-4" /> Add Item
      </>
    ),
  },
}

export const WithTrailingIcon: Story = {
  args: {
    children: (
      <>
        Next <ArrowRight className="h-4 w-4" />
      </>
    ),
  },
}

export const LoadingState: Story = {
  args: {
    children: (
      <>
        <Loader2 className="h-4 w-4 animate-spin" /> Loading
      </>
    ),
    disabled: true,
  },
}

// ── Dimension 4: Theme ───────────────────────────────────

export const ThemeComparison: Story = {
  render: () => (
    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '2rem' }}>
      <div style={{ padding: '1rem' }}>
        <p style={{ marginBottom: '0.5rem', fontSize: '0.75rem', opacity: 0.6 }}>Light</p>
        <Button variant="default">Light Button</Button>
      </div>
      <div
        className="dark"
        style={{
          padding: '1rem',
          background: 'oklch(0.141 0.005 285.823)',
          borderRadius: '0.5rem',
        }}
      >
        <p
          style={{
            marginBottom: '0.5rem',
            fontSize: '0.75rem',
            color: 'oklch(0.985 0 0)',
            opacity: 0.6,
          }}
        >
          Dark
        </p>
        <Button variant="default">Dark Button</Button>
      </div>
    </div>
  ),
}

// ── Dimension 11: Edge Cases ─────────────────────────────

export const LongText: Story = {
  args: { children: 'This is a very long button label that should handle overflow gracefully' },
}
