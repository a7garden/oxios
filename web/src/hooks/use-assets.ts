import { useCallback, useEffect, useState } from 'react'
import { api } from '@/lib/api-client'
import type { Asset, AssetFilter, AssetListResponse } from '@/types/asset'

/** List assets with optional filters. */
export function useAssets(filter: AssetFilter = {}) {
  const [data, setData] = useState<AssetListResponse | null>(null)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const params: Record<string, string> = {}
  if (filter.type) params.type = filter.type
  if (filter.source) params.source = filter.source
  if (filter.search) params.search = filter.search
  if (filter.page) params.page = String(filter.page)
  if (filter.limit) params.limit = String(filter.limit)

  // Stringify for deps — stable unless values change.
  const key = JSON.stringify(params)

  useEffect(() => {
    let cancelled = false
    setLoading(true)
    api
      .get<AssetListResponse>('/api/assets', params)
      .then((res) => {
        if (!cancelled) {
          setData(res)
          setError(null)
        }
      })
      .catch((e) => {
        if (!cancelled) setError(String(e.message || e))
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key])

  return { data, loading, error }
}

/** Upload an asset via multipart form. Returns the created asset. */
export function useUploadAsset() {
  const [uploading, setUploading] = useState(false)

  const upload = useCallback(
    async (
      file: File,
      opts?: { source?: string; title?: string; sourceRef?: string },
    ): Promise<Asset | null> => {
      setUploading(true)
      try {
        const fd = new FormData()
        fd.append('file', file)
        if (opts?.source) fd.append('source', opts.source)
        if (opts?.title) fd.append('title', opts.title)
        if (opts?.sourceRef) fd.append('source_ref', opts.sourceRef)
        return await api.upload<Asset>('/api/assets', fd)
      } catch (e) {
        console.error('Asset upload failed:', e)
        return null
      } finally {
        setUploading(false)
      }
    },
    [],
  )

  return { upload, uploading }
}

/** Delete an asset by storage name. */
export function useDeleteAsset() {
  const [deleting, setDeleting] = useState(false)

  const deleteAsset = useCallback(async (storageName: string): Promise<boolean> => {
    setDeleting(true)
    try {
      await api.delete(`/api/assets/${encodeURIComponent(storageName)}`)
      return true
    } catch (e) {
      console.error('Asset delete failed:', e)
      return false
    } finally {
      setDeleting(false)
    }
  }, [])

  return { deleteAsset, deleting }
}

/** Update asset metadata (title, tags). */
export function useUpdateAssetMeta() {
  const [saving, setSaving] = useState(false)

  const updateMeta = useCallback(
    async (
      storageName: string,
      body: { title?: string; tags?: string[] },
    ): Promise<Asset | null> => {
      setSaving(true)
      try {
        return await api.put<Asset>(`/api/assets/${encodeURIComponent(storageName)}/meta`, body)
      } catch (e) {
        console.error('Asset meta update failed:', e)
        return null
      } finally {
        setSaving(false)
      }
    },
    [],
  )

  return { updateMeta, saving }
}
