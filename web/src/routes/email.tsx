import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import DOMPurify from 'dompurify'
import {
  ArrowRight,
  CheckCircle2,
  ExternalLink,
  Eye,
  EyeOff,
  History,
  LayoutTemplate,
  Loader2,
  Mail,
  MailCheck,
  MailWarning,
  RefreshCw,
  Send,
  Settings,
  Wrench,
} from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select } from '@/components/ui/select'
import { Separator } from '@/components/ui/separator'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { api } from '@/lib/api-client'
import { cn } from '@/lib/utils'

export const Route = createFileRoute('/email')({
  component: EmailPage,
  validateSearch: (search: Record<string, unknown>) => ({
    tab: (search.tab as string) || undefined,
  }),
})

// ─── Types ─────────────────────────────────────────────────

interface EmailStatus {
  configured: boolean
  email: string | null
  provider: string | null
  template_count: number
  total_sent: number
}

interface SentEmail {
  id: string
  sent_at: string
  subject: string
  to: string
  template_used: string | null
  message_id: string
  html_preview: string
}

interface EmailHistory {
  emails: SentEmail[]
  total: number
  limit: number
}

interface TemplateInfo {
  name: string
  preview: string
  size: number
}

interface TemplatesResponse {
  templates: TemplateInfo[]
}

interface SetupResponse {
  status: string
  message: string
  email: string
}

interface TestResponse {
  status: string
  message: string
  to: string
}

// ─── Hooks ─────────────────────────────────────────────────

function useEmailStatus() {
  return useQuery({
    queryKey: ['email-status'],
    queryFn: () => api.get<EmailStatus>('/api/email/status'),
    refetchInterval: 10_000,
  })
}

function useEmailHistory(limit = 50, enabled = false) {
  return useQuery({
    queryKey: ['email-history', limit],
    queryFn: () => api.get<EmailHistory>(`/api/email/history?limit=${limit}`),
    enabled,
  })
}

function useEmailTemplates(enabled = false) {
  return useQuery({
    queryKey: ['email-templates'],
    queryFn: () => api.get<TemplatesResponse>('/api/email/templates'),
    enabled,
  })
}

// ─── Provider metadata ─────────────────────────────────────

const PROVIDERS = [
  { value: 'resend', label: 'Resend' },
  { value: 'gmail', label: 'Gmail' },
  { value: 'icloud', label: 'iCloud' },
  { value: 'fastmail', label: 'Fastmail' },
  { value: 'custom', label: 'Custom SMTP' },
] as const

function providerDashboardUrl(provider: string | null): string | null {
  switch (provider) {
    case 'resend':
      return 'https://resend.com/emails'
    case 'gmail':
      return 'https://mail.google.com'
    case 'icloud':
      return 'https://www.icloud.com/mail'
    case 'fastmail':
      return 'https://app.fastmail.com'
    default:
      return null
  }
}

function providerDashboardLabel(provider: string | null): string | null {
  switch (provider) {
    case 'resend':
      return 'Resend Dashboard'
    case 'gmail':
      return 'Gmail'
    case 'icloud':
      return 'iCloud Mail'
    case 'fastmail':
      return 'Fastmail'
    default:
      return null
  }
}

// ─── Page ──────────────────────────────────────────────────

