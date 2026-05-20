import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { TanStackRouterVite } from '@tanstack/router-plugin/vite'
import path from 'node:path'

export default defineConfig({
  plugins: [TanStackRouterVite({ autoCodeSplitting: true }), react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  optimizeDeps: {
    include: [
      'codemirror',
      'codemirror/lib/codemirror',
      'codemirror/mode/javascript/javascript',
      'codemirror/mode/python/python',
      'codemirror/mode/go/go',
      'codemirror/mode/shell/shell',
      'codemirror/mode/php/php',
      'codemirror/mode/markdown/markdown',
      'codemirror/addon/edit/continuelist',
      'codemirror/addon/selection/active-line',
      'codemirror/addon/hint/show-hint',
    ],
  },
  server: {
    proxy: {
      '/api': 'http://localhost:3000',
      '/health': 'http://localhost:3000',
    },
  },
})
