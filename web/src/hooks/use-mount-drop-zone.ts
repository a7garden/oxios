import { useCallback, useState } from 'react'

/**
 * MIME type used to carry a Mount id via the HTML5 drag-and-drop dataTransfer.
 * Custom application/* prefix so it never collides with browser-managed types.
 */
export const MOUNT_DRAG_MIME = 'application/x-oxios-mount'

interface UseMountDropZoneOptions {
  /** Called with the dropped Mount id. Receives only the parsed id. */
  onDropMount: (id: string) => void
}

interface MountDropZone {
  /** True while a valid mount drag is hovering the zone. */
  isOver: boolean
  /** Spread onto the drop-zone element. */
  dropProps: {
    onDragOver: (e: React.DragEvent) => void
    onDragEnter: (e: React.DragEvent) => void
    onDragLeave: (e: React.DragEvent) => void
    onDrop: (e: React.DragEvent) => void
  }
}

/**
 * Encapsulates the HTML5 DnD plumbing needed by any element that accepts Mount
 * cards as drops. Returns spreadable props plus an `isOver` flag for styling.
 *
 * The hook looks for `application/x-oxios-mount` on the dataTransfer; drops that
 * don't carry it (e.g. plain file drops) are ignored silently so the page can
 * still handle them elsewhere.
 */
export function useMountDropZone({ onDropMount }: UseMountDropZoneOptions): MountDropZone {
  const [isOver, setIsOver] = useState(false)

  const hasMountPayload = (e: React.DragEvent) =>
    Array.from(e.dataTransfer.types).includes(MOUNT_DRAG_MIME)

  const onDragOver = useCallback((e: React.DragEvent) => {
    if (!hasMountPayload(e)) return
    e.preventDefault()
    e.dataTransfer.dropEffect = 'copy'
    setIsOver(true)
  }, [])

  const onDragEnter = useCallback((e: React.DragEvent) => {
    if (!hasMountPayload(e)) return
    e.preventDefault()
    setIsOver(true)
  }, [])

  const onDragLeave = useCallback((e: React.DragEvent) => {
    // Only clear when the cursor truly leaves the zone (not when crossing a child).
    if (e.currentTarget === e.target) setIsOver(false)
  }, [])

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      if (!hasMountPayload(e)) return
      e.preventDefault()
      const id = e.dataTransfer.getData(MOUNT_DRAG_MIME)
      setIsOver(false)
      if (id) onDropMount(id)
    },
    [onDropMount],
  )

  return { isOver, dropProps: { onDragOver, onDragEnter, onDragLeave, onDrop } }
}