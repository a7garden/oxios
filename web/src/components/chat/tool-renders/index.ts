// Tool renders barrel — imports and registers all custom tool renders at module init
import { BashRender } from './Bash'
import { FileEditRender } from './FileEdit'
import { FileReadRender } from './FileRead'
import { registerToolRender } from './registry'
import { WebSearchRender } from './WebSearch'

// Register built-in renders. Modules that import this file will
// have the registry populated before any ToolCallCard renders.
registerToolRender('read', FileReadRender)
registerToolRender('write', FileEditRender)
registerToolRender('edit', FileEditRender)
registerToolRender('bash', BashRender)
registerToolRender('exec', BashRender) // Oxios exec tool aliases to bash render
registerToolRender('web_search', WebSearchRender)
registerToolRender('get_search_results', WebSearchRender)

export { registerToolRender, getToolRender, DefaultToolRender } from './registry'
export type { ToolRenderComponent, ToolRenderProps } from './registry'
