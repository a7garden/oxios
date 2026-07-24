// React artifact template — ported from @lobechat/artifact-template.
//
// Wraps model-generated React/JSX/TSX into a complete Vite + React + TS project
// that Sandpack can compile in-browser. Keeps a broad dependency set so typical
// LLM output (shadcn-style components, recharts, lucide icons, motion, etc.)
// resolves without per-artifact setup.

export interface ReactArtifactProject {
  dependencies: Record<string, string>
  /** CDN scripts injected into the preview HTML (Tailwind play CDN). */
  externalResources: string[]
  /** Sandpack file map (absolute paths). */
  files: Record<string, { code: string; hidden?: boolean }>
}

const TAILWIND_CDN = 'https://cdn.tailwindcss.com'

// Broad but stable set. lucide-react pinned to 0.x (LLM training data still
// emits 0.x icon names; 1.x renamed/dropped several).
const DEFAULT_DEPENDENCIES: Record<string, string> = {
  '@radix-ui/react-accordion': 'latest',
  '@radix-ui/react-avatar': 'latest',
  'class-variance-authority': 'latest',
  '@radix-ui/react-dialog': 'latest',
  '@radix-ui/react-dropdown-menu': 'latest',
  '@radix-ui/react-label': 'latest',
  '@radix-ui/react-popover': 'latest',
  '@radix-ui/react-progress': 'latest',
  '@radix-ui/react-scroll-area': 'latest',
  '@radix-ui/react-select': 'latest',
  '@radix-ui/react-separator': 'latest',
  '@radix-ui/react-slot': 'latest',
  '@radix-ui/react-switch': 'latest',
  '@radix-ui/react-tabs': 'latest',
  '@radix-ui/react-toast': 'latest',
  '@radix-ui/react-tooltip': 'latest',
  classvarianceAuthority: 'latest',
  clsx: 'latest',
  'date-fns': 'latest',
  'lucide-react': '^0.544.0',
  motion: 'latest',
  react: '19.2.7',
  'react-dom': '19.2.7',
  recharts: 'latest',
  sonner: 'latest',
  'tailwind-merge': 'latest',
}

const escapeHtml = (v: string) =>
  v.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;')

const indexHtml = (title: string) => `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>${escapeHtml(title)}</title>
    <script src="${TAILWIND_CDN}"></script>
  </head>
  <body>
    <div id="root"></div>
  </body>
</html>
`

// Bootstrap: mount the user's default-exported App.
const entry = `import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import App from './App';

const container = document.getElementById('root');
if (!container) throw new Error('Root container #root not found');

createRoot(container).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
`

/**
 * Wrap raw user code so it always default-exports a component, whether the
 * model wrote a full component definition, a JSX fragment, or just markup.
 */
function wrapAppCode(code: string): string {
  const trimmed = code.trim()

  // Already declares a default export — keep as-is.
  if (/export\s+default\s+/m.test(trimmed)) return trimmed

  // Has a named function/const component — re-export as default.
  const namedFn = trimmed.match(/(?:function|const)\s+([A-Z]\w*)\s*[=(]/)
  if (namedFn) return `${trimmed}\n\nexport default ${namedFn[1]};\n`

  // Bare JSX/element — wrap in an inline component.
  return `export default function App() {\n  return (\n${trimmed}\n  );\n}\n`
}

/**
 * Build the Sandpack file map for a React artifact.
 */
export function buildReactArtifactProject(
  appCode: string,
  title = 'Artifact',
): ReactArtifactProject {
  return {
    dependencies: DEFAULT_DEPENDENCIES,
    externalResources: [TAILWIND_CDN],
    files: {
      '/App.tsx': { code: wrapAppCode(appCode) },
      '/index.tsx': { code: entry, hidden: true },
      '/index.html': { code: indexHtml(title), hidden: true },
    },
  }
}
