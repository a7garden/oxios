// Markdown string utilities, ported from files.md (web/lib/md.js).
// Source: https://github.com/zakirullin/files.md
// License: MIT
//
// These functions operate on raw markdown text strings. They are used by
// both the editor and the chat to manipulate checklists, headers, etc.

/**
 * Add a checklist item to a markdown string.
 * If the item already exists (by text or by hash), it is removed first.
 * Completed items (- [x]) are pushed to the end; incomplete items (- [ ])
 * are inserted before the last incomplete item.
 */
export function addChecklistItem(
  md: string,
  item: string,
  checked = false,
): string {
  md = normNewLines(md)

  const lines = md.split('\n')
  const filteredLines: string[] = []

  for (const line of lines) {
    const trimmedLine = line.trim()
    if (trimmedLine.length < 6) {
      filteredLines.push(trimmedLine)
      continue
    }
    const itemText = trimmedLine.substring(6)
    if (itemText === item) continue
    filteredLines.push(trimmedLine)
  }

  if (checked) {
    filteredLines.push('- [x] ' + item)
  } else {
    // Find the last incomplete item and insert before it
    let insertIndex = filteredLines.length
    for (let i = filteredLines.length - 1; i >= 0; i--) {
      if (filteredLines[i]!.trim().startsWith('- [ ] ')) {
        insertIndex = i
      }
    }
    if (insertIndex === filteredLines.length) {
      filteredLines.push('- [ ] ' + item)
    } else {
      filteredLines.splice(insertIndex, 0, '- [ ] ' + item)
    }
  }

  return filteredLines.join('\n')
}

/**
 * Extract the title (first `# ...` line) and body (everything after).
 * Returns `{ header, body }`. Title is capped at `maxTitleLen` chars.
 */
export function extractHeaderAndBody(
  text: string,
  maxTitleLen = 100,
): { header: string; body: string } {
  const lines = text.split('\n')

  let header = ''
  const bodyLines: string[] = []
  let headerFound = false

  for (const line of lines) {
    if (!headerFound && line.startsWith('# ')) {
      header = line.substring(2).trim()
      if (header.length > maxTitleLen) {
        header = header.substring(0, maxTitleLen) + '...'
      }
      headerFound = true
    } else {
      bodyLines.push(line)
    }
  }

  return { header, body: bodyLines.join('\n').trim() }
}

/**
 * Add a header line and text body to a markdown file.
 * If `atStart` is true, prepends; otherwise appends.
 * Optionally adds a timestamp before the text.
 */
export function addHeaderAndBody(
  existingMd: string,
  header: string,
  text: string,
  options?: { atStart?: boolean; withTimestamp?: boolean; timezone?: string },
): string {
  const { atStart = false, withTimestamp = true } = options ?? {}
  const md = normNewLines(existingMd)

  const timestamp = withTimestamp
    ? `\`${formatTime(options?.timezone)}\` `
    : ''

  const block = `### ${header}\n${timestamp}${text}`

  if (atStart) {
    // Insert after the title (# first line)
    const lines = md.split('\n')
    if (lines.length > 0 && lines[0]!.startsWith('# ')) {
      lines.splice(1, 0, '', block, '')
      return lines.join('\n')
    }
    return block + '\n\n' + md
  }

  return md ? md + '\n\n' + block : `# ${header}\n\n${text}`
}

/** Normalize line endings (\\r\\n → \\n, strip trailing whitespace). */
export function normNewLines(text: string): string {
  return text.replace(/\r\n/g, '\n').replace(/\r/g, '\n')
}

/** Check if markdown text contains an image. */
export function hasImage(text: string): boolean {
  return /!\[.*?\]\(.*?\)/.test(text)
}

/** Format current time as `HH:MM` in the given timezone. */
function formatTime(timezone?: string): string {
  const now = new Date()
  if (timezone) {
    return now.toLocaleTimeString('en-GB', { timeZone: timezone, hour: '2-digit', minute: '2-digit' })
  }
  return now.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' })
}
