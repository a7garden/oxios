import {
  CircleCheckIcon,
  InfoIcon,
  Loader2Icon,
  OctagonXIcon,
  TriangleAlertIcon,
} from "lucide-react"
import { useEffect } from "react"
import { toast, Toaster as Sonner, type ToasterProps } from "sonner"

function useMutationErrorListener() {
  useEffect(() => {
    const handler = (e: Event) => {
      const { message } = (e as CustomEvent<{ message: string }>).detail
      toast.error(message || "Unknown error")
    }
    window.addEventListener("oxios:mutation-error", handler)
    return () => window.removeEventListener("oxios:mutation-error", handler)
  }, [])
}

const Toaster = ({ ...props }: ToasterProps) => {
  useMutationErrorListener()

  return (
    <Sonner
      theme="system"
      className="toaster group"
      icons={{
        success: <CircleCheckIcon className="size-4" />,
        info: <InfoIcon className="size-4" />,
        warning: <TriangleAlertIcon className="size-4" />,
        error: <OctagonXIcon className="size-4" />,
        loading: <Loader2Icon className="size-4 animate-spin" />,
      }}
      style={
        {
          "--normal-bg": "var(--popover)",
          "--normal-text": "var(--popover-foreground)",
          "--normal-border": "var(--border)",
          "--border-radius": "var(--radius)",
        } as React.CSSProperties
      }
      {...props}
    />
  )
}

export { Toaster, toast }
