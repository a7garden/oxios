import { ArrowDown, ArrowUp, ArrowUpDown, FolderOpen } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { ColumnFilter } from './column-filter'
import { EmptyState } from './empty-state'
import { LoadingCards } from './loading'
import { Pagination } from './pagination'
import { SearchBar } from './search-bar'

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface Column<T> {
  /** Column header label */
  header: string
  /** Key on T to read, or custom render function */
  accessor: keyof T | ((row: T) => React.ReactNode)
  /** Sort key — if provided, column is sortable */
  sortKey?: keyof T
  /** Filter definition — if provided, column is filterable */
  filter?: {
    options: { label: string; value: string }[]
  }
  /** Additional CSS class for cells */
  className?: string
  /** Mobile card view priority */
  mobilePriority?: 'primary' | 'secondary' | 'hidden'
}

export interface DataTableProps<T> {
  /** Column definitions */
  columns: Column<T>[]
  /** Row data */
  data: T[]
  /** Unique key extractor for each row */
  keyExtractor: (row: T) => string
  /** Row click handler */
  onRowClick?: (row: T) => void

  // Search
  /** Enable global search bar */
  searchable?: boolean
  /** Placeholder text for search input */
  searchPlaceholder?: string
  /** Fields to search across */
  searchKeys?: (keyof T)[]
  /** External search value (controlled) */
  searchValue?: string
  /** External search change handler */
  onSearchChange?: (value: string) => void

  // Filtering
  /** Column-level filter definitions */
  filterable?: {
    key: keyof T
    options: { label: string; value: string }[]
  }[]

  // Sorting
  /** Keys that are sortable */
  sortable?: (keyof T)[]
  /** Initial sort key */
  defaultSortKey?: keyof T
  /** Initial sort direction */
  defaultSortDir?: 'asc' | 'desc'

  // Pagination
  /** Enable pagination with given page size */
  pagination?: { pageSize: number }
  /** Show mobile card view (dual-render: hidden on md+ via CSS) */
  mobileCardView?: boolean

