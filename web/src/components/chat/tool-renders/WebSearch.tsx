// WebSearch render — search results with favicons
import { Globe } from 'lucide-react'
import type { ToolRenderComponent } from './registry'

export const WebSearchRender: ToolRenderComponent = ({ args, result, isRunning }) => {
  const query = (args?.query ?? args?.search_query ?? '') as string

  // Parse result — might be a string with URLs or structured data
  const results = parseResults(result)

  return (
    <div className="space-y-2 text-sm">
      {/* Query display */}
      <div className="flex items-center gap-2 text-xs">
        <Globe className="w-3.5 h-3.5 text-muted-foreground" />
        <span className="text-muted-foreground italic">
          {query.length > 80 ? query.slice(0, 80) + '...' : query}
        </span>
      </div>

      {/* Results */}
      {isRunning ? (
        <div className="flex items-center gap-2 text-muted-foreground">
          <span className="inline-block w-2 h-2 rounded-full bg-amber-500 animate-pulse" />
          Searching...
        </div>
      ) : results.length > 0 ? (
        <div className="space-y-1.5">
          {results.slice(0, 10).map((r, i) => (
            <a
              key={i}
              href={r.url}
              target="_blank"
              rel="noopener noreferrer"
              className="block px-2 py-1.5 rounded hover:bg-muted transition-colors group"
            >
              <div className="flex items-start gap-2">
                {r.favicon ? (
                  <img
                    src={r.favicon}
                    alt=""
                    className="w-4 h-4 rounded mt-0.5 shrink-0"
                    onError={(e) => {
                      (e.target as HTMLImageElement).style.display = 'none'
                    }}
                  />
                ) : (
                  <Globe className="w-4 h-4 text-muted-foreground mt-0.5 shrink-0" />
                )}
                <div className="min-w-0">
                  <div className="text-xs font-medium truncate group-hover:text-primary transition-colors">
                    {r.title || r.url}
                  </div>
                  {r.snippet && (
                    <div className="text-xs text-muted-foreground line-clamp-2 mt-0.5">
                      {r.snippet}
                    </div>
                  )}
                  <div className="text-[10px] text-muted-foreground/60 truncate mt-0.5">
                    {r.url}
                  </div>
                </div>
              </div>
            </a>
          ))}
        </div>
      ) : result != null ? (
        <pre className="p-2 rounded bg-muted text-xs overflow-x-auto max-h-48 whitespace-pre-wrap">
          {typeof result === 'string' ? result.slice(0, 3000) : JSON.stringify(result, null, 2)}
        </pre>
      ) : null}
    </div>
  )
}

// ── Helpers ──

interface ParsedResult {
  url: string
  title?: string
  snippet?: string
  favicon?: string
}

function parseResults(raw: unknown): ParsedResult[] {
  if (!raw) return []

  // Try structured array
  if (Array.isArray(raw)) {
    return raw.map((item) => {
      if (typeof item === 'string') return { url: item }
      return {
        url: item?.url ?? item?.link ?? '',
        title: item?.title,
        snippet: item?.snippet ?? item?.description,
        favicon: item?.favicon,
      }
    })
  }

  // Try string — extract URLs
  if (typeof raw === 'string') {
    const urlRegex = /https?:\/\/[^\s<>"{}|\\^`[\]]+/g
    const urls = raw.match(urlRegex)
    if (urls) return urls.map((url) => ({ url }))
  }

  return []
}
