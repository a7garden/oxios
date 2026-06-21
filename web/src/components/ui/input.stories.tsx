import type { Meta, StoryObj } from '@storybook/tanstack-react'
import { Input } from '@/components/ui/input'

const meta: Meta<typeof Input> = {
  title: 'UI/Input',
  component: Input,
  args: {
    placeholder: 'Type something...',
  },
}

export default meta
type Story = StoryObj<typeof Input>

export const Default: Story = {}

export const WithValue: Story = {
  args: { defaultValue: 'agent-runtime-01' },
}

export const Disabled: Story = {
  args: { disabled: true, defaultValue: 'Cannot edit' },
}

export const WithLabel: Story = {
  render: () => (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.375rem', width: '300px' }}>
      <label className="text-sm font-medium">Agent Name</label>
      <Input placeholder="Enter agent name..." />
      <span className="text-xs text-muted-foreground">The name used to identify this agent.</span>
    </div>
  ),
}

export const ThemeComparison: Story = {
  render: () => (
    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '2rem' }}>
      <div style={{ width: '280px' }}>
        <Input placeholder="Light input" />
      </div>
      <div
        className="dark"
        style={{
          width: '280px',
          padding: '1rem',
          background: 'oklch(0.141 0.005 285.823)',
          borderRadius: '0.5rem',
        }}
      >
        <Input placeholder="Dark input" />
      </div>
    </div>
  ),
}
