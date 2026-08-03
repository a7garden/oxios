import { createFileRoute } from '@tanstack/react-router'
import { CodeWorkspaceRoute } from '@/components/code/workspace/code-workspace-route'

export const Route = createFileRoute('/code/$sessionId')({
  component: CodeWorkspaceRoute,
})
