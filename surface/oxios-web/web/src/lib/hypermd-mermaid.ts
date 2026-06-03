// Mermaid renderer for HyperMD's fold-code addon.
// Ported from files.md (lib/hypermd-mermaid.js).
// Source: https://github.com/zakirullin/files.md
//
// Key design: mermaid (~3MB) is lazy-loaded only when the first
// ```mermaid code block is encountered. Until then, zero cost.
//
// Usage: This module self-registers when imported. Just add:
//   import '@/lib/hypermd-mermaid'
// before creating the editor.

import type * as CodeMirrorNS from 'codemirror'

// Mermaid is loaded lazily — only import the type for safety.
type MermaidAPI = {
  initialize: (config: Record<string, unknown>) => void
  render: (id: string, code: string) => Promise<{ svg: string; bindFunctions?: (el: HTMLElement) => void }>
}

let mermaidLoad: Promise<MermaidAPI> | null = null
const mermaidIdPrefix = `_mermaid_${Math.random().toString(36).slice(2, 18)}_`
let mermaidCounter = 0

async function ensureMermaidLoaded(): Promise<MermaidAPI> {
  // Check if mermaid was already loaded globally
  const globalMermaid = (window as any).mermaid as MermaidAPI | undefined
  if (globalMermaid && typeof globalMermaid.render === 'function') {
    return globalMermaid
  }

  if (mermaidLoad) return mermaidLoad

  mermaidLoad = import('mermaid').then((mod) => {
    const mermaid = mod.default as unknown as MermaidAPI

    // Read theme accents from CSS variables (with fallbacks)
    const rootStyle = getComputedStyle(document.documentElement)
    const isDark = document.documentElement.classList.contains('dark')
    const primary = (rootStyle.getPropertyValue('--primary') || '').trim() || '#18181b'
    const textCol = isDark ? '#fafafa' : '#18181b'

    mermaid.initialize({
      startOnLoad: false,
      securityLevel: 'loose',
      theme: isDark ? 'dark' : 'default',
      themeVariables: {
        primaryColor: isDark ? '#27272a' : '#f4f4f5',
        primaryBorderColor: primary,
        primaryTextColor: textCol,
        lineColor: isDark ? '#a1a1aa' : '#52525b',
        signalColor: isDark ? '#a1a1aa' : '#52525b',
        textColor: textCol,
        labelTextColor: textCol,
        loopTextColor: textCol,
        signalTextColor: textCol,
        actorTextColor: textCol,
        noteTextColor: '#18181b',
      },
    })

    // Cache on window for subsequent calls
    ;(window as any).mermaid = mermaid
    return mermaid
  })

  mermaidLoad.catch(() => {
    mermaidLoad = null
  })

  return mermaidLoad
}

/**
 * CodeMirror renderer for ```mermaid blocks.
 * Follows the HyperMD fold-code renderer contract.
 */
function mermaidRenderer(
  code: string,
  info: { changed: () => void },
): HTMLElement {
  const id = mermaidIdPrefix + (mermaidCounter++).toString(36)
  const el = document.createElement('div')
  el.setAttribute('id', id)
  el.setAttribute('class', 'hmd-fold-code-image hmd-fold-code-mermaid')
  el.textContent = 'Loading mermaid…'

  ensureMermaidLoaded()
    .then(async (mermaid) => {
      try {
        const result = await mermaid.render(id + '_svg', code)
        el.innerHTML = result.svg
        el.removeAttribute('id')
        if (typeof result.bindFunctions === 'function') {
          result.bindFunctions(el)
        }
      } catch (err) {
        console.error('[hypermd-mermaid] render error:', err)
        el.textContent = code
      }
      info.changed()
    })
    .catch((err) => {
      console.error('[hypermd-mermaid] could not load mermaid:', err)
      el.textContent = code
      info.changed()
    })

  return el
}

// ── Self-registration with HyperMD's fold-code addon ───────────
//
// HyperMD registers renderers via CodeMirror option `hmdFoldCode`.
// We patch into it by importing this module before creating the editor.
// The editor config then just needs: hmdFoldCode: { mermaid: true }

// Register the renderer on the HyperMD.FoldCode namespace if available
function registerMermaidRenderer() {
  const CM = (window as any).CodeMirror as typeof CodeMirrorNS | undefined
  if (!CM) return

  // HyperMD exposes registerRenderer via its fold-code addon
  const HyperMD = (window as any).HyperMD
  if (HyperMD?.FoldCode?.registerRenderer) {
    HyperMD.FoldCode.registerRenderer({
      name: 'mermaid',
      pattern: /^mermaid$/i,
      renderer: mermaidRenderer,
      suggested: true,
    })
  }
}

// Auto-register on import
if (typeof window !== 'undefined') {
  // Defer registration until HyperMD is loaded
  if ((window as any).HyperMD) {
    registerMermaidRenderer()
  } else {
    // Will be called after hypermd-setup.ts imports complete
    window.addEventListener('load', () => registerMermaidRenderer(), { once: true })
    // Also try immediately in case HyperMD loaded synchronously
    setTimeout(registerMermaidRenderer, 0)
  }
}

export { mermaidRenderer, ensureMermaidLoaded }
