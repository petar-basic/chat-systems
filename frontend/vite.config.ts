import { defineConfig } from 'vite';
import { fileURLToPath, URL } from 'node:url';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

const ELECTRON_CSP =
  "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; " +
  "img-src 'self' data: blob: http: https:; font-src 'self' data:; connect-src 'self' http: https: ws: wss:; " +
  "media-src 'self' blob: https:; worker-src 'self' blob:; object-src 'none'; base-uri 'self'";

export default defineConfig(({ command, mode }) => ({
  base: mode === 'electron' ? './' : '/',
  plugins: [
    react(),
    tailwindcss(),
    mode === 'electron'
      ? {
          name: 'inject-csp',
          transformIndexHtml(html: string) {
            return html.replace(
              '</title>',
              `</title>\n    <meta http-equiv="Content-Security-Policy" content="${ELECTRON_CSP}" />`,
            );
          },
        }
      : null,
  ],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  // Strip stray console/debugger statements from production bundles only;
  // dev (`vite`/`serve`) keeps them for debugging.
  esbuild: command === 'build' ? { drop: ['console', 'debugger'] } : {},
  server: {
    port: 3001,
    proxy: {
      '/api': 'http://localhost:3000',
      '/ws': {
        target: 'ws://localhost:3004',
        ws: true,
        configure: (proxy) => {
          proxy.on('error', (err) => {
            if ((err as NodeJS.ErrnoException).code === 'EPIPE') return;
            console.error('ws proxy error:', err.message);
          });
        },
      },
    },
  },
}));
