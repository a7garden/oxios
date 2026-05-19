import { cn } from '@/lib/utils'

interface Column<T> {
  header: string
  accessor: keyof T | ((row: T) => React.ReactNode)
  className?: string
}

interface DataTableProps<T> {
  columns: Column<T>[]
  data: T[]
  keyExtractor: (row: T) => string
  onRowClick?: (row: T) => void
  className?: string
}

export function DataTable<T>({ columns, data, keyExtractor, onRowClick, className }: DataTableProps<T>) {
  return (
    <div className={cn('rounded-xl border', className)}>
      <div className="overflow-x-auto">
        <table className="w-full">
          <thead>
            <tr className="border-b bg-muted/50">
              {columns.map((col, i) => (
                <th key={i} className={cn('px-4 py-3 text-left text-sm font-medium text-muted-foreground', col.className)}>
                  {col.header}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data.map((row) => (
              <tr
                key={keyExtractor(row)}
                className={cn('border-b last:border-0 transition-colors', onRowClick && 'cursor-pointer hover:bg-muted/50')}
                onClick={() => onRowClick?.(row)}
              >
                {columns.map((col, i) => (
                  <td key={i} className={cn('px-4 py-3 text-sm', col.className)}>
                    {typeof col.accessor === 'function' ? col.accessor(row) : String(row[col.accessor] ?? '')}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}
