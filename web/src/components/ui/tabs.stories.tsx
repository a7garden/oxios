import type { Meta, StoryObj } from '@storybook/tanstack-react'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'

const meta: Meta<typeof Tabs> = {
  title: 'UI/Tabs',
  component: Tabs,
}

export default meta
type Story = StoryObj<typeof Tabs>

export const Default: Story = {
  render: () => (
    <Tabs defaultValue="overview" className="w-[400px]">
      <TabsList>
        <TabsTrigger value="overview">Overview</TabsTrigger>
        <TabsTrigger value="logs">Logs</TabsTrigger>
        <TabsTrigger value="settings">Settings</TabsTrigger>
      </TabsList>
      <TabsContent value="overview">
        <p className="text-sm text-muted-foreground pt-4">Agent overview content here.</p>
      </TabsContent>
      <TabsContent value="logs">
        <p className="text-sm text-muted-foreground pt-4">Agent log output here.</p>
      </TabsContent>
      <TabsContent value="settings">
        <p className="text-sm text-muted-foreground pt-4">Agent settings panel here.</p>
      </TabsContent>
    </Tabs>
  ),
}

export const ThemeComparison: Story = {
  render: () => (
    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '2rem' }}>
      <Tabs defaultValue="a" className="w-[350px]">
        <TabsList>
          <TabsTrigger value="a">Light A</TabsTrigger>
          <TabsTrigger value="b">Light B</TabsTrigger>
        </TabsList>
        <TabsContent value="a">
          <p className="text-sm pt-4">Light content A</p>
        </TabsContent>
      </Tabs>
      <div
        className="dark"
        style={{
          padding: '1rem',
          background: 'oklch(0.141 0.005 285.823)',
          borderRadius: '0.5rem',
        }}
      >
        <Tabs defaultValue="a" className="w-[350px]">
          <TabsList>
            <TabsTrigger value="a">Dark A</TabsTrigger>
            <TabsTrigger value="b">Dark B</TabsTrigger>
          </TabsList>
          <TabsContent value="a">
            <p className="text-sm pt-4">Dark content A</p>
          </TabsContent>
        </Tabs>
      </div>
    </div>
  ),
}
