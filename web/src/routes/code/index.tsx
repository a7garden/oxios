import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useState, useCallback, useEffect } from 'react'
import { Code2, FolderOpen, ChevronRight, Home, Loader2, ArrowLeft } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Separator } from '@/components/ui/separator'
import { codeApi } from '@/lib/code-api'
import type { DirEntry } from '@/types/code'

export const Route = createFileRoute('/code/')({
  component: CodeSessionPicker,
})

function CodeSessionPicker() {
  const navigate = useNavigate()
  const [browsePath, setBrowsePath] = useState<string>(
    sessionStorage.getItem('oxios-code-last-path') || '/Volumes/MERCURY/PROJECTS',
  )
  const [entries, setEntries] = useState<DirEntry[]>([])
  const [loading, setLoading] = useState(false)
  const [creating, setCreating] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const browse = useCallback(
    async (path: string) => {
      setLoading(true)
      setError(null)
      try {
        const result = await codeApi.browse(path)
        setEntries(result)
        setBrowsePath(path)
        sessionStorage.setItem('oxios-code-last-path', path)
      } catch (e) {
        setError(e instanceof Error ? e.message : 'Failed to browse directory')
        setEntries([])
      } finally {
        setLoading(false)
      }
    },
    [],
  )

  // Initial browse on mount
  useEffect(() => {
    browse(browsePath)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  async function openProject(path: string) {
    setCreating(true)
    try {
      const session = await codeApi.createSession(path)
      navigate({ to: '/code/$sessionId', params: { sessionId: session.id } })
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to create session')
    } finally {
      setCreating(false)
    }
  }

  const pathParts = browsePath.split('/').filter(Boolean)

  return (
    <div className="flex h-full flex-col bg-surface">
      {/* Header */}
      <div className="flex h-14 items-center gap-3 border-b px-6">
        <Code2 className="h-6 w-6 text-primary" />
        <h1 className="text-lg font-bold">Code Workspace</h1>
      </div>

      <div className="flex flex-1 overflow-hidden">
        {/* Directory browser */}
        <div className="flex flex-1 flex-col overflow-hidden">
          <div className="flex items-center gap-1 border-b px-3 py-2">
            <Button
              variant="ghost"
              size="sm"
              className="h-7 px-2"
              onClick={() => {
                const parent = browsePath.split('/').slice(0, -1).join('/') || '/'
                browse(parent)
              }}
            >
              <ArrowLeft className="h-4 w-4" />
            </Button>
            <Button variant="ghost" size="sm" className="h-7 px-2" onClick={() => browse('/')}>
              <Home className="h-4 w-4" />
            </Button>
            <Separator orientation="vertical" className="h-4 mx-1" />
            <div className="flex items-center gap-0.5 overflow-x-auto text-sm">
              <button
                className="px-1.5 py-0.5 rounded hover:bg-surface-sunken text-muted-foreground"
                onClick={() => browse('/')}
              >
                /
              </button>
              {pathParts.map((part, i) => {
                const fullPath = '/' + pathParts.slice(0, i + 1).join('/')
                const isLast = i === pathParts.length - 1
                return (
                  <div key={fullPath} className="flex items-center gap-0.5">
                    <ChevronRight className="h-3 w-3 text-muted-foreground/50" />
                    <button
                      className={`px-1.5 py-0.5 rounded hover:bg-surface-sunken ${
                        isLast ? 'font-medium text-text' : 'text-muted-foreground'
                      }`}
                      onClick={() => browse(fullPath)}
                    >
                      {part}
                    </button>
                  </div>
                )
              })}
            </div>
          </div>

          <ScrollArea className="flex-1">
            <div className="p-2">
              {loading && (
                <div className="flex items-center justify-center py-8 text-muted-foreground">
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                  Loading...
                </div>
              )}
              {error && (
                <div className="px-3 py-4 text-sm text-destructive">{error}</div>
              )}
              {!loading && !error && entries.length === 0 && (
                <div className="px-3 py-4 text-sm text-muted-foreground">
                  No directories found.
                </div>
              )}
              {!loading &&
                entries.map((entry) => (
                  <button
                    key={entry.path}
                    className="flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-sm hover:bg-surface-sunken transition-colors text-left"
                    onClick={() => entry.is_dir && browse(entry.path)}
                    onDoubleClick={() => entry.is_dir && openProject(entry.path)}
                  >
                    <FolderOpen
                      className={`h-4 w-4 ${entry.is_dir ? 'text-primary' : 'text-muted-foreground'}`}
                    />
                    <span className="flex-1 truncate">{entry.name}</span>
                    {entry.is_dir && (
                      <span className="text-xs text-muted-foreground">Open →</span>
                    )}
                  </button>
                ))}
            </div>
          </ScrollArea>
        </div>

        {/* Action panel */}
        <div className="w-72 border-l p-4 flex flex-col gap-4">
          <div>
            <h2 className="text-sm font-semibold mb-2">Selected Path</h2>
            <code className="block rounded-md bg-surface-sunken px-3 py-2 text-xs break-all">
              {browsePath}
            </code>
          </div>

          <Button
            className="w-full"
            size="lg"
            disabled={creating}
            onClick={() => openProject(browsePath)}
          >
            {creating ? (
              <>
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                Creating...
              </>
            ) : (
              <>
                <FolderOpen className="mr-2 h-4 w-4" />
                Open as Project
              </>
            )}
          </Button>

          <div className="text-xs text-muted-foreground space-y-1">
            <p>Double-click a directory to open it as a project.</p>
            <p>The agent will have full file access within the selected directory.</p>
          </div>
        </div>
      </div>
    </div>
  )
}
