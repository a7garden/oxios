import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Cpu, Download, Power, PowerOff, RefreshCw } from 'lucide-react'
import { useState } from 'react'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { api } from '@/lib/api-client'
import type { Program } from '@/types'

export const Route = createFileRoute('/programs')({ component: ProgramsPage })

function ProgramsPage() {
  const queryClient = useQueryClient()
  const [installName, setInstallName] = useState('')

  const {
    data: programs,
    isLoading,
    isError,
    refetch,
    isFetching,
  } = useQuery({
    queryKey: ['programs'],
    queryFn: async () => {
      const res = await api.get<{ items: Program[] }>('/api/programs')
      return res.items ?? []
    },
    refetchInterval: 30000,
  })

  const toggleMutation = useMutation({
    mutationFn: ({ name, enabled }: { name: string; enabled: boolean }) =>
      api.post(`/api/programs/${name}/${enabled ? 'enable' : 'disable'}`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['programs'] }),
  })

  const installMutation = useMutation({
    mutationFn: (path: string) => api.post('/api/programs', { path }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['programs'] })
      setInstallName('')
    },
  })

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const items = (programs ?? []) as Program[]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Programs</h1>
          <p className="text-muted-foreground">Manage OS-level programs</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => refetch()} disabled={isFetching}>
          <RefreshCw className={`h-4 w-4 mr-1 ${isFetching ? 'animate-spin' : ''}`} /> Refresh
        </Button>
      </div>

      {/* Install */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">Install Program</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex gap-2">
            <Input
              value={installName}
              onChange={(e) => setInstallName(e.target.value)}
              placeholder="Program name..."
              className="max-w-xs"
            />
            <Button
              onClick={() => installMutation.mutate(installName)}
              disabled={!installName.trim() || installMutation.isPending}
              size="sm"
            >
              <Download className="h-4 w-4 mr-1" /> Install
            </Button>
          </div>
        </CardContent>
      </Card>

      {items.length === 0 ? (
        <EmptyState
          icon={<Cpu className="h-10 w-10" />}
          title="No programs"
          description="Install programs to extend Oxios capabilities."
        />
      ) : (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {items.map((program) => (
            <Card key={program.name}>
              <CardHeader className="flex flex-row items-start justify-between pb-2">
                <div>
                  <CardTitle className="text-base flex items-center gap-2">
                    <Cpu className="h-4 w-4" /> {program.name}
                  </CardTitle>
                  {program.version && (
                    <p className="text-xs text-muted-foreground">v{program.version}</p>
                  )}
                </div>
                <Badge variant={program.enabled ? 'success' : 'secondary'}>
                  {program.enabled ? 'Enabled' : 'Disabled'}
                </Badge>
              </CardHeader>
              <CardContent>
                {program.description && (
                  <p className="text-sm text-muted-foreground mb-3">{program.description}</p>
                )}
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() =>
                    toggleMutation.mutate({ name: program.name, enabled: !program.enabled })
                  }
                  disabled={toggleMutation.isPending}
                >
                  {program.enabled ? (
                    <>
                      <PowerOff className="h-3 w-3 mr-1" /> Disable
                    </>
                  ) : (
                    <>
                      <Power className="h-3 w-3 mr-1" /> Enable
                    </>
                  )}
                </Button>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  )
}
