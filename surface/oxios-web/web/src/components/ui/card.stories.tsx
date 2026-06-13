import type { Meta, StoryObj } from '@storybook/tanstack-react'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'

const meta: Meta<typeof Card> = {
  title: 'UI/Card',
  component: Card,
}

export default meta
type Story = StoryObj<typeof Card>

export const Default: Story = {
  render: () => (
    <Card className="w-[350px]">
      <CardHeader>
        <CardTitle>Agent Runtime</CardTitle>
        <CardDescription>Ouroboros protocol active</CardDescription>
      </CardHeader>
      <CardContent>
        <p className="text-sm text-muted-foreground">
          Seed execution in progress. Memory tier: Hot.
        </p>
      </CardContent>
      <CardFooter className="flex justify-end gap-2">
        <Button variant="outline" size="sm">
          View Logs
        </Button>
        <Button size="sm">Details</Button>
      </CardFooter>
    </Card>
  ),
}

export const Minimal: Story = {
  render: () => (
    <Card className="w-[300px]">
      <CardContent className="pt-6">
        <p className="text-sm">Simple card with just content.</p>
      </CardContent>
    </Card>
  ),
}

export const ThemeComparison: Story = {
  render: () => (
    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '2rem' }}>
      <Card className="w-[300px]">
        <CardHeader>
          <CardTitle>Light Card</CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">Light mode content</p>
        </CardContent>
      </Card>
      <div
        className="dark"
        style={{
          padding: '1rem',
          background: 'oklch(0.141 0.005 285.823)',
          borderRadius: '0.5rem',
        }}
      >
        <Card className="w-[300px]">
          <CardHeader>
            <CardTitle>Dark Card</CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-sm text-muted-foreground">Dark mode content</p>
          </CardContent>
        </Card>
      </div>
    </div>
  ),
}
