// tool-renders/index.ts — registers all builtin tool renders + inspectors.
//
// Aliases: a single render often covers multiple tool names. We register
// under both Oxios kernel names (exec, knowledge) and oxi-sdk builtins
// (read_file, write_file, edit_file, glob, grep, list_files, web_search,
// web_fetch) so lookups hit regardless of which layer emitted the call.

import { BashRender } from './Bash'
import { FileEditRender } from './FileEdit'
import { FileReadRender } from './FileRead'
import { GlobRender } from './Glob'
import { GrepRender } from './Grep'
import { ListFilesRender } from './ListFiles'
import { registerToolRender } from './registry'
import { WebFetchRender } from './WebFetch'
import { WebSearchRender } from './WebSearch'

// ── File operations ──
registerToolRender('read_file', FileReadRender)
registerToolRender('readFile', FileReadRender)
registerToolRender('read', FileReadRender)

registerToolRender('write_file', FileEditRender)
registerToolRender('writeFile', FileEditRender)
registerToolRender('write', FileEditRender)

registerToolRender('edit_file', FileEditRender)
registerToolRender('editFile', FileEditRender)
registerToolRender('edit', FileEditRender)

// ── Shell ──
registerToolRender('exec', BashRender) // Oxios kernel name
registerToolRender('bash', BashRender)
registerToolRender('run_command', BashRender)
registerToolRender('shell', BashRender)

// ── Search/listing ──
registerToolRender('glob', GlobRender)
registerToolRender('find_files', GlobRender)
registerToolRender('grep', GrepRender)
registerToolRender('search_files', GrepRender)
registerToolRender('list_files', ListFilesRender)
registerToolRender('listFiles', ListFilesRender)
registerToolRender('ls', ListFilesRender)

// ── Web ──
registerToolRender('web_search', WebSearchRender)
registerToolRender('webSearch', WebSearchRender)
registerToolRender('get_search_results', WebSearchRender)
registerToolRender('web_fetch', WebFetchRender)
registerToolRender('webFetch', WebFetchRender)
registerToolRender('fetch', WebFetchRender)

export {
  DefaultToolRender,
  getToolInspector,
  getToolIntervention,
  getToolRender,
  getToolStreaming,
  registerToolInspector,
  registerToolIntervention,
  registerToolRender,
  registerToolStreaming,
} from './registry'
export type {
  ToolInspectorComponent,
  ToolInspectorProps,
  ToolInterventionComponent,
  ToolInterventionProps,
  ToolRenderComponent,
  ToolRenderProps,
  ToolStreamingComponent,
  ToolStreamingProps,
} from './registry'
