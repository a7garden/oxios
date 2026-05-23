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

export const Route = createFileRoute('/settings')({ component: SettingsPage })

interface SettingsSection {
  key: string
  label: string
  fields: SettingsField[]
}

interface SettingsField {
  key: string
  label: string
  type: 'text' | 'number' | 'password' | 'checkbox'
  placeholder?: string
}

// Sections mapped to actual backend OxiosConfig fields
const sections: SettingsSection[] = [
  {
    key: 'kernel',
    label: 'Kernel',
    fields: [
      {
        key: 'workspace',
        label: 'Workspace Path',
        type: 'text',
        placeholder: '~/.oxios/workspace',
      },
      { key: 'max_agents', label: 'Max Concurrent Agents', type: 'number', placeholder: '16' },
      {
        key: 'event_bus_capacity',
        label: 'Event Bus Capacity',
        type: 'number',
        placeholder: '256',
      },
    ],
  },
  {
    key: 'engine',
    label: 'Engine',
    fields: [
      { key: 'default_model', label: 'Default Model', type: 'text', placeholder: 'openai/gpt-4o' },
      { key: 'api_key', label: 'API Key Override', type: 'password', placeholder: 'sk-...' },
    ],
  },
  {
    key: 'security',
    label: 'Security',
    fields: [
      { key: 'network_access', label: 'Network Access', type: 'text', placeholder: 'false' },
      {
        key: 'max_execution_time_secs',
        label: 'Max Execution Time (s)',
        type: 'number',
        placeholder: '300',
      },
      { key: 'max_memory_mb', label: 'Max Memory (MB)', type: 'number', placeholder: '512' },
    ],
  },
  {
    key: 'scheduler',
    label: 'Scheduler',
    fields: [
      { key: 'max_concurrent', label: 'Max Concurrent Tasks', type: 'number', placeholder: '10' },
      { key: 'rate_limit_per_minute', label: 'Rate Limit/min', type: 'number', placeholder: '60' },
      {
        key: 'zombie_timeout_secs',
        label: 'Zombie Timeout (s)',
        type: 'number',
        placeholder: '600',
      },
    ],
  },
  {
    key: 'memory',
    label: 'Memory',
    fields: [
      { key: 'enabled', label: 'Enabled', type: 'text', placeholder: 'true' },
      {
        key: 'embedding_model',
        label: 'Embedding Model',
        type: 'text',
        placeholder: 'text-embedding-3-small',
      },
      { key: 'context_window', label: 'Context Window', type: 'number', placeholder: '128000' },
    ],
  },
  {
    key: 'resource_monitor',
    label: 'Monitoring',
    fields: [
      { key: 'interval_secs', label: 'Poll Interval (s)', type: 'number', placeholder: '5' },
      { key: 'history_max', label: 'Max History', type: 'number', placeholder: '100' },
      { key: 'cpu_threshold', label: 'CPU Threshold (%)', type: 'number', placeholder: '80' },
    ],
  },
  {
    key: 'daemon',
    label: 'Daemon',
    fields: [
      { key: 'pid_file', label: 'PID File', type: 'text', placeholder: '~/.oxios/oxios.pid' },
      { key: 'log_dir', label: 'Log Directory', type: 'text', placeholder: '~/.oxios/logs' },
    ],
  },
  {
    key: 'gateway',
    label: 'Gateway',
    fields: [
      { key: 'host', label: 'Host', type: 'text', placeholder: '127.0.0.1' },
      { key: 'port', label: 'Port', type: 'number', placeholder: '3000' },
    ],
  },
]

function SettingsPage() {
  const queryClient = useQueryClient()
  const [formValues, setFormValues] = useState<Record<string, Record<string, string>>>({})
  const [activeTab, setActiveTab] = useState('kernel')

  const {
    data: config,
    isLoading,
    isError,
    refetch,
  } = useQuery({
    queryKey: ['config'],
    queryFn: () => api.get<Record<string, unknown>>('/api/config'),
  })

  const saveMutation = useMutation({
    mutationFn: (updated: Record<string, unknown>) => api.put('/api/config', updated),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['config'] }),
  })

  useEffect(() => {
    if (!config) return
    const values: Record<string, Record<string, string>> = {}
    for (const section of sections) {
      const sectionConfig = config[section.key] as Record<string, unknown> | undefined
      if (!sectionConfig) continue
      values[section.key] = {}
      for (const field of section.fields) {
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
    saveMutation.mutate(updated)
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
                    <label htmlFor={`${section.key}-${field.key}`} className="text-sm font-medium">
                      {field.label}
                    </label>
                    <div className="md:col-span-2">
                      <Input
                        id={`${section.key}-${field.key}`}
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