  // Display
  /** Empty state message */
  emptyMessage?: string
  /** Loading state */
  loading?: boolean
  /** Table caption for a11y */
  caption?: string
  /** Additional CSS class */
  className?: string
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

type SortState<T> = { key: keyof T; dir: 'asc' | 'desc' } | null

function getCellValue<T>(row: T, accessor: Column<T>['accessor']): React.ReactNode {
  if (typeof accessor === 'function') return accessor(row)
  const val = row[accessor]
  if (val === null || val === undefined) return ''
  return String(val)
}

function getStringValue<T>(row: T, key: keyof T): string {
  const val = row[key]
  if (val === null || val === undefined) return ''
  if (typeof val === 'string') return val.toLowerCase()
  if (typeof val === 'number' || typeof val === 'boolean') return String(val).toLowerCase()
  if (val instanceof Date) return val.toISOString().toLowerCase()
  return String(val).toLowerCase()
}

// ---------------------------------------------------------------------------
// CardRow (mobile card view)
// ---------------------------------------------------------------------------

function CardRow<T>({
  row,
  columns,
  onRowClick,
}: {
  row: T
  columns: Column<T>[]
  onRowClick?: (row: T) => void
}) {
  const primary = columns.find((c) => c.mobilePriority === 'primary')
  const secondary = columns.filter((c) => c.mobilePriority === 'secondary')

  return (
    <button
      type="button"
      onClick={() => onRowClick?.(row)}
      className="w-full text-left p-4 hover:bg-muted/50 active:bg-muted transition-colors"
    >
      {primary && (
        <div className="font-medium text-sm truncate">{getCellValue(row, primary.accessor)}</div>
      )}
      {secondary.length > 0 && (
        <div className="mt-1 grid grid-cols-2 gap-x-3 gap-y-1 text-xs text-muted-foreground">
          {secondary.map((col) => (
            <div key={String(col.accessor)} className="flex gap-1 min-w-0">
              <span className="opacity-60 shrink-0">{col.header}:</span>
              <span className="truncate">{getCellValue(row, col.accessor)}</span>
            </div>
          ))}
        </div>
      )}
    </button>
  )
}

// ---------------------------------------------------------------------------
// DataTable
// ---------------------------------------------------------------------------

export function DataTable<T>({
  columns,
  data,
  keyExtractor,
  onRowClick,
  searchable,
  searchPlaceholder,
  searchKeys,
  searchValue: externalSearch,
  onSearchChange,
  filterable,
  sortable,
  defaultSortKey,
  defaultSortDir,
  pagination,
  mobileCardView,
  emptyMessage,
  loading,
  caption,
  className,
}: DataTableProps<T>) {
  const { t } = useTranslation()

  // ── Internal state (uncontrolled mode) ──
  const [internalSearch, setInternalSearch] = useState('')
  const [sort, setSort] = useState<SortState<T>>(
    defaultSortKey ? { key: defaultSortKey, dir: defaultSortDir ?? 'asc' } : null,
  )
  const [filters, setFilters] = useState<Record<string, string[]>>({})
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(pagination?.pageSize ?? 20)

  // Use external search value if provided
  const search = externalSearch ?? internalSearch
  const setSearch = onSearchChange ?? setInternalSearch

  // ── Filter / Search / Sort ──
  const processed = useMemo(() => {
    let result = [...data]

    // Search
    if (search) {
      const lower = search.toLowerCase()
      const keys =
        searchKeys ??
        (columns
          .map((c) => (typeof c.accessor === 'function' ? undefined : c.accessor))
          .filter(Boolean) as (keyof T)[])
      result = result.filter((row) => keys.some((key) => getStringValue(row, key).includes(lower)))
    }

    // Filters
    for (const [key, selected] of Object.entries(filters)) {
      if (selected.length === 0) continue
      const typedKey = key as keyof T
      result = result.filter((row) => {
        const val = getStringValue(row, typedKey)
        return selected.some((s) => val === s.toLowerCase())
      })
    }

    // Sort
    if (sort) {
      result.sort((a, b) => {
        const aVal = getStringValue(a, sort.key)
        const bVal = getStringValue(b, sort.key)
        const cmp = aVal.localeCompare(bVal)
        return sort.dir === 'asc' ? cmp : -cmp
      })
    }

    return result
  }, [data, search, filters, sort, searchKeys, columns])

  // ── Pagination ──
  const total = processed.length
  const totalPages = Math.max(1, Math.ceil(total / pageSize))
  const safePage = Math.min(page, totalPages)
  const paginated = pagination
    ? processed.slice((safePage - 1) * pageSize, safePage * pageSize)
    : processed

  // Reset page when search/filters change
  const handleSearch = (val: string) => {
    setSearch(val)
    setPage(1)
  }

  const handleFilterChange = (key: string, selected: string[]) => {
    setFilters((prev) => ({ ...prev, [key]: selected }))
    setPage(1)
  }

  const handleSort = (key: keyof T) => {
    setSort((prev) => {
      if (!prev || prev.key !== key) return { key, dir: 'asc' }
      if (prev.dir === 'asc') return { key, dir: 'desc' }
      return null // Third click: remove sort
    })
  }

  // ── Render ──
  const hasToolbar = searchable || (filterable && filterable.length > 0)

  if (loading) return <LoadingCards count={3} />

  return (
    <div className={cn('rounded-xl border', className)}>
      {/* Toolbar */}
      {hasToolbar && (
        <div className="flex items-center gap-2 px-4 py-3 border-b bg-muted/30">
          {searchable && (
            <SearchBar value={search} onChange={handleSearch} placeholder={searchPlaceholder} />
          )}
          {filterable?.map((f) => (
            <ColumnFilter
              key={String(f.key)}
              columnKey={String(f.key)}
              label={
                columns.find((c) => {
                  const acc = c.accessor
                  return typeof acc !== 'function' && acc === f.key
                })?.header ?? String(f.key)
              }
              options={f.options}
              selected={filters[String(f.key)] ?? []}
              onChange={(selected) => handleFilterChange(String(f.key), selected)}
            />
          ))}
          {sort && (
            <span className="ml-auto text-xs text-muted-foreground">
              {t('dataTable.sortedBy')}{' '}
              {columns.find((c) => {
                const acc = c.accessor
                return typeof acc !== 'function' && acc === sort.key
              })?.header ?? String(sort.key)}{' '}
              {sort.dir === 'asc' ? '↑' : '↓'}
            </span>
          )}
        </div>
      )}

      {/* Table — hidden on mobile when card view active */}
      <div className={cn('overflow-x-auto', mobileCardView && 'hidden md:block')}>
        <table className="w-full" aria-label={caption}>
          {caption && <caption className="sr-only">{caption}</caption>}
          <thead>
            <tr className="border-b bg-muted/50">
              {columns.map((col) => {
                const canSort = sortable?.includes(
                  col.sortKey ??
                    ((typeof col.accessor !== 'function' ? col.accessor : undefined) as keyof T),
                )
                const sortKey =
                  col.sortKey ??
                  ((typeof col.accessor !== 'function' ? col.accessor : undefined) as keyof T)
                const isSorted = sort?.key === sortKey

                return (
                  <th
                    key={col.header}
                    scope="col"
                    className={cn(
                      'px-4 py-3 text-left text-sm font-medium text-muted-foreground',
                      canSort && 'cursor-pointer select-none hover:text-foreground',
                      col.className,
                    )}
                    onClick={() => canSort && sortKey && handleSort(sortKey)}
                  >
                    <span className="inline-flex items-center gap-1">
                      {col.header}
                      {canSort &&
                        (isSorted ? (
                          sort?.dir === 'asc' ? (
                            <ArrowUp className="h-3.5 w-3.5" />
                          ) : (
                            <ArrowDown className="h-3.5 w-3.5" />
                          )
                        ) : (
                          <ArrowUpDown className="h-3.5 w-3.5 opacity-30" />
                        ))}
                    </span>
                  </th>
                )
              })}
            </tr>
          </thead>
          <tbody>
            {paginated.length === 0 ? (
              <tr>
                <td colSpan={columns.length} className="py-12 text-center">
                  <EmptyState
                    icon={<FolderOpen className="h-8 w-8" />}
                    title={emptyMessage ?? t('dataTable.noResults')}
                    className="py-6"
                  />
                </td>
              </tr>
            ) : (
              paginated.map((row) => (
                <tr
                  key={keyExtractor(row)}
                  className={cn(
                    'border-b last:border-0 transition-all',
                    onRowClick &&
                      'cursor-pointer select-none hover:bg-muted/80 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring focus-visible:ring-inset',
                  )}
                  onClick={() => onRowClick?.(row)}
                  tabIndex={onRowClick ? 0 : undefined}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter') onRowClick?.(row)
                  }}
                >
                  {columns.map((col) => (
                    <td
                      key={String(col.accessor)}
                      className={cn('px-4 py-3 text-sm', col.className)}
                    >
                      {getCellValue(row, col.accessor)}
                    </td>
                  ))}
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      {pagination && total > 0 && (
        <div className="border-t">
          <Pagination
            page={safePage}
            limit={pageSize}
            total={total}
            onPageChange={setPage}
            onLimitChange={setPageSize}
          />
        </div>
      )}

      {/* Mobile card view */}
      {mobileCardView !== false && (
        <div className="divide-y md:hidden">
          {paginated.map((row) => (
            <CardRow key={keyExtractor(row)} row={row} columns={columns} onRowClick={onRowClick} />
          ))}
        </div>
      )}
    </div>
  )
}
