import type { Extension } from '@codemirror/state'
import { StreamLanguage } from '@codemirror/language'
import { json } from '@codemirror/lang-json'
import { rust } from '@codemirror/lang-rust'
import { markdown } from '@codemirror/lang-markdown'
import { python } from '@codemirror/lang-python'
import { yaml } from '@codemirror/lang-yaml'
import { javascript } from '@codemirror/lang-javascript'
import { toml } from '@codemirror/legacy-modes/mode/toml'
import { shell } from '@codemirror/legacy-modes/mode/shell'

/**
 * Pick the right CM6 language extension based on file path extension.
 */
export function getLanguageExtension(path: string): Extension | null {
  const ext = path.split('.').pop()?.toLowerCase() ?? ''
  switch (ext) {
    case 'rs':
      return rust()
    case 'json':
      return json()
    case 'md':
    case 'markdown':
      return markdown()
    case 'py':
      return python()
    case 'yaml':
    case 'yml':
      return yaml()
    case 'ts':
    case 'tsx':
    case 'js':
    case 'jsx':
      return javascript({ typescript: ext.startsWith('t') })
    case 'toml':
      return StreamLanguage.define(toml)
    case 'sh':
    case 'bash':
      return StreamLanguage.define(shell)
    default:
      return null
  }
}
