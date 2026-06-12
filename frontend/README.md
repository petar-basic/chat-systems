# Frontend — Chat Systems

The web client — an installable PWA. React 19 + TypeScript, built with Vite,
styled with TailwindCSS, state via Zustand + TanStack Query, rich text via
TipTap. See [../docs/frontend.md](../docs/frontend.md) for architecture.

## Prerequisites

- Node 22+ (`nginx`/Docker build uses `node:22-alpine`)
- A running backend. Either start the full stack with Docker
  (`docker compose up -d` from the repo root) or run the api/realtime binaries on
  the host. The dev server proxies `/api` → `http://localhost:3000` and `/ws` →
  `ws://localhost:3004` (see `vite.config.ts`).

## Scripts

| Script                  | What it does                                                        |
|-------------------------|--------------------------------------------------------------------|
| `npm run dev`           | Vite dev server with HMR on **http://localhost:3001**              |
| `npm run build`         | Type-check (`tsc -b`) then production build to `dist/`            |
| `npm run preview`       | Serve the production build locally                                 |
| `npm run lint`          | ESLint (flat config; `no-explicit-any` and `no-console` are errors)|
| `npm run test`          | Vitest (jsdom) unit/component tests, run once                      |
| `npm run test:watch`    | Vitest in watch mode                                               |
| `npm run test:e2e`      | Playwright E2E against `E2E_BASE_URL` (default http://localhost:8080) |
| `npm run format`        | Prettier write over `src/` + `e2e/`                               |
| `npm run format:check`  | Prettier check (CI)                                               |

## Develop

```bash
npm install
npm run dev          # http://localhost:3001, proxying to a running backend
```

## Build (web)

```bash
npm run build        # → dist/
```

The Docker image (`frontend/Dockerfile`) builds this and serves `dist/` with
unprivileged nginx on port 8080, reverse-proxying `/api` and `/ws` to the backend
services. It is built by the `frontend` compose profile.

## Install as an app (PWA)

`public/manifest.webmanifest` + the icons under `public/icons/` make the app
installable from any Chromium browser (address-bar install icon) or Safari
(*File → Add to Dock*). Installed windows get the unread-count badge on the app
icon via the Badging API; nothing extra to build or sign.

## Notes

- Path alias `@` → `src/` (configured in `vite.config.ts` and `vitest.config.ts`).
- Production builds drop `console`/`debugger` via esbuild; `src/lib/logger.ts` is
  the one module allowed to use the console.
