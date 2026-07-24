// preprocessArtifacts — tag-protocol extraction at the STRING level, before
// any markdown parsing.
//
// WHY STRING-LEVEL (not a rehype plugin):
// CommonMark tokenises raw HTML into multiple `raw` nodes (splitting the
// artifact's inner code across nodes), and HTML parsers lowercase tag names.
// Both make a rehype-plugin extraction fragile. By scanning the raw string we
// capture the inner code as a plain substring — regardless of HTML content —
// and rewrite the <lobeArtifact> tag into a normal fenced code block. The code
// then lives as text inside <code>, which rehype-sanitize never strips, so it
// flows through the SAME language-detection path as ```svg blocks with zero
// extra security surface.
//
// Two passes:
//   1. closed artifacts:  <lobeArtifact …>CODE</lobeArtifact>
//   2. unclosed (streaming): <lobeArtifact …>CODE  (captures to end of input)
//
// LobeHub analogue: Conversation/Markdown/plugins/LobeArtifact/rehypePlugin.ts.

import { artifactTypeToLanguage, tagTypeToArtifactType } from '@/types/artifact'

const OPEN_ATTRS = '([^>]*)'

/** Closed artifact: opening tag … closing tag. */
const CLOSED_RE = new RegExp(`<lobeArtifact\\b${OPEN_ATTRS}>([\\s\\S]*?)<\\/lobeArtifact>`, 'gi')

/** Unclosed artifact (streaming): opening tag … end of input. */
const UNCLOSED_RE = new RegExp(`<lobeArtifact\\b${OPEN_ATTRS}>([\\s\\S]*)$`, 'gi')

/**
 * Rewrite every <lobeArtifact> tag in `md` into a fenced code block whose
 * language encodes the artifact type. Returns markdown safe to feed to the
 * normal render pipeline.
 */
export function preprocessArtifacts(md: string): string {
  const closed = md.replace(CLOSED_RE, (_m, attrs: string, code: string) => toFenced(attrs, code))
  return closed.replace(UNCLOSED_RE, (_m, attrs: string, code: string) => toFenced(attrs, code))
}

/** Build a fenced block: ```{lang}\n# {title}\n{code}\n``` */
function toFenced(attrs: string, code: string): string {
  const parsed = parseAttrs(attrs)
  const type = tagTypeToArtifactType(parsed.type)
  const lang = parsed.language ?? (type ? artifactTypeToLanguage(type) : 'text')
  const header = parsed.title ? `# ${parsed.title}\n` : ''
  const body = code.replace(/\n+$/, '')
  return `\`\`\`${lang}\n${header}${body}\n\`\`\``
}

/** Minimal attribute parser for `key="value"` pairs (the tag's attribute list). */
function parseAttrs(attrString: string): Record<string, string> {
  const out: Record<string, string> = {}
  const re = /(\w+)\s*=\s*"([^"]*)"/g
  let m: RegExpExecArray | null = re.exec(attrString)
  while (m !== null) {
    const key = m[1]
    const val = m[2]
    if (key !== undefined && val !== undefined) out[key] = val
    m = re.exec(attrString)
  }
  return out
}
