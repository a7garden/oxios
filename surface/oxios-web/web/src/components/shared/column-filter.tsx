import { ChevronDown, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuItem } from '@/components/ui/dropdown-menu'

interface FilterOption {
  label: string
  value: string
}

interface ColumnFilterProps {
  columnKey: string
  label: string
  options: FilterOption[]
  selected: string[]
  onChange: (selected: string[]) => void
}

export function ColumnFilter({ label, options, selected, onChange }: ColumnFilterProps) {
  const { t } = useTranslation()

  return (
    <DropdownMenu
      trigger={
        <Button variant="outline" size="sm" className="h-9">
          {label}
          {selected.length > 0 && (
            <span className="ml-1.5 flex h-4 min-w-4 items-center justify-center rounded bg-primary px-1 text-2xs font-medium text-primary-foreground">
              {selected.length}
            </span>
          )}
          <ChevronDown className="ml-1 h-3.5 w-3.5" />
        </Button>
      }
      align="left"
    >
      {options.map((opt) => {
        const isSelected = selected.includes(opt.value)
        return (
          <DropdownMenuItem
            key={opt.value}
            onClick={() => {
              if (isSelected) {
                onChange(selected.filter((v) => v !== opt.value))
              } else {
                onChange([...selected, opt.value])
              }
            }}
            className="cursor-pointer"
          >
            <span className={isSelected ? 'opacity-100' : 'opacity-30 mr-2'}>
              {isSelected ? '✓' : ''}
            </span>
            {opt.label}
          </DropdownMenuItem>
        )
      })}
      {selected.length > 0 && (
        <DropdownMenuItem
          onClick={() => onChange([])}
          className="cursor-pointer text-muted-foreground border-t mt-1"
        >
          <X className="mr-2 h-3.5 w-3.5" />
          {t('dataTable.clearFilters', 'Clear filters')}
        </DropdownMenuItem>
      )}
    </DropdownMenu>
  )
}
