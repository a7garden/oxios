// HyperMD setup — import all needed CM5/HyperMD modules.
// This file MUST be imported before calling HyperMD.fromTextArea().
//
// IMPORTANT: Vite 8 uses Rolldown which can't resolve hypermd/* deep imports
// during production build. We use a single bundled entry point instead.
//
// Vite 8 (Rolldown) may fail to create a proper default export from
// codemirror's CJS `module.exports = factory()`. We import both ways
// and fall back to `window.CodeMirror` set by codemirror's own IIFE.

import 'codemirror/lib/codemirror.css'
import CodeMirrorDefault from 'codemirror'
import * as CodeMirrorNS from 'codemirror'

// Resolve the actual CodeMirror constructor from whichever import strategy worked
const _resolveCodeMirror = (): any => {
  if (typeof window !== 'undefined' && (window as any).CodeMirror) {
    return (window as any).CodeMirror
  }
  if (CodeMirrorDefault && typeof CodeMirrorDefault.fromTextArea === 'function') {
    return CodeMirrorDefault
  }
  if ((CodeMirrorNS as any).default && typeof (CodeMirrorNS as any).default.fromTextArea === 'function') {
    return (CodeMirrorNS as any).default
  }
  // Namespace itself might be the constructor
  if (typeof (CodeMirrorNS as any).fromTextArea === 'function') {
    return CodeMirrorNS
  }
  return undefined
}

const CodeMirror = _resolveCodeMirror()

if (typeof window !== 'undefined') {
  ;(window as any).CodeMirror = CodeMirror
}

// Import HyperMD as a single bundle (includes fold, click, read-link, etc.)
import 'hypermd/everything'
import 'hypermd/theme/hypermd-light.css'

// CodeMirror addons (these resolve fine)
import 'codemirror/addon/edit/continuelist'
import 'codemirror/addon/selection/active-line'
import 'codemirror/addon/hint/show-hint'
import 'codemirror/addon/hint/show-hint.css'

// Code block syntax highlighting
import 'codemirror/mode/javascript/javascript'
import 'codemirror/mode/python/python'
import 'codemirror/mode/go/go'
import 'codemirror/mode/shell/shell'
import 'codemirror/mode/php/php'
import 'codemirror/mode/markdown/markdown'

// Re-export CodeMirror type for convenience
export type CodeMirrorEditor = import('codemirror').Editor
export type CodeMirrorEditorConfiguration = import('codemirror').EditorConfiguration

/**
 * Create a HyperMD WYSIWYG editor from a textarea element.
 *
 * Uses `HyperMD.fromTextArea` from the core module, which automatically
 * merges `suggestedEditorConfig` (theme, hmdFold, lineWrapping, etc.)
 * with the user-provided config. This is what makes the editor render
 * markdown live — headers fold, links render, images display inline, etc.
 */
export function createHyperMDEditor(
  textarea: HTMLTextAreaElement,
  userConfig?: Record<string, unknown>,
): CodeMirror.Editor {
  // HyperMD.fromTextArea is available on window after the `hypermd/everything` import.
  const HMD = (window as any).HyperMD
  if (HMD?.fromTextArea) {
    return HMD.fromTextArea(textarea, userConfig) as CodeMirror.Editor
  }

  // Fallback: manual merge of suggestedEditorConfig (shouldn't normally happen)
  const suggested = HMD?.suggestedEditorConfig ?? {}
  const finalConfig = { ...suggested, ...userConfig }
  const CM = (window as any).CodeMirror as typeof CodeMirror
  return CM.fromTextArea(textarea, finalConfig) as CodeMirror.Editor
}
