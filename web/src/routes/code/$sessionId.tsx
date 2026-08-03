import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/code/$sessionId')({
  component: CodeWorkspaceRoute,
})

function CodeWorkspaceRoute() {
  const { sessionId } = Route.useParams()
  return (
    <div className="flex h-full items-center justify-center">
      <p className="text-muted-foreground">Code Workspace: {sessionId}</p>
    </div>
  )
}
