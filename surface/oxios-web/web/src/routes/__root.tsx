import type { QueryClient } from '@tanstack/react-query'
import { QueryClientProvider } from '@tanstack/react-query'
import { createRootRouteWithContext } from '@tanstack/react-router'
import { AppLayout } from '@/components/layout/app-layout'
import { ErrorBoundary } from '@/components/shared/error-boundary'
import { Toaster } from '@/components/ui/sonner'
import { TooltipProvider } from '@/components/ui/tooltip'

interface RouterContext {
  queryClient: QueryClient
}

export const Route = createRootRouteWithContext<RouterContext>()({
  component: function RootComponent() {
    const { queryClient } = Route.useRouteContext()
    return (
      <QueryClientProvider client={queryClient}>
        <TooltipProvider>
            <ErrorBoundary>
              <AppLayout />
            </ErrorBoundary>
          <Toaster />
        </TooltipProvider>
      </QueryClientProvider>
    )
  },
})
