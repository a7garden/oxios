import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { TanStackRouterVite } from '@tanstack/router-plugin/vite'
import path from 'node:path'
import { execSync } from 'node:child_process'
import { readFileSync, writeFileSync } from 'node:fs'
import type { Plugin } from 'vite'

// Root workspace Cargo.toml is the single source of truth for versioning.
// RFC-026 moved `web/` to the repo root, so the Cargo.toml is ONE level up
// (was three when this lived under `surface/oxios-web/web/`). The stale
// `..`/`..`/`..` resolved outside the repo, read a non-existent Cargo.toml,
// and silently fell back to "0.0.0" — which made the daily health check
// think every build was outdated and re-download forever.
const WORKSPACE_ROOT = path.resolve(__dirname, '..')

/// Reads the binary version from the root `[package].version` in Cargo.toml.
function readBinaryVersion(): string {
  try {
    const cargo = readFileSync(path.join(WORKSPACE_ROOT, 'Cargo.toml'), 'utf8')
    const m = cargo.match(/^version\s*=\s*"([^"]+)"/m)
    return m ? m[1] : '0.0.0'
  } catch {
    return '0.0.0'
  }
}

/// Short git SHA of the HEAD commit (null when git is unavailable).
function readGitSha(): string | null {
  try {
    return (
      execSync('git rev-parse --short HEAD', { cwd: WORKSPACE_ROOT, stdio: ['ignore', 'pipe', 'ignore'] })
        .toString()
        .trim() || null
    )
  } catch {
    return null
  }
}

/**
 * Emits `dist/version.json` so the backend can report the exact Web UI build
 * that is being served. Runs at the end of every `vite build`, so both CI
 * releases and local `build:deploy` produce it without extra steps.
 *
 * Single source of truth: root `Cargo.toml` `[package].version` — the same
 * value the binary stamps via `env!("CARGO_PKG_VERSION")`. This keeps
 * `version` and `web_version` in `/api/status` in lockstep by construction.
 */
function generateVersionJson(): Plugin {
  return {
    name: 'oxios-generate-version-json',
    writeBundle() {
      const data = {
        version: readBinaryVersion(),
        git_sha: readGitSha(),
        built_at: new Date().toISOString(),
      }
      writeFileSync(
        path.resolve(__dirname, 'dist', 'version.json'),
        `${JSON.stringify(data, null, 2)}\n`,
      )
      // eslint-disable-next-line no-console
      console.log(`  version.json → ${data.version}${data.git_sha ? ` (${data.git_sha})` : ''}`)
    },
  }
}

/**
 * Rewrites `es-toolkit/compat/<name>` imports to re-export from the ESM compat
 * index.
 *
 * Why: es-toolkit's `./compat/<name>` subpath exports map ONLY to CommonJS (no
 * ESM condition). recharts pulls these in via default imports
 * (`import get from 'es-toolkit/compat/get'`). Vite 8 / Rolldown 1.0.3 wraps
 * that CJS with `__commonJSMin` and mishandles the internal circular requires,
 * emitting a self-referential `var require_get = require_get()` that crashes at
 * runtime with "t is not a function" on any route that renders a recharts chart
 * (e.g. /resources). Re-exporting from the ESM compat index — which ships a
 * real ESM entry — sidesteps the CJS interop entirely.
 */
function fixEsToolkitCompat(): Plugin {
  const PREFIX = '\0estk-compat:'
  return {
    name: 'ox-fix-es-toolkit-compat',
    enforce: 'pre',
    resolveId(source) {
      const m = source.match(/^es-toolkit\/compat\/([^/]+)$/)
      if (m) return PREFIX + m[1]
    },
    load(id) {
      if (!id.startsWith(PREFIX)) return null
      const name = id.slice(PREFIX.length)
      return `export { ${name} as default, ${name} } from "es-toolkit/compat";`
    },
  }
}
export default defineConfig({
  plugins: [
    fixEsToolkitCompat(),
    TanStackRouterVite({ autoCodeSplitting: true }),
    react(),
    tailwindcss(),
    generateVersionJson(),
  ],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  optimizeDeps: {
    include: [
      'radix-ui',
      '@uiw/react-codemirror',
      '@codemirror/lang-markdown',
      '@codemirror/autocomplete',
      '@codemirror/commands',
      '@codemirror/view',
      '@codemirror/state',
      '@codemirror/language',
    ],
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:4200',
        ws: true,
      },
      '/health': 'http://localhost:4200',
    },
  },
})
