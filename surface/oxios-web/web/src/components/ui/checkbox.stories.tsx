import type { Meta, StoryObj } from '@storybook/tanstack-react'
import { Checkbox } from '@/components/ui/checkbox'

const meta: Meta<typeof Checkbox> = {
  title: 'UI/Checkbox',
  component: Checkbox,
}

export default meta
type Story = StoryObj<typeof Checkbox>

export const Default: Story = {}

export const Checked: Story = {
  args: { defaultChecked: true },
}

export const Disabled: Story = {
  args: { disabled: true },
}

export const DisabledChecked: Story = {
  args: { disabled: true, defaultChecked: true },
}

export const WithLabel: Story = {
  render: () => (
    <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
      <Checkbox id="terms" />
      <label htmlFor="terms" className="text-sm">
        Accept terms and conditions
      </label>
    </div>
  ),
}

export const AllStates: Story = {
  render: () => (
    <div style={{ display: 'flex', gap: '1rem', alignItems: 'center' }}>
      <div style={{ textAlign: 'center' }}>
        <Checkbox />
        <p className="text-xs text-muted-foreground mt-1">Unchecked</p>
      </div>
      <div style={{ textAlign: 'center' }}>
        <Checkbox defaultChecked />
        <p className="text-xs text-muted-foreground mt-1">Checked</p>
      </div>
      <div style={{ textAlign: 'center' }}>
        <Checkbox disabled />
        <p className="text-xs text-muted-foreground mt-1">Disabled</p>
      </div>
      <div style={{ textAlign: 'center' }}>
        <Checkbox disabled defaultChecked />
        <p className="text-xs text-muted-foreground mt-1">Disabled+</p>
      </div>
    </div>
  ),
}
