import { useQuery } from '@tanstack/react-query'
import { Code, FileText, Pencil } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { Button } from '@/components/ui/button'
import { api } from '@/lib/api-client'
import { cn } from '@/lib/utils'
import type { Skill } from '@/types'

interface SkillContentProps {
  skill: Skill
  onEdit?: () => void
}

/**
 * Content tab of the skill inspector (design F4).
 *
 * Lazy-fetches the raw SKILL.md body only when this component mounts (i.e. when
 * the user opens the Content tab), then renders it as markdown. A Raw toggle
 * shows the verbatim source. The Edit button opens the inline editor (F3).
 */
export function SkillContent({ skill, onEdit }: SkillContentProps) {
  const { t } = useTranslation()
  const [raw, setRaw] = useState(false)

  const {
    data: content,
    isLoading,
    isError,
  } = useQuery({
    queryKey: ['skill', skill.name, 'content'],
    queryFn: async () => {
      const r = await api.get<{ name: string; content: string }>(
        `/api/skills/${encodeURIComponent(skill.name)}/content`,
      )
      return r?.content ?? ''
    },
    staleTime: 30_000,
  })

  if (isLoading) {
    return (
      <div className="space-y-2">
        <div className="h-3 bg-muted rounded w-3/4 animate-pulse" />
        <div className="h-3 bg-muted rounded w-1/2 animate-pulse" />
        <div className="h-3 bg-muted rounded w-2/3 animate-pulse" />
      </div>
    )
  }

  if (isError) {
    return <p className="text-sm text-destructive">{t('skills.loadContentFailed')}</p>
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="sm" className="gap-1.5" onClick={() => setRaw((v) => !v)}>
          {raw ? <FileText className="h-3.5 w-3.5" /> : <Code className="h-3.5 w-3.5" />}
          {raw ? t('skills.content') : t('skills.viewRaw')}
        </Button>
        {onEdit && (
          <Button variant="outline" size="sm" className="gap-1.5 ml-auto" onClick={onEdit}>
            <Pencil className="h-3.5 w-3.5" />
            {t('skills.edit')}
          </Button>
        )}
      </div>

      {raw ? (
        <pre
          className={cn(
            'text-xs font-mono whitespace-pre-wrap break-words',
            'bg-background border rounded-md p-3 max-h-[420px] overflow-auto',
            'text-muted-foreground leading-relaxed',
          )}
        >
          {content}
        </pre>
      ) : (
        <div className="prose prose-sm dark:prose-invert max-w-none text-muted-foreground [&_code]:font-mono [&_code]:text-xs [&_pre]:bg-background [&_pre]:border">
          <ReactMarkdown remarkPlugins={[remarkGfm]}>{content ?? ''}</ReactMarkdown>
        </div>
      )}
    </div>
  )
}
