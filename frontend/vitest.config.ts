import { defineConfig } from 'vitest/config';
import { fileURLToPath, URL } from 'node:url';

// Standalone Vitest config (does NOT touch vite.config.ts).
// jsdom gives us a `window` so ApiClient's constructor (window.location.origin)
// works; `globals` lets the test files use describe/it/expect/vi without imports.
export default defineConfig({
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.{test,spec}.{ts,tsx}'],
  },
});
