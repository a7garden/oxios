// Component test: MarkdownMessage renders an ArtifactCard for a renderable
// code block, clicking it opens the panel, and the HTML renderer mounts a
// sandboxed iframe (allow-scripts, WITHOUT allow-same-origin) whose srcdoc
// carries the artifact code.

import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { MarkdownMessage } from '@/components/chat/markdown-message'
import { PortalPanel } from '@/components/portal/portal-panel'
import { usePortalStore } from '@/stores/portal'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key, i18n: { language: 'en' } }),
}))

const HTML_ARTIFACT = '```html\n<h1>Hello</h1>\n<script>alert(1)</script>\n```'

function renderChat(md: string) {
  return render(
    <>
      <MarkdownMessage messageId="m1">{md}</MarkdownMessage>
      <PortalPanel />
    </>,
  )
}

describe('artifact card → panel wiring', () => {
  beforeEach(() => {
    usePortalStore.setState({ stack: [] })
  })

  it('renders an artifact card for a renderable code block', () => {
    renderChat(HTML_ARTIFACT)
    // The card is a button with the open-preview aria label.
    expect(screen.getByRole('button', { name: 'artifact.openPreview' })).toBeTruthy()
  })

  it('opens the panel with a sandboxed iframe on click', () => {
    renderChat(HTML_ARTIFACT)
    fireEvent.click(screen.getByRole('button', { name: 'artifact.openPreview' }))

    const iframe = document.querySelector(
      'iframe[title="artifact-html-preview"]',
    ) as HTMLIFrameElement
    expect(iframe).toBeTruthy()

    // Security: allow-scripts present, allow-same-origin ABSENT.
    const sandbox = iframe.getAttribute('sandbox') ?? ''
    expect(sandbox).toContain('allow-scripts')
    expect(sandbox).not.toContain('allow-same-origin')

    // The artifact code (including <script>) is carried in srcdoc as a string.
    const srcdoc = iframe.getAttribute('srcdoc') ?? ''
    expect(srcdoc).toContain('<h1>Hello</h1>')
    expect(srcdoc).toContain('alert(1)')
  })

  it('does NOT render a card for non-renderable languages', () => {
    renderChat('```bash\necho hi\n```')
    expect(screen.queryByRole('button', { name: 'artifact.openPreview' })).toBeNull()
  })
})
