import { Save, Settings } from 'lucide-react'
import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { useKnowledgeConfig, useKnowledgeConfigUpdate } from '@/hooks/use-knowledge'
import type { KnowledgeConfig } from '@/types/knowledge'

export function KnowledgeSettings() {
  const { data: config, isLoading } = useKnowledgeConfig()
  const updateConfig = useKnowledgeConfigUpdate()
  const [form, setForm] = useState<Partial<KnowledgeConfig>>({})

  useEffect(() => {
    if (config) setForm(config)
  }, [config])

  if (isLoading) return <div className="p-6 text-muted-foreground">Loading settings...</div>

  const handleSave = async () => {
    await updateConfig.mutateAsync(form)
  }

  const update = (key: keyof KnowledgeConfig, value: unknown) => {
    setForm((prev) => ({ ...prev, [key]: value }))
  }

  return (
    <div className="p-4 sm:p-6 space-y-6 max-w-2xl">
      <h2 className="text-lg font-semibold flex items-center gap-2">
        <Settings className="h-5 w-5" />
        Knowledge Settings
      </h2>

      <Card>
        <CardHeader>
          <CardTitle className="text-sm">General</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-1 sm:grid-cols-3 items-center gap-2 sm:gap-4">
            <label className="text-sm text-muted-foreground">Language</label>
            <Input
              value={form.language ?? ''}
              onChange={(e) => update('language', e.target.value)}
              className="sm:col-span-2"
            />
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-3 items-center gap-2 sm:gap-4">
            <label className="text-sm text-muted-foreground">Timezone</label>
            <Input
              value={form.timezone ?? ''}
              onChange={(e) => update('timezone', e.target.value)}
              className="sm:col-span-2"
            />
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-3 items-center gap-2 sm:gap-4">
            <label className="text-sm text-muted-foreground">Mode</label>
            <Input
              value={form.mode ?? ''}
              onChange={(e) => update('mode', e.target.value)}
              className="sm:col-span-2"
              placeholder="chat, full, tasks, notes, journal"
            />
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-3 items-center gap-2 sm:gap-4">
            <label className="text-sm text-muted-foreground">Pomodoro (min)</label>
            <Input
              type="number"
              value={form.pomodoro_duration_in_minutes ?? 25}
              onChange={(e) => update('pomodoro_duration_in_minutes', Number(e.target.value))}
              className="sm:col-span-2"
            />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-sm">Features</CardTitle>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between">
            <label className="text-sm">Two emojis enabled</label>
            <Switch
              checked={form.two_emojis_enabled ?? false}
              onCheckedChange={(v) => update('two_emojis_enabled', v)}
            />
          </div>
          <div className="flex items-center justify-between">
            <label className="text-sm">Quick habits</label>
            <Switch
              checked={form.quick_habits_enabled ?? false}
              onCheckedChange={(v) => update('quick_habits_enabled', v)}
            />
          </div>
        </CardContent>
      </Card>

      <Button onClick={handleSave} disabled={updateConfig.isPending}>
        <Save className="h-4 w-4 mr-2" />
        {updateConfig.isPending ? 'Saving...' : 'Save'}
      </Button>
    </div>
  )
}
