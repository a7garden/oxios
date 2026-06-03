// KaTeX renderer for HyperMD's fold-math addon.
// Ported from files.md (editor.js — KatexRenderer class).
// Source: https://github.com/zakirullin/files.md
//
// When HyperMD encounters $...$ or $$...$$, it creates a container element
// and instantiates this renderer to render the LaTeX expression inline.
//
// Requires: katex (npm package) — loaded via import, not global script.

import katex from 'katex'
import 'katex/dist/katex.min.css'

export class KatexRenderer {
  private container: HTMLElement
  private mode: string
  private span: HTMLSpanElement

  constructor(container: HTMLElement, mode: string) {
    this.container = container
    this.mode = mode
    this.span = document.createElement('span')
    this.container.appendChild(this.span)
  }

  startRender(expr: string): void {
    try {
      katex.render(expr, this.span, {
        displayMode: this.mode === 'display',
        throwOnError: false,
      })
    } catch {
      this.span.textContent = expr
    }
    this.onChanged?.(expr)
  }

  clear(): void {
    if (this.span.parentNode === this.container) {
      this.container.removeChild(this.span)
    }
  }

  isReady(): boolean {
    return true
  }

  /** Callback set by HyperMD after instantiation. */
  onChanged?: (expr: string) => void
}

/**
 * Factory that matches HyperMD's expected renderer interface.
 * Pass to HyperMD config as: `hmdFoldMath: { renderer: KatexRendererFactory }`
 */
export const KatexRendererFactory = KatexRenderer
