// HyperMD setup — import all needed CM5/HyperMD modules.
// This file MUST be imported before creating an editor.
//
// Vite 8 (Rolldown) processes HyperMD's UMD as CJS, so `window.HyperMD`
// is never populated. We therefore hardcode the essential suggestedEditorConfig
// that `HyperMD.fromTextArea` would normally merge in.

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

// Import HyperMD as a single bundle (registers all addons & CodeMirror options)
import 'hypermd/everything'
import 'hypermd/theme/hypermd-light.css'
import './hypermd-dark.css'

// CodeMirror addons
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
 * Replicates what `HyperMD.fromTextArea` does internally: merges the
 * suggestedEditorConfig with user overrides, then calls
 * `CodeMirror.fromTextArea`. The suggested config is hardcoded here
 * because Vite/Rolldown processes HyperMD's UMD as CJS, meaning
 * `window.HyperMD` (and its `suggestedEditorConfig`) is never populated.
 *
 * Key WYSIWYG enablers from suggestedEditorConfig:
 *  - theme: "hypermd-light"    → WYSIWYG CSS styles
 *  - lineWrapping: true         → soft line wrap for prose
 *  - hmdFold: { code, emoji, image, link, math } → fold markers into rendered views
 *  - hmdHideToken: true         → hide `**`, `#`, `[]` on non-active lines
 *  - hmdTableAlign: true        → visual table alignment
 *  - hmdClick / hmdHover        → clickable links, hover previews
 */
export function createHyperMDEditor(
  textarea: HTMLTextAreaElement,
  userConfig?: Record<string, unknown>,
): CodeMirror.Editor {
  // These are the WYSIWYG defaults that `HyperMD.fromTextArea` normally merges.
  // See: hypermd/core/quick.js + each addon's registerFolder / suggestedOption.
  const suggestedConfig = {
    theme: 'hypermd-light',
    lineWrapping: true,
    mode: 'text/x-hypermd',
    tabSize: 4,
    hmdFold: {
      code: true,
      emoji: true,
      image: true,
      link: true,
      math: false, // no KaTeX loaded
      html: false,
    },
    hmdHideToken: { enabled: true },
    hmdClick: { enabled: true },
    hmdHover: { enabled: true },
    hmdTableAlign: { enabled: true },
    hmdCursorDebounce: { enabled: true },
  }

  const finalConfig = { ...suggestedConfig, ...userConfig }
  const CM = (window as any).CodeMirror as typeof import('codemirror')
  return CM.fromTextArea(textarea, finalConfig) as CodeMirror.Editor
}
