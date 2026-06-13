import { Component, type ReactNode } from 'react'
import i18n from '@/i18n'

interface Props {
  children: ReactNode
  fallback?: ReactNode
}

interface State {
  hasError: boolean
  error?: Error
  retryCount: number
}

const MAX_RETRIES = 2

/**
 * Detects module-import failures (stale deploy, network blip, Vite HMR
 * transient error). Safari reports "Importing a module script failed",
 * Chrome "Failed to fetch module", Firefox "error resolving module specifier".
 */
function isModuleImportError(error: Error): boolean {
  const msg = error.message ?? ''
  const name = error.name ?? ''
  return (
    msg.includes('Importing a module script failed') ||
    msg.includes('Failed to fetch dynamically imported module') ||
    msg.includes('error resolving module specifier') ||
    msg.includes('Unable to resolve specifier') ||
    name === 'TypeError'
  )
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props)
    this.state = { hasError: false, retryCount: 0 }
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error, retryCount: 0 }
  }

  private handleRetry = () => {
    const { error, retryCount } = this.state
    // For module-import errors, auto-reload after retries exhausted
    if (error && isModuleImportError(error) && retryCount >= MAX_RETRIES) {
      window.location.reload()
      return
    }
    this.setState((prev) => ({ hasError: false, retryCount: prev.retryCount + 1 }))
  }

  private handleReload = () => {
    window.location.reload()
  }

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) return this.props.fallback

      const { error, retryCount } = this.state
      const isModuleError = error ? isModuleImportError(error) : false
      const isChunkError =
        isModuleError ||
        (error?.message?.includes('chunk') ?? false) ||
        (error?.message?.includes('Lazy') ?? false)

      return (
        <div className="flex items-center justify-center p-8">
          <div className="text-center max-w-md">
            <h2 className="text-lg font-semibold text-destructive">
              {isChunkError
                ? i18n.t('errorBoundary.chunkError', '페이지를 불러오지 못했습니다')
                : i18n.t('errorBoundary.genericError', '문제가 발생했습니다')}
            </h2>
            <p className="mt-2 text-sm text-muted-foreground">
              {isChunkError
                ? i18n.t(
                    'errorBoundary.chunkErrorDesc',
                    '웹 UI 파일을 최신 상태로 업데이트 중입니다. 잠시 후 다시 시도해주세요.',
                  )
                : (error?.message ?? i18n.t('errorBoundary.unknownError', '알 수 없는 오류'))}
            </p>
            <div className="mt-4 flex items-center justify-center gap-3">
              <button
                type="button"
                className="rounded-md bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90 transition-colors"
                onClick={this.handleRetry}
              >
                {retryCount >= MAX_RETRIES
                  ? i18n.t('common.refresh', '새로고침')
                  : i18n.t('common.retry', '다시 시도')}
              </button>
              {!isChunkError && (
                <button
                  type="button"
                  className="rounded-md border px-4 py-2 text-sm text-foreground hover:bg-accent transition-colors"
                  onClick={this.handleReload}
                >
                  {i18n.t('common.refresh', '새로고침')}
                </button>
              )}
            </div>
            {import.meta.env.DEV && error && (
              <details className="mt-4 text-left">
                <summary className="text-xs text-muted-foreground cursor-pointer">
                  {i18n.t('errorBoundary.errorDetails', '오류 상세 (개발 모드)')}
                </summary>
                <pre className="mt-2 overflow-auto rounded-md bg-muted p-3 text-xs">
                  {error.stack ?? error.message}
                </pre>
              </details>
            )}
          </div>
        </div>
      )
    }
    return this.props.children
  }
}
