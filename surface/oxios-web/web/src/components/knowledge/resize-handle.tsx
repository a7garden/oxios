import { useCallback, useRef } from 'react'

interface ResizeHandleProps {
  width: number
  onResize: (width: number) => void
}

export function ResizeHandle({ onResize }: ResizeHandleProps) {
  const isDragging = useRef(false)

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault()
      isDragging.current = true
      document.body.style.cursor = 'col-resize'
      document.body.style.userSelect = 'none'

      const handleMouseMove = (e: MouseEvent) => {
        if (!isDragging.current) return
        onResize(e.clientX)
      }

      const handleMouseUp = () => {
        isDragging.current = false
        document.body.style.cursor = ''
        document.body.style.userSelect = ''
        document.removeEventListener('mousemove', handleMouseMove)
        document.removeEventListener('mouseup', handleMouseUp)
      }

      document.addEventListener('mousemove', handleMouseMove)
      document.addEventListener('mouseup', handleMouseUp)
    },
    [onResize],
  )

  // B6: hidden on touch/mobile devices — resize is a desktop interaction
  return (
    <div
      className="h-1 cursor-col-resize hover:bg-primary/20 active:bg-primary/40 transition-colors hidden lg:block"
      onMouseDown={handleMouseDown}
    />
  )
}
