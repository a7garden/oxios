/** Asset metadata — mirrors the backend `Asset` struct. */
export interface Asset {
  id: string
  filename: string
  title: string | null
  mime_type: string
  size_bytes: number
  source: string
  source_ref: string | null
  tags: string[]
  width: number | null
  height: number | null
  duration_secs: number | null
  sha256: string
  created_at: number
  storage_name: string
  url: string
}

export interface AssetListResponse {
  items: Asset[]
  total: number
  page: number
  limit: number
}

export type AssetType = 'image' | 'audio' | 'video' | 'document'

export type AssetSource =
  | 'upload'
  | 'editor-paste'
  | 'chat-attach'
  | 'generated'
  | 'web-search'
  | 'imported'

export interface AssetFilter {
  type?: AssetType
  source?: string
  search?: string
  page?: number
  limit?: number
}

/** Derive broad type category from MIME type. */
export function mimeTypeToCategory(mime: string): AssetType {
  if (mime.startsWith('image/')) return 'image'
  if (mime.startsWith('audio/')) return 'audio'
  if (mime.startsWith('video/')) return 'video'
  return 'document'
}

/** Human-readable file size. */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}
