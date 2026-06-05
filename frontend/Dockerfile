# ── Stage 1: Build ──────────────────────────────────────────
FROM node:22-alpine AS builder

WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

COPY index.html tsconfig.json tsconfig.app.json tsconfig.node.json vite.config.ts eslint.config.js ./
COPY public/ public/
COPY src/ src/

RUN npm run build

# ── Stage 2: Serve with nginx (non-root) ───────────────────
# nginxinc/nginx-unprivileged runs as the unprivileged `nginx` user (uid 101)
# out of the box and listens on 8080 instead of 80, so no root is required and
# no setcap/low-port binding is needed. The base image's own master process,
# cache and pid paths are already owned by the nginx user.
# NOTE: pin by digest for reproducible builds once you have one, e.g.:
#   FROM nginxinc/nginx-unprivileged:alpine@sha256:<digest>
FROM nginxinc/nginx-unprivileged:alpine

# Custom nginx config for SPA routing + API reverse proxy.
# NOTE: this conf must `listen 8080` (not 80) for the unprivileged image —
# see cross_file_needs for the docker/nginx.conf + docker-compose changes.
COPY docker/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist /usr/share/nginx/html

# Unprivileged image serves on 8080 (cannot bind <1024 as non-root).
EXPOSE 8080

# Runs as the non-root `nginx` user inherited from the base image.
CMD ["nginx", "-g", "daemon off;"]
