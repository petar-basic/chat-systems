import { defineConfig, type Plugin } from 'vite';
import { fileURLToPath, URL } from 'node:url';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

const buildId = process.env.APP_VERSION || Date.now().toString(36);

function versionFile(): Plugin {
  return {
    name: 'version-file',
    apply: 'build',
    generateBundle() {
      this.emitFile({
        type: 'asset',
        fileName: 'version.json',
        source: JSON.stringify({ version: buildId }),
      });
    },
  };
}

export default defineConfig(({ command }) => ({
  plugins: [react(), tailwindcss(), versionFile()],
  define: {
    'import.meta.env.VITE_BUILD_ID': JSON.stringify(command === 'build' ? buildId : 'dev'),
  },
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  // Strip stray console/debugger statements from production bundles only;
  // dev (`vite`/`serve`) keeps them for debugging. Vite 8 minifies with oxc
  // rather than esbuild, so this rides on the minifier instead of the transform.
  build:
    command === 'build'
      ? {
          rolldownOptions: {
            output: { minify: { compress: { dropConsole: true, dropDebugger: true } } },
          },
        }
      : {},
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
