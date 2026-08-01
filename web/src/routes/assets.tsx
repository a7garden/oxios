import { createFileRoute } from '@tanstack/react-router'
import {
  Copy,
  Download,
  FileText,
  Film,
  ImageIcon,
  Music,
  Trash2,
  Upload,
  X,
} from 'lucide-react'
import { useCallback, useMemo, useRef, useState } from 'react'
import { PageHeader } from '@/components/shared/page-header'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useDeleteAsset, useAssets, useUploadAsset } from '@/hooks/use-assets'
import { cn } from '@/lib/utils'
import { formatBytes, mimeTypeToCategory, type Asset, type AssetType } from '@/types/asset'

export const Route = createFileRoute('/assets')({ component: AssetsPage })

const TYPE_FILTERS: { label: string; value: AssetType | null }[] = [
  { label: 'All', value: null },
  { label: 'Images', value: 'image' },
  { label: 'Audio', value: 'audio' },
  { label: 'Video', value: 'video' },
  { label: 'Documents', value: 'document' },
]

function AssetsPage() {
  const [typeFilter, setTypeFilter] = useState<AssetType | null>(null)
  const [search, setSearch] = useState('')
  const [page, setPage] = useState(1)
  const [selected, setSelected] = useState<Asset | null>(null)
  const [dragOver, setDragOver] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const { data, loading, error } = useAssets({
    type: typeFilter ?? undefined,
    search: search || undefined,
    page,
    limit: 24,
  })
  const { upload, uploading } = useUploadAsset()
  const { deleteAsset } = useDeleteAsset()

  // Debounce search.
  const [searchInput, setSearchInput] = useState('')
  const searchTimer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const handleSearchChange = useCallback((v: string) => {
    setSearchInput(v)
    if (searchTimer.current) clearTimeout(searchTimer.current)
    searchTimer.current = setTimeout(() => {
      setSearch(v)
      setPage(1)
    }, 300)
  }, [])

  const handleFiles = useCallback(
    async (files: FileList | File[]) => {
      const arr = Array.from(files)
      for (const f of arr) {
        await upload(f, { source: 'upload' })
      }
      // Trigger refetch by bumping page (the useAssets hook re-runs on key change)
      setPage((p) => p)
      window.location.reload()
    },
    [upload],
  )

  const handleDelete = useCallback(
    async (asset: Asset) => {
      const ok = await deleteAsset(asset.storage_name)
      if (ok) {
        setSelected(null)
        window.location.reload()
      }
    },
    [deleteAsset],
  )

  const copyUrl = useCallback((asset: Asset) => {
    const url = `${window.location.origin}/api/assets/${asset.storage_name}`
    navigator.clipboard.writeText(url)
  }, [])

  const totalPages = data ? Math.ceil(data.total / data.limit) : 1

  return (
    <div className="space-y-4">
      <PageHeader title="Assets" subtitle="Unified asset library — images, audio, video, and documents" />

      {/* Upload zone */}
      <div
        className={cn(
          'rounded-lg border-2 border-dashed p-4 text-center transition-colors',
          dragOver
            ? 'border-primary bg-primary/5'
            : 'border-muted-foreground/25 hover:border-muted-foreground/50',
        )}
        onDragOver={(e) => {
          e.preventDefault()
          setDragOver(true)
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={(e) => {
          e.preventDefault()
          setDragOver(false)
          if (e.dataTransfer.files.length) handleFiles(e.dataTransfer.files)
        }}
      >
        <input
          ref={fileInputRef}
          type="file"
          className="hidden"
          multiple
          onChange={(e) => {
            if (e.target.files) handleFiles(e.target.files)
            e.target.value = ''
          }}
        />
        <button
          type="button"
          className="flex w-full flex-col items-center gap-1 text-sm text-muted-foreground transition-colors hover:text-foreground"
          onClick={() => fileInputRef.current?.click()}
          disabled={uploading}
        >
          <Upload className="h-5 w-5" />
          {uploading ? 'Uploading...' : 'Drop files here or click to upload'}
        </button>
      </div>

      {/* Filter tabs */}
      <div className="flex flex-wrap items-center gap-2">
        <div className="flex gap-1">
          {TYPE_FILTERS.map((tf) => (
            <button
              key={tf.label}
              type="button"
              className={cn(
                'rounded-md px-3 py-1 text-sm transition-colors',
                typeFilter === tf.value
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-muted text-muted-foreground hover:bg-muted/80',
              )}
              onClick={() => {
                setTypeFilter(tf.value)
                setPage(1)
              }}
            >
              {tf.label}
            </button>
          ))}
        </div>
        <Input
          className="ml-auto max-w-xs"
          placeholder="Search assets..."
          value={searchInput}
          onChange={(e) => handleSearchChange(e.target.value)}
        />
      </div>

      {/* Grid */}
      {loading && <p className="text-sm text-muted-foreground">Loading...</p>}
      {error && <p className="text-sm text-destructive">Error: {error}</p>}
      {!loading && data && data.items.length === 0 && (
        <p className="py-12 text-center text-sm text-muted-foreground">No assets found.</p>
      )}

      {data && data.items.length > 0 && (
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-6">
          {data.items.map((asset) => (
            <AssetCard key={asset.id} asset={asset} onClick={() => setSelected(asset)} />
          ))}
        </div>
      )}

      {/* Pagination */}
      {totalPages > 1 && (
        <div className="flex items-center justify-center gap-2 pt-4">
          <Button variant="outline" size="sm" disabled={page <= 1} onClick={() => setPage(page - 1)}>
            Prev
          </Button>
          <span className="text-sm text-muted-foreground">
            {page} / {totalPages}
          </span>
          <Button
            variant="outline"
            size="sm"
            disabled={page >= totalPages}
            onClick={() => setPage(page + 1)}
          >
            Next
          </Button>
        </div>
      )}

      {/* Detail drawer */}
      {selected && (
        <AssetDetail
          asset={selected}
          onClose={() => setSelected(null)}
          onDelete={() => handleDelete(selected)}
          onCopyUrl={() => copyUrl(selected)}
        />
      )}
    </div>
  )
}

// ── Asset Card ──────────────────────────────────────────────────────

function AssetCard({ asset, onClick }: { asset: Asset; onClick: () => void }) {
  const category = mimeTypeToCategory(asset.mime_type)

  return (
    <button
      type="button"
      onClick={onClick}
      className="group relative overflow-hidden rounded-lg border bg-muted/30 transition-all hover:border-primary/50 hover:shadow-md"
    >
      <div className="flex aspect-square items-center justify-center overflow-hidden">
        {category === 'image' ? (
          <img
            src={`/api/assets/${asset.storage_name}`}
            alt={asset.filename}
            className="h-full w-full object-cover transition-transform group-hover:scale-105"
            loading="lazy"
          />
        ) : (
          <div className="flex h-full w-full flex-col items-center justify-center gap-2 text-muted-foreground">
            <CategoryIcon category={category} />
            <span className="max-w-[80%] truncate text-xs">{asset.filename}</span>
          </div>
        )}
      </div>
      <div className="p-1.5">
        <p className="truncate text-xs font-medium">{asset.title || asset.filename}</p>
        <p className="text-[10px] text-muted-foreground">{formatBytes(asset.size_bytes)}</p>
      </div>
    </button>
  )
}

function CategoryIcon({ category }: { category: AssetType }) {
  const cls = 'h-8 w-8'
  if (category === 'audio') return <Music className={cls} />
  if (category === 'video') return <Film className={cls} />
  if (category === 'document') return <FileText className={cls} />
  return <ImageIcon className={cls} />
}

// ── Detail Drawer ───────────────────────────────────────────────────

function AssetDetail({
  asset,
  onClose,
  onDelete,
  onCopyUrl,
}: {
  asset: Asset
  onClose: () => void
  onDelete: () => void
  onCopyUrl: () => void
}) {
  const [copied, setCopied] = useState(false)
  const category = mimeTypeToCategory(asset.mime_type)
  const [confirmDelete, setConfirmDelete] = useState(false)

  const handleCopy = useCallback(() => {
    onCopyUrl()
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }, [onCopyUrl])

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
      onClick={onClose}
    >
      <div
        className="relative max-h-[90vh] w-full max-w-2xl overflow-auto rounded-lg border bg-background p-6 shadow-lg"
        onClick={(e) => e.stopPropagation()}
      >
        <button
          type="button"
          className="absolute right-4 top-4 rounded-md p-1 text-muted-foreground hover:bg-muted"
          onClick={onClose}
        >
          <X className="h-4 w-4" />
        </button>

        {/* Preview */}
        <div className="mb-4 flex min-h-[200px] items-center justify-center rounded-lg bg-muted/30">
          {category === 'image' ? (
            <img
              src={`/api/assets/${asset.storage_name}`}
              alt={asset.filename}
              className="max-h-[400px] rounded-lg object-contain"
            />
          ) : category === 'audio' ? (
            <audio controls className="w-full">
              <source src={`/api/assets/${asset.storage_name}`} type={asset.mime_type} />
            </audio>
          ) : category === 'video' ? (
            <video controls className="max-h-[400px] rounded-lg">
              <source src={`/api/assets/${asset.storage_name}`} type={asset.mime_type} />
            </video>
          ) : (
            <div className="flex flex-col items-center gap-3 py-12 text-muted-foreground">
              <CategoryIcon category={category} />
              <p className="text-sm">{asset.filename}</p>
            </div>
          )}
        </div>

        {/* Metadata */}
        <div className="space-y-2 text-sm">
          <div className="flex justify-between">
            <span className="text-muted-foreground">Filename</span>
            <span className="font-medium">{asset.filename}</span>
          </div>
          {asset.title && (
            <div className="flex justify-between">
              <span className="text-muted-foreground">Title</span>
              <span className="font-medium">{asset.title}</span>
            </div>
          )}
          <div className="flex justify-between">
            <span className="text-muted-foreground">Type</span>
            <span className="font-medium">{asset.mime_type}</span>
          </div>
          <div className="flex justify-between">
            <span className="text-muted-foreground">Size</span>
            <span className="font-medium">{formatBytes(asset.size_bytes)}</span>
          </div>
          {asset.width && asset.height && (
            <div className="flex justify-between">
              <span className="text-muted-foreground">Dimensions</span>
              <span className="font-medium">
                {asset.width} × {asset.height}
              </span>
            </div>
          )}
          <div className="flex justify-between">
            <span className="text-muted-foreground">Source</span>
            <span className="font-medium">{asset.source}</span>
          </div>
          {asset.tags.length > 0 && (
            <div className="flex justify-between">
              <span className="text-muted-foreground">Tags</span>
              <div className="flex flex-wrap gap-1">
                {asset.tags.map((tag) => (
                  <span
                    key={tag}
                    className="rounded-full bg-primary/10 px-2 py-0.5 text-xs text-primary"
                  >
                    {tag}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Actions */}
        <div className="mt-4 flex gap-2">
          <Button variant="outline" size="sm" onClick={handleCopy}>
            <Copy className="mr-1 h-3 w-3" />
            {copied ? 'Copied!' : 'Copy URL'}
          </Button>
          <a href={`/api/assets/${asset.storage_name}`} download={asset.filename}>
            <Button variant="outline" size="sm">
              <Download className="mr-1 h-3 w-3" />
              Download
            </Button>
          </a>
          {confirmDelete ? (
            <>
              <Button variant="destructive" size="sm" onClick={onDelete}>
                <Trash2 className="mr-1 h-3 w-3" />
                Confirm Delete
              </Button>
              <Button variant="outline" size="sm" onClick={() => setConfirmDelete(false)}>
                Cancel
              </Button>
            </>
          ) : (
            <Button
              variant="outline"
              size="sm"
              className="ml-auto text-destructive hover:bg-destructive/10"
              onClick={() => setConfirmDelete(true)}
            >
              <Trash2 className="mr-1 h-3 w-3" />
              Delete
            </Button>
          )}
        </div>
      </div>
    </div>
  )
}
