import { ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Select } from '@/components/ui/select'

interface PaginationProps {
  page: number
  limit: number
  total: number
  onPageChange: (page: number) => void
  onLimitChange?: (limit: number) => void
  /** Maximum page buttons to show */
  maxButtons?: number
}

export function Pagination({
  page,
  limit,
  total,
  onPageChange,
  onLimitChange,
  maxButtons = 5,
}: PaginationProps) {
  const { t } = useTranslation()

  const totalPages = Math.max(1, Math.ceil(total / limit))
  const start = (page - 1) * limit + 1
  const end = Math.min(page * limit, total)

  // Build page number buttons
  const pages: (number | '...')[] = []
  if (totalPages <= maxButtons) {
    for (let i = 1; i <= totalPages; i++) pages.push(i)
  } else {
    // Always include first, last
    // Near current page: show up to 2 on each side
    pages.push(1)
    if (page > 3) pages.push('...')
    for (let i = Math.max(2, page - 1); i <= Math.min(totalPages - 1, page + 1); i++) {
      pages.push(i)
    }
    if (page < totalPages - 2) pages.push('...')
    pages.push(totalPages)
  }

  const go = (p: number) => {
    if (p < 1 || p > totalPages || p === page) return
    onPageChange(p)
  }

  return (
    <div className="flex items-center justify-between px-2 py-2 text-sm text-muted-foreground">
      <div className="flex items-center gap-2">
        {onLimitChange && (
          <Select
            value={String(limit)}
            onValueChange={(v) => onLimitChange(Number(v))}
            placeholder={String(limit)}
            options={[10, 20, 50, 100].map((n) => ({ label: `${n} / page`, value: String(n) }))}
            className="h-8 w-24"
          />
        )}
        <span>
          {t('dataTable.showing', 'Showing')} {start}–{end} {t('dataTable.of', 'of')} {total}
        </span>
      </div>

      <div className="flex items-center gap-1">
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8"
          onClick={() => go(1)}
          disabled={page <= 1}
          aria-label={t('common.firstPage', 'First page')}
        >
          <ChevronsLeft className="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8"
          onClick={() => go(page - 1)}
          disabled={page <= 1}
          aria-label={t('common.previousPage', 'Previous page')}
        >
          <ChevronLeft className="h-4 w-4" />
        </Button>

        {pages.map((p, i) =>
          p === '...' ? (
            <span key={`ellipsis-${i}`} className="px-1">
              …
            </span>
          ) : (
            <Button
              key={p}
              variant={p === page ? 'default' : 'ghost'}
              size="icon"
              className="h-8 w-8"
              onClick={() => go(p as number)}
            >
              {p}
            </Button>
          ),
        )}

        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8"
          onClick={() => go(page + 1)}
          disabled={page >= totalPages}
          aria-label={t('common.nextPage', 'Next page')}
        >
          <ChevronRight className="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8"
          onClick={() => go(totalPages)}
          disabled={page >= totalPages}
          aria-label={t('common.lastPage', 'Last page')}
        >
          <ChevronsRight className="h-4 w-4" />
        </Button>
      </div>
    </div>
  )
}
