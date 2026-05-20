import { ArrowLeft, Menu } from 'lucide-react'
import { useRouterState } from '@tanstack/react-router'
import { Separator } from '@/components/ui/separator'
import { useSidebarStore } from '@/stores/sidebar'
import { useKnowledgeStore } from '@/stores/knowledge'
import { cn } from '@/lib/utils'

export function Header() {
  const { setMobileOpen } = useSidebarStore()
  const router = useRouterState()
  const pathname = router.location.pathname
  const isKnowledge = pathname.startsWith('/knowledge')

  // Get knowledge state for breadcrumb
  const currentFilePath = useKnowledgeStore((s) => s.currentFilePath)
  const mode = useKnowledgeStore((s) => s.mode)

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

      {isKnowledge ? (
        <>
          <a
            href="/"
            className="flex items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground transition-colors"
          >
            <ArrowLeft className="h-4 w-4" />
            <span>Dashboard</span>
          </a>
          <Separator orientation="vertical" className="h-6" />
          <div className="flex items-center gap-2 text-sm">
            <span className={cn('font-medium', !currentFilePath && 'text-foreground')}>
              Knowledge
            </span>
            {currentFilePath && (
              <>
                <span className="text-muted-foreground">/</span>
                <span className="text-muted-foreground truncate max-w-[200px]">
                  {currentFilePath.replace(/\.md$/, '')}
                </span>
              </>
            )}
            {mode === 'chat' && !currentFilePath && (
              <>
                <span className="text-muted-foreground">/</span>
                <span className="text-muted-foreground">Chat</span>
              </>
            )}
          </div>
        </>
      ) : (
        <Separator orientation="vertical" className="hidden lg:block h-6" />
      )}

      <div className="flex-1" />
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <div className="h-2 w-2 rounded-full bg-emerald-500" aria-hidden="true" />
        <span>Oxios Agent OS</span>
      </div>
    </header>
  )
}
