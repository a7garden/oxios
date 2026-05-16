# OxiBrowser Headless Browser

## Purpose

Provides headless web browsing capability for Oxios agents via the embedded
OxiBrowser engine. Agents can navigate to URLs, click elements, type text,
extract content, execute JavaScript, query DOM elements, and more.

## Prerequisites

None. OxiBrowser is a pure Rust engine embedded directly in the Oxios kernel.
No external browser binary or CDP server is required.

## Browser Tool (Agent-Level)

When the `browser` feature is enabled, agents have access to the `browser` tool
with these actions:

| Action | Description | Parameters |
|--------|-------------|------------|
| `navigate` | Go to a URL | `url` |
| `back` | Navigate back in history | — |
| `forward` | Navigate forward in history | — |
| `reload` | Reload the current page | — |
| `click` | Click an element | `selector` |
| `type` | Type text into an element | `selector`, `text` |
| `evaluate` | Run JavaScript | `javascript` |
| `evaluate_with_await` | Run JavaScript, awaiting Promise resolution | `javascript`, `await_promise` |
| `html` | Get page HTML | — |
| `text` | Get page text content | — |
| `markdown` | Get page as proper Markdown | — |
| `screenshot` | Capture PNG screenshot | — |
| `query_all` | Get text of matching elements | `selector` |
| `wait_for` | Wait for element to appear | `selector`, `timeout_ms` |
| `load_sub_resources` | Preload JS/CSS/images | — |
| `close` | Close current page | — |

## Examples

### Navigate and extract content
```
Action: navigate
URL: https://example.com
→ "Navigated to 'https://example.com/'. Title: 'Example Domain'"

Action: text
→ "Example Domain\nThis domain is for use in illustrative examples..."
```

### Rich Markdown extraction
```
Action: markdown
→ "# Example Domain\n\n**Welcome** to our website.\n\n- Item 1\n- Item 2\n\n[Link](https://example.com)"
```

### Interactive form
```
Action: click
Selector: "#username"
→ "Clicked '#username'"

Action: type
Selector: "#username"
Text: "myuser"
→ "Typed 6 chars into '#username'"

Action: evaluate
JavaScript: "document.querySelector('#password').value"
→ "password123"
```

### Wait for dynamic content
```
Action: wait_for
Selector: ".loading-complete"
Timeout: 5000
→ "Element '.loading-complete' found within 5000ms"
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

## Security

- SSRF protection: blocks requests to private/internal IP ranges
- robots.txt obedience by default
- HttpOnly cookie enforcement for JavaScript access
- SameSite cookie enforcement
- Cookie count/size limits (RFC 6265)