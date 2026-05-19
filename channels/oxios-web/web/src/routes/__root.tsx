import { createRootRouteWithContext, Outlet } from '@tanstack/react-router'
import { AppLayout } from '@/components/layout/app-layout'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30000,
      retry: 1,
    },
  },
})

interface RouterContext {
  queryClient: QueryClient
}

export const Route = createRootRouteWithContext<RouterContext>()({
  component: () => (
    <QueryClientProvider client={queryClient}>
      <AppLayout />
    </QueryClientProvider>
  ),
  context: { queryClient },
})
