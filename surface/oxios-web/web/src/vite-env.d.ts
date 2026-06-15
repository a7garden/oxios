/// <reference types="vite/client" />

// RFC-025: drag-to-reparent session transfer. Set on dragstart, read on drop.
interface Window {
  __draggedSessionId?: string
}