function EmailPage() {
  const { t } = useTranslation()
  const { data: status, isLoading, isError, refetch } = useEmailStatus()
  const navigate = useNavigate({ from: '/email' })
  const { tab: tabParam } = Route.useSearch()
  const validTabs = ['overview', 'setup', 'history', 'templates'] as const
  const explicitTab = validTabs.includes(tabParam as (typeof validTabs)[number])
    ? (tabParam as string)
    : undefined
  // Default lands configured users on overview, unconfigured on setup — but an
  // explicit ?tab= is always respected (no forced jump away from user choice).
  const activeTab = explicitTab ?? (status?.configured ? 'overview' : 'setup')
  const setActiveTab = (v: string) =>
    navigate({ search: (prev) => ({ ...prev, tab: v }), replace: true })

  if (isLoading) return <LoadingCards count={3} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('email.title', 'Email')}</h1>
          <p className="text-muted-foreground">
            {t('email.subtitle', 'Configure and manage email sending')}
          </p>
        </div>
        <div className="flex items-center gap-2">
          {status?.configured && (
            <Badge variant="outline" className="gap-1">
              <MailCheck className="h-3 w-3 text-success" />
              {t('email.configured', 'Configured')}
            </Badge>
          )}
          {!status?.configured && (
            <Badge variant="outline" className="gap-1 text-muted-foreground">
              <MailWarning className="h-3 w-3" />
              {t('email.notConfigured', 'Not configured')}
            </Badge>
          )}
          <Button variant="ghost" size="icon" onClick={() => refetch()}>
            <RefreshCw className="h-4 w-4" />
          </Button>
        </div>
      </div>

      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <TabsList>
          <TabsTrigger value="overview">
            <Mail className="h-4 w-4 mr-2" />
            {t('email.overviewTab', 'Overview')}
          </TabsTrigger>
          <TabsTrigger value="setup" data-testid="email-setup-tab">
            <Settings className="h-4 w-4 mr-2" />
            {t('email.setupTab', 'Setup')}
          </TabsTrigger>
          <TabsTrigger value="history">
            <History className="h-4 w-4 mr-2" />
            {t('email.historyTab', 'Sent History')}
          </TabsTrigger>
          <TabsTrigger value="templates">
            <LayoutTemplate className="h-4 w-4 mr-2" />
            {t('email.templatesTab', 'Templates')}
          </TabsTrigger>
        </TabsList>

        <Separator className="my-4" />

        <TabsContent value="overview">
          <OverviewPanel status={status} onGoSetup={() => setActiveTab('setup')} />
        </TabsContent>
        <TabsContent value="setup">
          <SetupPanel
            status={status}
            onComplete={() => {
              refetch()
              setActiveTab('overview')
            }}
          />
        </TabsContent>
        <TabsContent value="history">
          <HistoryPanel active={activeTab === 'history'} />
        </TabsContent>
        <TabsContent value="templates">
          <TemplatesPanel active={activeTab === 'templates'} />
        </TabsContent>
      </Tabs>
    </div>
  )
}

// ─── Overview Panel ────────────────────────────────────────

