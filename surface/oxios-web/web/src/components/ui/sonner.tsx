import { X } from 'lucide-react'
import * as React from 'react'
import { cn } from '@/lib/utils'

type ToastVariant = 'default' | 'success' | 'destructive'

interface Toast {
  id: string
  message: string
  variant: ToastVariant
}

const ToastContext = React.createContext<{
  toast: (message: string, variant?: ToastVariant) => void
}>({ toast: () => {} })

export function useToast() {
  return React.useContext(ToastContext)
}

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = React.useState<Toast[]>([])

  const addToast = React.useCallback((message: string, variant: ToastVariant = 'default') => {
    const id = Math.random().toString(36).slice(2)
    setToasts((prev) => [...prev, { id, message, variant }])
    setTimeout(() => {
      setToasts((prev) => prev.filter((t) => t.id !== id))
    }, 4000)
  }, [])

  const removeToast = React.useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id))
  }, [])

  React.useEffect(() => {
    const handler = (e: Event) => {
      const { message } = (e as CustomEvent<{ message: string }>).detail
      addToast(message, 'destructive')
    }
    window.addEventListener('oxios:mutation-error', handler)
    return () => window.removeEventListener('oxios:mutation-error', handler)
  }, [addToast])

  return (
    <ToastContext.Provider value={{ toast: addToast }}>
      {children}
      <div className="fixed bottom-4 right-4 z-[100] flex flex-col gap-2">
        {toasts.map((t) => (
          <div
            key={t.id}
            className={cn(
              'flex items-center gap-2 rounded-lg border px-4 py-3 text-sm shadow-lg min-w-[280px] max-w-[420px]',
              t.variant === 'destructive' &&
                'border-destructive bg-destructive text-destructive-foreground',
              t.variant === 'success' &&
                'border-success/50 bg-success-muted text-success',
              t.variant === 'default' && 'border-border bg-background text-foreground',
            )}
          >
            <span className="flex-1">{t.message}</span>
            <button
              type="button"
              onClick={() => removeToast(t.id)}
              className="shrink-0 rounded-md p-0.5 opacity-70 hover:opacity-100"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  )
}
