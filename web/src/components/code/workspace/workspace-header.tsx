import {
  LayoutGrid,
  PanelLeft,
  PanelRight,
  Save,
  Sparkles,
  Terminal as TerminalIcon,
} from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { useCodeLayoutStore, useCodeSessionStore } from '@/stores/code/code-session'
import { useCodeActions } from './use-code-actions'

export function WorkspaceHeader() {
  const { session } = useCodeSessionStore()
  const {
    toggleExplorer,
    toggleAgent,
    toggleTerminal,
    toggleCanvas,
    showExplorer,
    showAgent,
    showTerminal,
    showCanvas,
  } = useCodeLayoutStore()
  const { saveAll } = useCodeActions()

  return (
    <div className="flex h-12 items-center gap-2 border-b bg-surface px-3">
      <div className="flex items-center gap-2">
        <Sparkles className="h-4 w-4 text-primary" />
        <span className="font-semibold text-sm">{session?.title ?? 'Code Workspace'}</span>
        {session?.model && <span className="text-xs text-muted-foreground">· {session.model}</span>}
      </div>

      <div className="flex-1" />

      <div className="flex items-center gap-1">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon" className="h-8 w-8" onClick={saveAll}>
              <Save className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Save All (⌘S)</TooltipContent>
        </Tooltip>

        <Separator orientation="vertical" className="h-5 mx-1" />

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant={showExplorer ? 'secondary' : 'ghost'}
              size="icon"
              className="h-8 w-8"
              onClick={toggleExplorer}
            >
              <PanelLeft className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Toggle Explorer</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant={showTerminal ? 'secondary' : 'ghost'}
              size="icon"
              className="h-8 w-8"
              onClick={toggleTerminal}
            >
              <TerminalIcon className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Toggle Terminal</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant={showCanvas ? 'secondary' : 'ghost'}
              size="icon"
              className="h-8 w-8"
              onClick={toggleCanvas}
            >
              <LayoutGrid className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Toggle Project Canvas</TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant={showAgent ? 'secondary' : 'ghost'}
              size="icon"
              className="h-8 w-8"
              onClick={toggleAgent}
            >
              <PanelRight className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Toggle Agent</TooltipContent>
        </Tooltip>
      </div>
    </div>
  )
}
