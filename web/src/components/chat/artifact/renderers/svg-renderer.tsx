// SVG renderer — sanitized inline render with PNG copy + SVG download.
//
// SECURITY: raw SVG executes scripts (<script>, onload, javascript: URIs).
// DOMPurify with the svg/svgFilters profiles strips those before inline render.

import DOMPurify from 'dompurify'
import { Copy, Download } from 'lucide-react'
import { memo, useCallback, useMemo, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'

interface SvgRendererProps {
  content: string
}

export const SvgRenderer = memo(function SvgRenderer({ content }: SvgRendererProps) {
  const { t } = useTranslation()
  const hostRef = useRef<HTMLDivElement>(null)

  // Strip scripts, on* handlers, javascript:/data: URIs, external refs.
  const clean = useMemo(
    () => DOMPurify.sanitize(content, { USE_PROFILES: { svg: true, svgFilters: true } }),
    [content],
  )

  const copyPng = useCallback(async () => {
    const svg = hostRef.current?.querySelector('svg')
    if (!svg) return
    const dataUrl = svgToDataUrl(svg)
    try {
      const blob = await imageUrlToBlob(dataUrl)
      await navigator.clipboard.write([new ClipboardItem({ [blob.type]: blob })])
    } catch {
      // Fallback: open in a new tab.
      window.open(dataUrl, '_blank')
    }
  }, [])

  const downloadSvg = useCallback(() => {
    const svg = hostRef.current?.querySelector('svg')
    if (!svg) return
    const blob = new Blob([new XMLSerializer().serializeToString(svg)], {
      type: 'image/svg+xml',
    })
    triggerDownload(blob, 'artifact.svg')
  }, [])

  return (
    <div className="relative flex h-full w-full items-center justify-center overflow-auto p-4">
      {/* eslint-disable-next-line react/no-danger -- DOMPurify-sanitized SVG */}
      <div
        ref={hostRef}
        className="max-h-full max-w-full"
        dangerouslySetInnerHTML={{ __html: clean }}
      />
      <div className="absolute bottom-2 right-2 flex gap-1">
        <Button size="sm" variant="secondary" className="h-7 gap-1 px-2 text-xs" onClick={copyPng}>
          <Copy className="size-3" />
          {t('artifact.copyPng')}
        </Button>
        <Button
          size="sm"
          variant="secondary"
          className="h-7 gap-1 px-2 text-xs"
          onClick={downloadSvg}
        >
          <Download className="size-3" />
          SVG
        </Button>
      </div>
    </div>
  )
})

function svgToDataUrl(svg: SVGSVGElement): string {
  const xml = new XMLSerializer().serializeToString(svg)
  const blob = new Blob([xml], { type: 'image/svg+xml;charset=utf-8' })
  return URL.createObjectURL(blob)
}

async function imageUrlToBlob(url: string): Promise<Blob> {
  const img = await loadHtmlImage(url)
  const canvas = document.createElement('canvas')
  canvas.width = img.naturalWidth || img.width || 600
  canvas.height = img.naturalHeight || img.height || 400
  const ctx = canvas.getContext('2d')
  if (!ctx) throw new Error('no 2d context')
  ctx.drawImage(img, 0, 0)
  URL.revokeObjectURL(url)
  return canvasToPngBlob(canvas)
}

function loadHtmlImage(url: string): Promise<HTMLImageElement> {
  const { promise, resolve, reject } = Promise.withResolvers<HTMLImageElement>()
  const el = new Image()
  el.crossOrigin = 'anonymous'
  el.onload = () => resolve(el)
  el.onerror = reject
  el.src = url
  return promise
}

function canvasToPngBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  const { promise, resolve, reject } = Promise.withResolvers<Blob>()
  canvas.toBlob((b) => (b ? resolve(b) : reject(new Error('toBlob failed'))), 'image/png')
  return promise
}

function triggerDownload(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}
