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
