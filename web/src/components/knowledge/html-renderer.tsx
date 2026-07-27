/**
 * HtmlRenderer — read-only HTML preview for `.html` knowledge files.
 *
 * Renders via sandboxed iframe srcdoc. Scripts, forms, and same-origin
 * access are blocked. The only allowed sandbox permission is `allow-popups`
 * so links in the document can open in a new tab.
 *
 * HTML files are not editable through the UI — they enter the knowledge
 * base when an AI agent writes them or the user places them on disk.
 */

interface HtmlRendererProps {
  filePath: string
  content: string
}

export function HtmlRenderer({ filePath, content }: HtmlRendererProps) {
  return (
    <div className="h-full w-full">
      <iframe
        sandbox="allow-popups"
        className="w-full h-full border-0"
        title={`${filePath} preview`}
        srcDoc={content}
      />
    </div>
  )
}
