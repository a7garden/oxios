export interface FileContent {
  path: string
  content: string
  mimeType: string
  size: number
}

export interface CreateFileRequest {
  is_dir: boolean
}

export interface FileAction {
  type: 'create-file' | 'create-dir' | 'delete' | 'rename'
  path: string
  newPath?: string
}

export type EditorMode = 'view' | 'edit'

export const EDITABLE_EXTENSIONS = new Set([
  'rs', 'ts', 'tsx', 'js', 'jsx', 'py', 'go', 'toml', 'json', 'yaml', 'yml',
  'md', 'txt', 'log', 'env', 'sh', 'bash', 'css', 'html', 'xml', 'sql',
])

export const IMAGE_EXTENSIONS = new Set(['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'ico'])

export function getFileExtension(path: string): string {
  const parts = path.split('.')
  return parts.length > 1 ? parts[parts.length - 1]!.toLowerCase() : ''
}

export function isEditable(path: string): boolean {
  return EDITABLE_EXTENSIONS.has(getFileExtension(path))
}

export function isImage(path: string): boolean {
  return IMAGE_EXTENSIONS.has(getFileExtension(path))
}
