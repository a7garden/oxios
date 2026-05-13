# OxiBrowser Headless Browser

## Purpose

Provides headless web browsing capability for Oxios agents via the embedded
OxiBrowser engine. Agents can navigate to URLs, click elements, type text,
extract content, execute JavaScript, and query DOM elements.

## Prerequisites

None. OxiBrowser is a pure Rust engine embedded directly in the Oxios kernel.
No external browser binary or CDP server is required.

## Tools

### `browse`
Open a URL and extract its text content.

### `fetch`
Fetch a URL and dump its raw content.

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

### Query specific elements
```
Action: query_all
Selector: h2
→ "1. Introduction\n2. Getting Started\n3. API Reference"
```

## Architecture Notes

OxiBrowser runs entirely in-process — no subprocess management needed.
Built on:
- `html5ever` (Servo ecosystem) for HTML parsing
- `boa_engine` for JavaScript evaluation
- `reqwest` for HTTP requests

Memory footprint: ~10MB (vs Chromium's ~2GB, Lightpanda's ~123MB).
