import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { LoadingCards } from '@/components/shared/loading'
import { ArrowLeft, Clock, MessageSquare } from 'lucide-react'
import type { Session, ChatMessage } from '@/types'

export const Route = createFileRoute('/sessions/$sessionId')({
  component: SessionDetailPage,
})

function SessionDetailPage() {
  const { sessionId } = Route.useParams()
  const navigate = useNavigate()

  const { data: session, isLoading: sessionLoading } = useQuery({
    queryKey: ['session', sessionId],
    queryFn: () => api.get<Session>(`/api/sessions/${sessionId}`),
  })

  const { data: messages } = useQuery({
    queryKey: ['session-messages', sessionId],
    queryFn: () => api.get<ChatMessage[]>(`/api/sessions/${sessionId}/messages`),
  })

  if (sessionLoading) return <LoadingCards count={3} />
  if (!session) return <p className="text-muted-foreground">Session not found.</p>

  const details = [
    { label: 'ID', value: session.id },
    { label: 'Agent ID', value: session.agent_id ?? '—' },
    { label: 'Space ID', value: session.space_id ?? '—' },
    { label: 'Messages', value: session.message_count ?? 0 },
    { label: 'Created', value: new Date(session.created_at).toLocaleString() },
    { label: 'Updated', value: session.updated_at ? new Date(session.updated_at).toLocaleString() : '—' },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" onClick={() => navigate({ to: '/sessions' })}>
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <div className="flex-1">
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Clock className="h-6 w-6" /> Session Detail
          </h1>
          <p className="text-muted-foreground font-mono text-xs">{sessionId}</p>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Session Info</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid gap-3 md:grid-cols-2">
            {details.map((d) => (
              <div key={d.label} className="flex items-center justify-between rounded-lg border p-3">
                <span className="text-sm text-muted-foreground">{d.label}</span>
                <span className="text-sm font-medium">{d.value}</span>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <MessageSquare className="h-4 w-4" /> Messages
          </CardTitle>
        </CardHeader>
        <CardContent>
          {messages && messages.length > 0 ? (
            <div className="space-y-3">
              {messages.map((msg, i) => (
                <div key={i} className="flex gap-3">
                  <Badge variant={msg.role === 'user' ? 'default' : 'secondary'} className="shrink-0 h-6">
                    {msg.role}
                  </Badge>
                  <div className="flex-1 rounded-lg bg-muted p-3">
                    <p className="text-sm whitespace-pre-wrap">{msg.content}</p>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">No messages in this session.</p>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
