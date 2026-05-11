# Lightpanda Headless Browser

## Purpose

Provides headless web browsing capability for Oxios agents. Agents can navigate
to URLs, click elements, type text, extract content, execute JavaScript, and
capture screenshots.

## Prerequisites

Lightpanda must be installed on the host system.

### Installation

**macOS (Apple Silicon):**
```bash
brew install lightpanda-io/browser/lightpanda
```

**Linux (x86_64):**
```bash
curl -L -o /usr/local/bin/lightpanda https://github.com/lightpanda-io/browser/releases/download/nightly/lightpanda-x86_64-linux
chmod +x /usr/local/bin/lightpanda
```

**Linux (ARM64):**
```bash
curl -L -o /usr/local/bin/lightpanda https://github.com/lightpanda-io/browser/releases/download/nightly/lightpanda-aarch64-linux
chmod +x /usr/local/bin/lightpanda
```

Verify installation:
```bash
lightpanda version
```

## Tools

### `browse`
Open a URL and extract its text content.

### `screenshot`
Capture a PNG screenshot of a web page.

## Browser Tool (Agent-Level)

When the `browser` feature is enabled, agents have access to the `browser` tool
with these actions:

| Action | Description | Parameters |
|--------|-------------|------------|
| `navigate` | Go to a URL | `url` |
| `click` | Click an element | `selector` |
| `type` | Type text into an element | `selector`, `text` |
| `evaluate` | Run JavaScript | `javascript` |
| `html` | Get page HTML | — |
| `text` | Get page text content | — |
| `screenshot` | Capture screenshot | — |
| `query_all` | Get text of matching elements | `selector` |
| `close` | Close current page | — |

## Examples

### Navigate and extract text
```
Action: navigate
URL: https://example.com
→ "Navigated to 'https://example.com/'. Title: 'Example Domain'"

Action: text
→ "Example Domain\nThis domain is for use in illustrative examples..."
```

### Click a button
```
Action: navigate
URL: https://example.com

Action: click
Selector: a[href="https://www.iana.org/domains/example"]
→ "Clicked 'a[href=\"https://www.iana.org/domains/example\"]'"
```

### Query specific elements
```
Action: query_all
Selector: h2
→ "1. Introduction\n2. Getting Started\n3. API Reference"
```

## Architecture Notes

Lightpanda runs as a CDP server subprocess. Oxios connects via WebSocket
using the `chromiumoxide` crate. The process lifecycle is managed automatically:
- Started on first browser use
- Reused across multiple operations
- Killed on agent shutdown

Memory footprint: ~123MB (vs Chromium's ~2GB).
