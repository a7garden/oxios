// HyperMD setup — import all needed CM5/HyperMD modules.
// This file MUST be imported before calling HyperMD.fromTextArea().
//
// IMPORTANT: Vite 8 uses Rolldown which can't resolve hypermd/* deep imports
// during production build. We use a single bundled entry point instead.

import 'codemirror/lib/codemirror.css'
import CodeMirror from 'codemirror'

// Ensure CodeMirror is available globally — Vite 8 (Rolldown) tree-shakes
// the side-effect assignment inside codemirror's main entry.
if (typeof window !== 'undefined') {
  ;(window as any).CodeMirror = (window as any).CodeMirror ?? CodeMirror
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
