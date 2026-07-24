// React renderer — compiles JSX/TSX in-browser via Sandpack (CodeSandbox).
//
// This component is lazy-loaded by the dispatcher, so the heavy
// @codesandbox/sandpack-react dependency lands in a separate chunk fetched only
// when a React artifact is opened. The model's code is wrapped into a complete
// Vite + React project by react-template.ts.

import { SandpackLayout, SandpackPreview, SandpackProvider } from '@codesandbox/sandpack-react'
import { useTheme } from 'next-themes'
import { memo, useMemo } from 'react'
import { buildReactArtifactProject } from '../react-template'

interface ReactRendererProps {
  content: string
  title?: string
}

function ReactRendererImpl({ content, title }: ReactRendererProps) {
  const { resolvedTheme } = useTheme()
  const project = useMemo(() => buildReactArtifactProject(content, title), [content, title])

  return (
    <SandpackProvider
      template="react-ts"
      files={project.files}
      customSetup={{ dependencies: project.dependencies }}
      theme={resolvedTheme === 'dark' ? 'dark' : 'light'}
      options={{ externalResources: [...project.externalResources] }}
      style={{ height: '100%' }}
    >
      <SandpackLayout style={{ height: '100%' }}>
        <SandpackPreview style={{ height: '100%' }} />
      </SandpackLayout>
    </SandpackProvider>
  )
}

export default memo(ReactRendererImpl)
