// HyperMD setup — import all needed CM5/HyperMD modules.
// This file MUST be imported before calling HyperMD.fromTextArea().
//
// We import side-effect-only modules to register them with CodeMirror.
// Tree-shaking is not applicable for CM5's plugin architecture.

// Core CodeMirror + CSS
import 'codemirror/lib/codemirror.css'
import 'codemirror'

// HyperMD theme + mode
import 'hypermd/theme/hypermd-light.css'
import 'hypermd/mode/hypermd'

// Editor addons
import 'codemirror/addon/edit/continuelist'
import 'codemirror/addon/selection/active-line'

// HyperMD core addons
import 'hypermd/fold'
import 'hypermd/fold-link'
import 'hypermd/fold-image'
import 'hypermd/fold-emoji'
import 'hypermd/click'
import 'hypermd/read-link'
import 'hypermd/core'

// Hint (autocomplete)
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
