import { createFileRoute, Navigate } from '@tanstack/react-router'

export const Route = createFileRoute('/marketplace')({
  component: () => <Navigate to="/skills" search={{ tab: 'marketplace' }} />,
})
