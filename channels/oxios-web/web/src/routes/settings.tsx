import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Save, Settings } from 'lucide-react'
import { useEffect, useState } from 'react'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { api } from '@/lib/api-client'
import type { OxiosConfig } from '@/types'

export const Route = createFileRoute('/settings')({ component: SettingsPage })

interface SettingsSection {
  key: string
  label: string
  fields: SettingsField[]
}

interface SettingsField {
  key: string
  label: string
  type: 'text' | 'number' | 'password'
  placeholder?: string
}

const sections: SettingsSection[] = [
  {
    key: 'general',
    label: 'General',
    fields: [
      { key: 'default_model', label: 'Default Model', type: 'text', placeholder: 'gpt-4o' },
      {
        key: 'max_concurrent_agents',
        label: 'Max Concurrent Agents',
        type: 'number',
        placeholder: '10',
      },
      {
        key: 'workspace_path',
        label: 'Workspace Path',
        type: 'text',
        placeholder: '~/.oxios/workspace',
      },
    ],
  },
  {
    key: 'engine',
    label: 'Engine',
    fields: [
      { key: 'provider', label: 'Provider', type: 'text', placeholder: 'openai' },
      { key: 'model', label: 'Model', type: 'text', placeholder: 'gpt-4o' },
      { key: 'api_key', label: 'API Key', type: 'password', placeholder: 'sk-...' },
      {
        key: 'base_url',
        label: 'Base URL',
        type: 'text',
        placeholder: 'https://api.openai.com/v1',
      },
    ],
  },
  {
    key: 'exec_security',
    label: 'Exec & Security',
    fields: [
      { key: 'shell_allowed', label: 'Shell Allowed', type: 'text', placeholder: 'true' },
      { key: 'sandbox_enabled', label: 'Sandbox Enabled', type: 'text', placeholder: 'false' },
      { key: 'audit_enabled', label: 'Audit Enabled', type: 'text', placeholder: 'true' },
    ],
  },
  {
    key: 'agents',
    label: 'Agents',
    fields: [
      {
        key: 'default_timeout_ms',
        label: 'Default Timeout (ms)',
        type: 'number',
        placeholder: '300000',
      },
      { key: 'auto_kill_zombies', label: 'Auto Kill Zombies', type: 'text', placeholder: 'true' },
      { key: 'max_retries', label: 'Max Retries', type: 'number', placeholder: '3' },
    ],
  },
  {
    key: 'integrations',
    label: 'Integrations',
    fields: [
      { key: 'mcp_enabled', label: 'MCP Enabled', type: 'text', placeholder: 'true' },
      { key: 'a2a_enabled', label: 'A2A Enabled', type: 'text', placeholder: 'true' },
      { key: 'browser_enabled', label: 'Browser Enabled', type: 'text', placeholder: 'true' },
    ],
  },
  {
    key: 'memory_context',
    label: 'Memory & Context',
    fields: [
      { key: 'memory_enabled', label: 'Memory Enabled', type: 'text', placeholder: 'true' },
      { key: 'context_window', label: 'Context Window', type: 'number', placeholder: '128000' },
      {
        key: 'embedding_model',
        label: 'Embedding Model',
        type: 'text',
        placeholder: 'text-embedding-3-small',
      },
    ],
  },
  {
    key: 'monitoring',
    label: 'Monitoring',
    fields: [
      { key: 'telemetry_enabled', label: 'Telemetry Enabled', type: 'text', placeholder: 'false' },
      { key: 'log_level', label: 'Log Level', type: 'text', placeholder: 'info' },
      {
        key: 'resource_poll_interval_ms',
        label: 'Resource Poll Interval (ms)',
        type: 'number',
        placeholder: '5000',
      },
    ],
  },
  {
    key: 'advanced',
    label: 'Advanced',
    fields: [
      { key: 'daemon_mode', label: 'Daemon Mode', type: 'text', placeholder: 'true' },
      { key: 'pid_file', label: 'PID File', type: 'text', placeholder: '~/.oxios/oxios.pid' },
      {
        key: 'config_path',
        label: 'Config Path',
        type: 'text',
        placeholder: '~/.oxios/config.toml',
      },
    ],
  },
]

