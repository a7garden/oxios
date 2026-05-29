import type { QueryClient } from '@tanstack/react-query'
import { QueryClientProvider } from '@tanstack/react-query'
import { createRootRouteWithContext } from '@tanstack/react-router'
import { AppLayout } from '@/components/layout/app-layout'
import { ToastProvider } from '@/components/ui/sonner'

interface RouterContext {
  queryClient: QueryClient
}

export const Route = createRootRouteWithContext<RouterContext>()({
  component: function RootComponent() {
    const { queryClient } = Route.useRouteContext()
    return (
      <QueryClientProvider client={queryClient}>
        <ToastProvider>
          <AppLayout />
        </ToastProvider>
      </QueryClientProvider>
    )
  },
})
