import { createFileRoute } from '@tanstack/react-router'
import { FolderPlus, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { CreateMountDialog } from '@/components/mount/create-mount-dialog'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useMounts, useDeleteMount } from '@/hooks/use-mounts'
import type { Mount } from '@/types'

export const Route = createFileRoute('/mounts/')({ component: MountsPage })

function MountsPage() {
  const { t } = useTranslation()
  const [search, setSearch] = useState('')
  const [showCreate, setShowCreate] = useState(false)

  const { data, isLoading, isError, refetch } = useMounts(search || undefined)
  const deleteMount = useDeleteMount()

  const mounts = Array.isArray(data?.items) ? data.items : []

  const handleDelete = async (mount: Mount) => {
    try {
      await deleteMount.mutateAsync(mount.id)
      toast.success(t('mounts.deleted', 'Mount가 삭제되었습니다'))
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t('mounts.deleteFailed', '삭제 실패'))
    }
  }

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('mounts.title', 'Mounts')}</h1>
          <p className="text-muted-foreground text-sm">
            {t(
              'mounts.desc',
              '경로 별칭. 이름을 언급하면 자동으로 컨텍스트에 주입됩니다.',
            )}
          </p>
        </div>
        <Button onClick={() => setShowCreate(true)}>
          <FolderPlus className="h-4 w-4 mr-2" />
          {t('mounts.create', 'Mount 만들기')}
        </Button>
      </div>

      {/* Search */}
      <div className="flex items-center gap-2">
        <Input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder={t('mounts.searchPlaceholder', '이름, 설명, 언어로 검색...')}
          className="max-w-xs"
        />
        <RefreshButton onClick={() => refetch()} />
      </div>

      {/* Content */}
      {isLoading ? (
        <LoadingCards />
      ) : isError ? (
        <ErrorState onRetry={() => refetch()} />
      ) : mounts.length === 0 ? (
        <EmptyState
          icon={<FolderPlus className="h-8 w-8" />}
          title={t('mounts.empty', 'Mount가 없습니다')}
          description={t(
            'mounts.emptyDesc',
            'Mount를 만들어 경로에 이름을 붙이세요. 에이전트가 자동으로 설명을 채웁니다.',
          )}
          action={
            <Button onClick={() => setShowCreate(true)}>
              <FolderPlus className="h-4 w-4 mr-2" />
              {t('mounts.create', 'Mount 만들기')}
            </Button>
          }
        />
      ) : (
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {mounts.map((mount) => (
            <div
              key={mount.id}
              className="group relative rounded-lg border bg-card p-4 transition-all hover:shadow-sm"
            >
              {/* Delete button */}
              <Button
                variant="ghost"
                size="icon"
                className="absolute right-2 top-2 h-7 w-7 opacity-0 transition-opacity group-hover:opacity-100"
                onClick={() => handleDelete(mount)}
              >
                <Trash2 className="h-3.5 w-3.5" />
              </Button>

              {/* Name */}
              <div className="mb-2 flex items-center gap-2">
                <span className="text-lg">🔧</span>
                <h3 className="font-semibold truncate">{mount.name}</h3>
                {mount.enrichment_pending && (
                  <span className="rounded-full bg-amber-500/10 px-2 py-0.5 text-xs text-amber-600">
                    {t('mounts.needsRefresh', '갱신 필요')}
                  </span>
                )}
              </div>

              {/* Path */}
              <p className="mb-2 text-xs text-muted-foreground truncate font-mono">
                {mount.paths[0] ?? '(no path)'}
              </p>

              {/* Auto-description */}
              {mount.auto_description && (
                <p className="mb-2 text-sm text-muted-foreground line-clamp-2">
                  {mount.auto_description}
                </p>
              )}

              {/* Languages + stack */}
              <div className="flex flex-wrap gap-1">
                {mount.auto_meta.languages.map((lang) => (
                  <span
                    key={lang}
                    className="rounded bg-primary/10 px-1.5 py-0.5 text-xs text-primary"
                  >
                    {lang}
                  </span>
                ))}
                {mount.auto_meta.stack.slice(0, 4).map((s) => (
                  <span
                    key={s}
                    className="rounded bg-muted px-1.5 py-0.5 text-xs text-muted-foreground"
                  >
                    {s}
                  </span>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Create dialog */}
      <CreateMountDialog open={showCreate} onOpenChange={setShowCreate} />
    </div>
  )
}
