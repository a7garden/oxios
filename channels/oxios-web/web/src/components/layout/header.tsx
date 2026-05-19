import { Menu } from 'lucide-react'
import { Separator } from '@/components/ui/separator'
import { useSidebarStore } from '@/stores/sidebar'

export function Header() {
  const { setMobileOpen } = useSidebarStore()

  return (
    <header className="flex h-14 items-center gap-4 border-b bg-background px-4 lg:px-6">
      <button
        type="button"
        className="lg:hidden"
        onClick={() => setMobileOpen(true)}
        aria-label="Open navigation menu"
      >
        <Menu className="h-5 w-5" />
      </button>
      <Separator orientation="vertical" className="hidden lg:block h-6" />
      <div className="flex-1" />
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <div className="h-2 w-2 rounded-full bg-emerald-500" aria-hidden="true" />
        <span>Oxios Agent OS</span>
      </div>
    </header>
  )
}