function SettingsPage() {
  const queryClient = useQueryClient()
  const [formValues, setFormValues] = useState<Record<string, Record<string, string>>>({})
  const [activeTab, setActiveTab] = useState('general')

  const { data: config, isLoading, isError, refetch } = useQuery({
    queryKey: ['config'],
    queryFn: () => api.get<OxiosConfig>('/api/config'),
  })

  const saveMutation = useMutation({
    mutationFn: (updated: OxiosConfig) => api.put('/api/config', updated),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['config'] }),
  })

  useEffect(() => {
    if (!config) return
    const values: Record<string, Record<string, string>> = {}
    for (const section of sections) {
      const sectionConfig = config[section.key] as Record<string, unknown> | undefined
      if (!sectionConfig) continue
      values[section.key] = {} as Record<string, string>
      for (const field of section.fields) {
        // biome-ignore lint/style/noNonNullAssertion: guaranteed by assignment on prev line
        values[section.key]![field.key] = String(sectionConfig[field.key] ?? '')
      }
    }
    setFormValues(values)
  }, [config])

  const handleSave = () => {
    const updated: Record<string, unknown> = {}
    for (const section of sections) {
      const sectionValues: Record<string, unknown> = {}
      for (const field of section.fields) {
        const val = formValues[section.key]?.[field.key]
        if (val !== undefined && val !== '') {
          sectionValues[field.key] = field.type === 'number' ? Number(val) : val
        }
      }
      updated[section.key] = sectionValues
    }
    saveMutation.mutate(updated as OxiosConfig)
  }

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Settings</h1>
          <p className="text-muted-foreground">Oxios configuration</p>
        </div>
        <Button onClick={handleSave} disabled={saveMutation.isPending}>
          <Save className="h-4 w-4 mr-1" /> Save
        </Button>
      </div>

      {saveMutation.isSuccess && (
        <div className="rounded-lg bg-emerald-500/15 p-3 text-sm text-emerald-700 dark:text-emerald-400">
          Settings saved successfully.
        </div>
      )}
      {saveMutation.isError && (
        <div className="rounded-lg bg-red-500/15 p-3 text-sm text-red-700 dark:text-red-400">
          Failed to save settings.
        </div>
      )}

      <Tabs>
        <TabsList className="flex-wrap">
          {sections.map((s) => (
            <TabsTrigger
              key={s.key}
              data-state={activeTab === s.key ? 'active' : 'inactive'}
              onClick={() => setActiveTab(s.key)}
            >
              {s.label}
            </TabsTrigger>
          ))}
        </TabsList>

        {sections.map((section) => (
          <TabsContent key={section.key} value={section.key}>
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Settings className="h-4 w-4" /> {section.label}
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                {section.fields.map((field) => (
                  <div key={field.key} className="grid gap-2 md:grid-cols-3 items-center">
                    <label htmlFor={field.key} className="text-sm font-medium">
                      {field.label}
                    </label>
                    <div className="md:col-span-2">
                      <Input
                        id={field.key}
                        type={
                          field.type === 'password'
                            ? 'password'
                            : field.type === 'number'
                              ? 'number'
                              : 'text'
                        }
                        value={formValues[section.key]?.[field.key] ?? ''}
                        onChange={(e) =>
                          setFormValues((prev) => ({
                            ...prev,
                            [section.key]: { ...prev[section.key], [field.key]: e.target.value },
                          }))
                        }
                        placeholder={field.placeholder}
                      />
                    </div>
                  </div>
                ))}
              </CardContent>
            </Card>
          </TabsContent>
        ))}
      </Tabs>
    </div>
  )
}
