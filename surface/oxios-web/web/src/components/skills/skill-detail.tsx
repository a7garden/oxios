import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Power, Trash2, X } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useToast } from '@/components/ui/sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { api } from '@/lib/api-client'
import { cn } from '@/lib/utils'
import { SkillUpdateIndicator } from './update-badge'
import type { Skill, SkillFormat } from '@/types'

const FORMAT_META: Record<SkillFormat, { label: string; variant: 'default' | 'secondary' | 'outline'; description: string }> = {
  oxios: { label: 'Oxios', variant: 'default', description: 'Oxios native skill' },
  openclaw: { label: 'OpenClaw', variant: 'secondary', description: 'ClawHub marketplace skill' },
  claude_code: { label: 'Claude', variant: 'outline', description: 'Claude Code skill — core instructions compatible, some features may not apply' },
  agent_skills: { label: 'Standard', variant: 'outline', description: 'Agent Skills standard (agentskills.io)' },
}

function FormatBadge({ format }: { format: SkillFormat }) {
  const m = FORMAT_META[format]
  return <Badge variant={m.variant} className="text-xs" title={m.description}>{m.label}</Badge>
}

interface SkillDetailProps {
  skill: Skill
  onClose: () => void
}

export function SkillDetail({ skill, onClose }: SkillDetailProps) {
  const { t } = useTranslation()
  const qc = useQueryClient()
  const { toast } = useToast()
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false)
  const isDisabled = skill.status === 'disabled'

  const toggleMutation = useMutation({
    mutationFn: () => {
      const endpoint = isDisabled
        ? `/api/skills/${encodeURIComponent(skill.name)}/enable`
        : `/api/skills/${encodeURIComponent(skill.name)}/disable`
      return api.post(endpoint)
    },
    onSuccess: () => {
      toast(t('skills.toggleSuccess'), 'success')
      qc.invalidateQueries({ queryKey: ['skills'] })
    },
    onError: (err: unknown) => {
      toast(err instanceof Error ? err.message : t('common.error'), 'destructive')
    },
  })

  const deleteMutation = useMutation({
    mutationFn: () => api.delete(`/api/skills/${encodeURIComponent(skill.name)}`),
    onSuccess: () => {
      toast(t('skills.deleteSuccess', { name: skill.name }), 'success')
      qc.invalidateQueries({ queryKey: ['skills'] })
      onClose()
    },
    onError: (err: unknown) => {
      toast(err instanceof Error ? err.message : t('common.error'), 'destructive')
    },
  })

  const hasRequirements =
    skill.requirements.bins.length > 0 ||
    skill.requirements.anyBins.length > 0 ||
    skill.requirements.env.length > 0 ||
    skill.requirements.config.length > 0

  return (
    <div className="space-y-5">
      {/* Header */}
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-start gap-3 min-w-0">
          <span className="text-2xl leading-none mt-0.5 shrink-0">{skill.emoji || '⚡'}</span>
          <div className="min-w-0">
            <h2 className="text-lg font-semibold leading-tight">{skill.name}</h2>
            {skill.description && (
              <p className="text-sm text-muted-foreground mt-1">{skill.description}</p>
            )}
          </div>
        </div>
        <Button variant="ghost" size="icon" className="shrink-0" onClick={onClose}>
          <X className="h-4 w-4" />
        </Button>
      </div>

      {/* Meta badges */}
      <div className="flex flex-wrap items-center gap-2">
        <FormatBadge format={skill.format} />
        <Badge variant={skill.source === 'bundled' ? 'secondary' : skill.source === 'managed' ? 'outline' : 'default'} className="text-xs">
          {skill.source}
        </Badge>
        {skill.version && (
          <Badge variant="outline" className="text-xs font-mono">
            v{skill.version}
          </Badge>
        )}
        {skill.author && (
          <span className="text-xs text-muted-foreground">{t('skills.by')} {skill.author}</span>
        )}
        <SkillUpdateIndicator slug={skill.name} />
      </div>

      {/* Status */}
      <div className="rounded-md border px-3 py-2">
        <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-1">{t('common.details')}</p>
        <div className="flex items-center gap-2 text-sm">
          <span>{t('skills.version')}:</span>
          <span className="font-mono">{skill.version || '—'}</span>
        </div>
        {skill.homepage && (
          <div className="flex items-center gap-2 text-sm mt-1">
            <span>Homepage:</span>
            <a href={skill.homepage} target="_blank" rel="noopener noreferrer" className="text-blue-600 dark:text-blue-400 hover:underline truncate">
              {skill.homepage}
            </a>
          </div>
        )}
        <div className="flex items-center gap-2 text-sm mt-1">
          <span>Path:</span>
          <span className="font-mono text-xs truncate" title={skill.file_path}>{skill.file_path}</span>
        </div>
        {skill.os.length > 0 && (
          <div className="flex items-center gap-2 text-sm mt-1">
            <span>OS:</span>
            <span>{skill.os.join(', ')}</span>
          </div>
        )}
      </div>

      {/* Requirements */}
      {hasRequirements && (
        <div className="space-y-2">
          <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">{t('skills.requires')}</p>
          <div className="rounded-md border px-3 py-2 space-y-1.5">
            {skill.requirements.bins.length > 0 && (
              <ReqLine label={t('skills.bins')} items={skill.requirements.bins} missing={skill.missing.bins} />
            )}
            {skill.requirements.anyBins.length > 0 && (
              <ReqLine label={t('skills.anyBins')} items={skill.requirements.anyBins} missing={skill.missing.anyBins} />
            )}
            {skill.requirements.env.length > 0 && (
              <ReqLine label={t('skills.env')} items={skill.requirements.env} missing={skill.missing.env} />
            )}
            {skill.requirements.config.length > 0 && (
              <ReqLine label={t('skills.config')} items={skill.requirements.config} missing={skill.missing.config} />
            )}
          </div>
        </div>
      )}

      {/* Install specs */}
      {skill.install.length > 0 && (
        <div className="space-y-2">
          <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">{t('skills.install')}</p>
          <div className="rounded-md border px-3 py-2 space-y-1">
            {skill.install.map((sp, i) => (
              <div key={`${sp.kind}-${i}`} className="flex items-center gap-2 text-sm text-muted-foreground">
                <span className="text-xs font-mono bg-muted px-1.5 py-0.5 rounded">{sp.kind}</span>
                <span>{sp.label ?? sp.bins.join(', ')}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Actions */}
      <div className="flex items-center gap-2 pt-2 border-t">
        <Button
          variant={isDisabled ? 'default' : 'outline'}
          size="sm"
          onClick={() => toggleMutation.mutate()}
          disabled={toggleMutation.isPending}
          className="gap-1.5"
        >
          <Power className="h-3.5 w-3.5" />
          {isDisabled ? t('skills.enable') : t('skills.disable')}
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => setShowDeleteConfirm(true)}
          className="gap-1.5 text-destructive hover:text-destructive"
        >
          <Trash2 className="h-3.5 w-3.5" />
          {t('skills.delete')}
        </Button>
      </div>

      {/* Delete confirmation dialog */}
      <Dialog open={showDeleteConfirm} onOpenChange={setShowDeleteConfirm}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t('skills.deleteConfirm')}</DialogTitle>
            <DialogDescription>
              {t('skills.deleteDescription', { name: skill.name })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="outline" size="sm" onClick={() => setShowDeleteConfirm(false)}>
              {t('common.cancel')}
            </Button>
            <Button
              variant="destructive"
              size="sm"
              onClick={() => deleteMutation.mutate()}
              disabled={deleteMutation.isPending}
            >
              {t('common.delete')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}

function ReqLine({ label, items, missing }: { label: string; items: string[]; missing: string[] }) {
  const { t } = useTranslation()
  return (
    <div className="flex items-start gap-2 text-xs">
      <span className="text-muted-foreground w-16 shrink-0 pt-px">{label}</span>
      <div className="flex flex-wrap gap-x-3 gap-y-0.5">
        {items.map((item) => {
          const m = missing.includes(item)
          return (
            <span
              key={item}
              className={cn(m ? 'text-red-600 dark:text-red-400' : 'text-emerald-600 dark:text-emerald-400')}
            >
              {item}{m && ` (${t('skills.missing')})`}
            </span>
          )
        })}
      </div>
    </div>
  )
}