function OverviewPanel({ status, onGoSetup }: { status?: EmailStatus; onGoSetup: () => void }) {
  const { t } = useTranslation()

  if (!status) return null

  return (
    <div className="space-y-4">
      <div className="grid gap-4 md:grid-cols-3">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm text-muted-foreground">
              {t('email.statusCard', 'Status')}
            </CardTitle>
          </CardHeader>
          <CardContent>
            {status.configured ? (
              <div className="flex items-center gap-2">
                <CheckCircle2 className="h-5 w-5 text-success" />
                <span className="font-semibold">{t('email.ready', 'Ready')}</span>
              </div>
            ) : (
              <div className="flex items-center gap-2 text-muted-foreground">
                <MailWarning className="h-5 w-5" />
                <span>{t('email.notReady', 'Not configured')}</span>
              </div>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm text-muted-foreground">
              {t('email.senderAddress', 'Sender')}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="font-semibold truncate">{status.email ?? t('email.noAddress', '—')}</p>
            {status.provider && (
              <p className="text-xs text-muted-foreground mt-1 capitalize">{status.provider}</p>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm text-muted-foreground">
              {t('email.activity', 'Activity')}
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-2xl font-bold">{status.total_sent}</p>
            <p className="text-xs text-muted-foreground">
              {t('email.totalSent', 'total sent')}
              {' · '}
              {status.template_count} {t('email.templates', 'templates')}
            </p>
          </CardContent>
        </Card>
      </div>

      {/* CTA when not configured */}
      {!status.configured && (
        <Card className="border-dashed">
          <CardContent className="flex items-center justify-between py-6">
            <div>
              <p className="font-medium">{t('email.getStarted', 'Get started with email')}</p>
              <p className="text-sm text-muted-foreground">
                {t(
                  'email.getStartedDesc',
                  'Connect Resend or another SMTP provider to let agents send emails.',
                )}
              </p>
            </div>
            <Button onClick={onGoSetup}>
              {t('email.setupTab', 'Setup')}
              <ArrowRight className="h-4 w-4 ml-1" />
            </Button>
          </CardContent>
        </Card>
      )}
    </div>
  )
}

// ─── Setup Panel ───────────────────────────────────────────

function SetupPanel({ status, onComplete }: { status?: EmailStatus; onComplete: () => void }) {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [provider, setProvider] = useState('resend')
  const [myEmail, setMyEmail] = useState(status?.email ?? '')
  const [password, setPassword] = useState('')
  const [showPassword, setShowPassword] = useState(false)
  const [host, setHost] = useState('')
  const [port, setPort] = useState('587')
  const [user, setUser] = useState('')
  const [testResult, setTestResult] = useState<{
    ok: boolean
    message: string
  } | null>(null)

  const isResend = provider === 'resend'
  const isCustom = provider === 'custom'

  const setupMutation = useMutation({
    mutationFn: (body: {
      my_email: string
      provider: string
      password: string
      host?: string
      port?: number
      user?: string
    }) => api.post<SetupResponse>('/api/email/setup', body),
    onSuccess: () => {
      setTestResult({
        ok: true,
        message: t(
          'email.setupSuccess',
          'Email configured successfully. Restart oxios to activate.',
        ),
      })
      queryClient.invalidateQueries({ queryKey: ['email-status'] })
      onComplete()
    },
    onError: (err: Error) => {
      setTestResult({ ok: false, message: err.message })
    },
  })

  const testMutation = useMutation({
    mutationFn: () => api.post<TestResponse>('/api/email/test'),
    onSuccess: (data) => {
      setTestResult({ ok: true, message: data.message })
    },
    onError: (err: Error) => {
      setTestResult({ ok: false, message: err.message })
    },
  })

  const handleSetup = (e: React.FormEvent) => {
    e.preventDefault()
    setTestResult(null)
    const body: Record<string, unknown> = {
      my_email: myEmail,
      provider,
      password,
    }
    if (isCustom) {
      body.host = host
      body.port = parseInt(port, 10) || 587
      if (user) body.user = user
    }
    setupMutation.mutate(body as Parameters<typeof setupMutation.mutate>[0])
  }

  const handleTest = () => {
    setTestResult(null)
    testMutation.mutate()
  }

  const isPending = setupMutation.isPending || testMutation.isPending

  // ── Already configured state ────────────────────────────
  if (status?.configured) {
    const dashUrl = providerDashboardUrl(status.provider)
    const dashLabel = providerDashboardLabel(status.provider)

    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <MailCheck className="h-5 w-5 text-success" />
            {t('email.alreadyConfigured', 'Email is configured')}
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <p className="text-sm text-muted-foreground">
            {t('email.configuredWith', 'Sending from')} <strong>{status.email}</strong>
            {status.provider ? ` (${status.provider})` : ''}
          </p>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={handleTest} disabled={isPending}>
              {isPending ? (
                <Loader2 className="h-4 w-4 mr-1 animate-spin" />
              ) : (
                <Send className="h-4 w-4 mr-1" />
              )}
              {t('email.sendTest', 'Send Test')}
            </Button>
            {dashUrl && dashLabel && (
              <Button variant="outline" size="sm" onClick={() => window.open(dashUrl, '_blank')}>
                <ExternalLink className="h-4 w-4 mr-1" />
                {dashLabel}
              </Button>
            )}
          </div>
          {testResult && (
            <div
              className={cn(
                'text-sm p-3 rounded-md border',
                testResult.ok
                  ? 'border-success-subtle text-success'
                  : 'border-destructive/50 text-destructive',
              )}
            >
              {testResult.message}
            </div>
          )}
        </CardContent>
        <CardFooter className="border-t text-xs text-muted-foreground">
          {t('email.reconfigNotice', 'To reconfigure, update config.toml and restart.')}
        </CardFooter>
      </Card>
    )
  }

  // ── Setup form ──────────────────────────────────────────
  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Wrench className="h-5 w-5" />
          {t('email.setupTitle', 'Email Setup')}
        </CardTitle>
        <p className="text-sm text-muted-foreground">
          {t(
            'email.setupDescription',
            'Connect an SMTP provider to enable email sending for agents.',
          )}
        </p>
      </CardHeader>
      <CardContent>
        <form onSubmit={handleSetup} className="space-y-5">
          {/* Provider */}
          <div className="space-y-2">
            <Label>{t('email.providerLabel', 'Provider')}</Label>
            <Select
              value={provider}
              onValueChange={(v) => {
                setProvider(v)
                setTestResult(null)
              }}
              options={PROVIDERS.map((p) => ({
                label: p.label,
                value: p.value,
              }))}
              className="w-full sm:w-64"
            />
          </div>

          {/* Email */}
          <div className="space-y-2">
            <Label htmlFor="email">{t('email.emailLabel', 'Email address')}</Label>
            <Input
              id="email"
              type="email"
              value={myEmail}
              onChange={(e) => setMyEmail(e.target.value)}
              placeholder={isResend ? 'hello@mydomain.com' : 'user@gmail.com'}
              required
              className="max-w-sm"
            />
            {isResend && (
              <p className="text-xs text-muted-foreground">
                {t(
                  'email.resendDomainHint',
                  'Must use a domain verified in your Resend dashboard.',
                )}
              </p>
            )}
          </div>

          {/* Password / API Key */}
          <div className="space-y-2">
            <Label htmlFor="password">
              {isResend
                ? t('email.apiKeyLabel', 'Resend API key')
                : t('email.passwordLabel', 'SMTP password / app password')}
            </Label>
            <div className="relative max-w-sm">
              <Input
                id="password"
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={isResend ? 're_...' : 'Enter password'}
                required
                className="pr-9"
              />
              <button
                type="button"
                className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                onClick={() => setShowPassword(!showPassword)}
                tabIndex={-1}
                aria-label={
                  showPassword
                    ? t('email.hidePassword', 'Hide password')
                    : t('email.showPassword', 'Show password')
                }
              >
                {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            </div>
            {isResend && (
              <p className="text-xs text-muted-foreground">
                {t('email.resendApiKeyHint', 'Get your API key from')}{' '}
                <a
                  href="https://resend.com/api-keys"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-primary underline underline-offset-2 hover:no-underline"
                >
                  resend.com/api-keys
                  <ExternalLink className="h-3 w-3 inline ml-0.5" />
                </a>
              </p>
            )}
            {provider === 'gmail' && (
              <p className="text-xs text-muted-foreground">
                {t(
                  'email.gmailHint',
                  'Use an App Password (not your regular password). Create one at',
                )}{' '}
                <a
                  href="https://myaccount.google.com/apppasswords"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-primary underline underline-offset-2 hover:no-underline"
                >
                  myaccount.google.com/apppasswords
                  <ExternalLink className="h-3 w-3 inline ml-0.5" />
                </a>
              </p>
            )}
            {provider === 'icloud' && (
              <p className="text-xs text-muted-foreground">
                {t('email.icloudHint', 'Use an App-Specific Password from')}{' '}
                <a
                  href="https://appleid.apple.com"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-primary underline underline-offset-2 hover:no-underline"
                >
                  appleid.apple.com
                  <ExternalLink className="h-3 w-3 inline ml-0.5" />
                </a>
              </p>
            )}
          </div>

          {/* Custom SMTP fields */}
          {isCustom && (
            <>
              <Separator />
              <div className="grid gap-4 sm:grid-cols-2">
                <div className="space-y-2">
                  <Label htmlFor="host">{t('email.hostLabel', 'SMTP Host')}</Label>
                  <Input
                    id="host"
                    value={host}
                    onChange={(e) => setHost(e.target.value)}
                    placeholder="smtp.example.com"
                    required={isCustom}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="port">{t('email.portLabel', 'Port')}</Label>
                  <Input
                    id="port"
                    type="number"
                    value={port}
                    onChange={(e) => setPort(e.target.value)}
                    placeholder="587"
                    required={isCustom}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="user">{t('email.userLabel', 'Username')}</Label>
                  <Input
                    id="user"
                    value={user}
                    onChange={(e) => setUser(e.target.value)}
                    placeholder={myEmail || 'user@example.com'}
                  />
                  <p className="text-xs text-muted-foreground">
                    {t('email.userHint', 'Defaults to the email address if empty.')}
                  </p>
                </div>
              </div>
            </>
          )}

          {/* Submit */}
          <div className="flex gap-2 pt-2">
            <Button type="submit" disabled={!myEmail || !password || isPending}>
              {isPending ? (
                <Loader2 className="h-4 w-4 mr-2 animate-spin" />
              ) : (
                <Settings className="h-4 w-4 mr-2" />
              )}
              {isPending ? t('email.saving', 'Saving...') : t('email.saveAndTest', 'Save & Test')}
            </Button>
          </div>

          {testResult && (
            <div
              className={cn(
                'text-sm p-3 rounded-md border',
                testResult.ok
                  ? 'border-success-subtle text-success bg-success/5'
                  : 'border-destructive/50 text-destructive bg-destructive/5',
              )}
            >
              {testResult.message}
            </div>
          )}
        </form>
      </CardContent>
    </Card>
  )
}

// ─── History Panel ─────────────────────────────────────────

function HistoryPanel({ active }: { active: boolean }) {
  const { t } = useTranslation()
  const { data, isLoading, isError, refetch, isFetching } = useEmailHistory(100, active)

  const emails = Array.isArray(data?.emails) ? data.emails : []

  if (isError) {
    return (
      <Card>
        <CardContent className="py-8">
          <ErrorState onRetry={() => refetch()} />
        </CardContent>
      </Card>
    )
  }

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between space-y-0">
        <CardTitle className="flex items-center gap-2">
          <History className="h-4 w-4" />
          {t('email.sentHistory', 'Sent History')}
        </CardTitle>
        <Button variant="ghost" size="icon" onClick={() => refetch()}>
          <RefreshCw className={cn('h-4 w-4', isFetching && 'animate-spin')} />
        </Button>
      </CardHeader>
      <CardContent>
        {isLoading ? (
          <LoadingCards count={3} />
        ) : emails.length === 0 ? (
          <div className="text-center py-8 text-sm text-muted-foreground">
            {t('email.noSentEmails', 'No emails sent yet.')}
          </div>
        ) : (
          <div className="space-y-3">
            {emails.map((email) => (
              <div
                key={email.id}
                className="flex items-start justify-between gap-4 rounded-lg border p-3"
              >
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium truncate">{email.subject}</p>
                  <p className="text-xs text-muted-foreground">
                    {t('email.to', 'To')}: {email.to}
                    {email.template_used &&
                      ` · ${t('email.template', 'template')}: ${email.template_used}`}
                  </p>
                </div>
                <div className="shrink-0 text-right">
                  <p className="text-xs text-muted-foreground">
                    {new Date(email.sent_at).toLocaleString()}
                  </p>
                  <SentEmailDialog email={email} />
                </div>
              </div>
            ))}
          </div>
        )}
      </CardContent>
      {data && data.total > data.limit && (
        <CardFooter className="text-xs text-muted-foreground">
          {t('email.showingLatest', 'Showing latest')} {data.emails.length} / {data.total}
        </CardFooter>
      )}
    </Card>
  )
}

function SentEmailDialog({ email }: { email: SentEmail }) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  return (
    <>
      <Button variant="link" size="sm" className="h-auto p-0 text-xs" onClick={() => setOpen(true)}>
        {t('email.viewDetails', 'View')}
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Mail className="h-4 w-4" />
              {email.subject}
            </DialogTitle>
          </DialogHeader>
          <div className="space-y-3 text-sm">
            <div className="grid grid-cols-2 gap-2">
              <div>
                <span className="text-muted-foreground">{t('email.to', 'To')}:</span> {email.to}
              </div>
              <div>
                <span className="text-muted-foreground">{t('email.sentAt', 'Sent at')}:</span>{' '}
                {new Date(email.sent_at).toLocaleString()}
              </div>
              <div>
                <span className="text-muted-foreground">ID:</span>{' '}
                <code className="text-xs">{email.id}</code>
              </div>
              {email.template_used && (
                <div>
                  <span className="text-muted-foreground">{t('email.template', 'Template')}:</span>{' '}
                  {email.template_used}
                </div>
              )}
            </div>
            <Separator />
            <div className="max-h-96 overflow-auto border rounded-md p-3 bg-muted/30">
              <div
                className="prose prose-sm dark:prose-invert max-w-none"
                dangerouslySetInnerHTML={{
                  __html: DOMPurify.sanitize(email.html_preview),
                }}
              />
            </div>
          </div>
        </DialogContent>
      </Dialog>
    </>
  )
}

// ─── Templates Panel ───────────────────────────────────────

function TemplatesPanel({ active }: { active: boolean }) {
  const { t } = useTranslation()
  const { data, isLoading, isError, refetch, isFetching } = useEmailTemplates(active)

  const templates = Array.isArray(data?.templates) ? data.templates : []

  if (isError) {
    return (
      <Card>
        <CardContent className="py-8">
          <ErrorState onRetry={() => refetch()} />
        </CardContent>
      </Card>
    )
  }

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between space-y-0">
        <CardTitle className="flex items-center gap-2">
          <LayoutTemplate className="h-4 w-4" />
          {t('email.emailTemplates', 'Email Templates')}
        </CardTitle>
        <Button variant="ghost" size="icon" onClick={() => refetch()}>
          <RefreshCw className={cn('h-4 w-4', isFetching && 'animate-spin')} />
        </Button>
      </CardHeader>
      <CardContent>
        {isLoading ? (
          <LoadingCards count={3} />
        ) : templates.length === 0 ? (
          <div className="text-center py-8 text-sm text-muted-foreground">
            {t(
              'email.noTemplates',
              'No templates yet. Agents can save templates when sending emails.',
            )}
          </div>
        ) : (
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {templates.map((tmpl) => (
              <div key={tmpl.name} className="rounded-lg border p-3">
                <p className="text-sm font-medium">{tmpl.name}</p>
                <p className="text-xs text-muted-foreground mt-1">
                  {(tmpl.size / 1024).toFixed(1)} KB
                </p>
                <p className="text-xs text-muted-foreground mt-2 line-clamp-3">{tmpl.preview}</p>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  )
}
