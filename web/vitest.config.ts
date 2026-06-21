import { defineConfig } from 'vitest/config';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const dirname =
  typeof __dirname !== 'undefined' ? __dirname : path.dirname(fileURLToPath(import.meta.url));

// Unit-test project only. The Storybook interaction tests (`.stories.tsx`)
// run via the `storybook` CLI locally; they are intentionally excluded from
// the default `test` script because they require a Playwright browser
// environment, which — per the repo CI policy — is not run in CI.
export default defineConfig({
  resolve: {
    alias: {
      '@': path.resolve(dirname, './src'),
    },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
    setupFiles: ['./src/__tests__/setup.ts'],
  },
});
