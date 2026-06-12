# Contributing & Running

How to run Chat Systems for development and production, and the standards the
codebase holds itself to. For the *why* behind the design, see
[backend.md](./backend.md) and [frontend.md](./frontend.md).

## Prerequisites

- **Docker** + Docker Compose v2 (the only hard requirement for running the stack)
- **Rust** stable (with `rustfmt` + `clippy`) — for host-run backend development
- **Node** 20+ — for host-run frontend development

## Configuration

All configuration is via environment variables. Copy the template and fill in the
secrets — `docker compose` auto-loads `.env`; the API refuses to start with a weak
or default `JWT_SECRET`.

```bash
cp .env.example .env
openssl rand -hex 32   # use for JWT_SECRET
```

The full list with defaults lives in [`backend/api/src/config.rs`](../backend/api/src/config.rs).

## Running

### Full stack in Docker (quickest)

```bash
docker compose --profile frontend up -d --build
ADMIN_PASSWORD=... ./seed.sh         # optional demo data
```

- App: http://localhost:8080 · MailHog: http://localhost:8025
- Convenience ports (Postgres 5433, Redis 6380, MailHog 1025/8025) are bound to
  `127.0.0.1` only.

### Local development (run the binaries on the host)

Run infra in Docker, and the api / realtime / SPA on the host for fast iteration.

```bash
# 1. Infra only (Postgres, Redis, MailHog)
docker compose -f docker-compose.yml -f docker-compose.dev.yml up -d

# 2. backend/.env for the host binaries — reuse the secrets from your root .env, do NOT commit
cat > backend/.env << EOF
DATABASE_URL=postgres://chat:${POSTGRES_PASSWORD}@localhost:5433/chatsystems
REDIS_URL=redis://127.0.0.1:6380
ADMIN_EMAIL=admin@dev.local
ADMIN_PASSWORD=${ADMIN_PASSWORD}
SMTP_HOST=localhost
JWT_SECRET=${JWT_SECRET}
EOF

# 3. Backend (two terminals)
cd backend && cargo run -p chat-api
cd backend && cargo run -p chat-realtime

# 4. Frontend — Vite on :3001, proxies /api → :3000 and /ws → :3004
cd frontend && npm install && npm run dev
```

### Production (HTTPS, restart policies, backups)

The production override adds a Caddy edge proxy (automatic Let's Encrypt TLS),
`restart: unless-stopped`, resource limits, S3/MinIO storage, real SMTP, an
`autoheal` sidecar that restarts containers Docker marks unhealthy, and two
backup sidecars — `db-backup` (verified `pg_dump`) and `minio-backup`
(`mc mirror` of the upload bucket). Point your domain's DNS at the host first.

```bash
# Required in .env: DOMAIN, ACME_EMAIL, JWT_SECRET, ADMIN_PASSWORD,
#   POSTGRES_PASSWORD, MINIO_ROOT_PASSWORD, SMTP_HOST, SMTP_FROM_ADDRESS
# Optional in .env: VERSION (tags the app images for rollback — set to a git SHA
#   or release tag), BACKUP_OFFSITE_REMOTE (off-host copy of each verified dump).
docker compose -f docker-compose.yml -f docker-compose.prod.yml \
  --profile frontend --profile s3 up -d --build
```

Caddy is the only service that publishes public ports (80/443); everything else
stays on the docker network. `PUBLIC_URL=https://$DOMAIN` makes auth cookies
`Secure`; HSTS and other security headers are applied at the edge and in nginx.
Postgres dumps land in the `pg_backups` volume and the mirrored upload bucket in
`minio_backups`. For backup, restore, upgrade, and rollback procedures see
[RUNBOOK.md](./RUNBOOK.md).

## Project layout

```
backend/
  api/        chat-api binary — stateless REST API (feature modules under src/)
  realtime/   chat-realtime binary — WebSocket gateway
  shared/     shared crates (common errors/CORS/validation, event envelopes)
  migrations/ SQL migrations (run automatically on api start)
frontend/
  src/        React SPA (features/, components/, hooks/, stores/, lib/, shared/)
  electron/   desktop wrapper (main + preload)
  e2e/        Playwright end-to-end tests
docker/       Caddyfile, MinIO init, Postgres + MinIO backup scripts
docs/         this folder
```

## Coding standards

### Backend (Rust)

- **Feature-modular layering.** Each feature owns `mod / models / repo / routes`, plus
  `service / publisher / consumer / executor / storage` where warranted. Routes parse +
  authorize + delegate; **all SQL lives in repos**; features don't reach into another
  feature's repo. See [backend.md](./backend.md) for the full contract.
- **No `unwrap` / `expect` / `panic` in request paths** — return `AppError`. Startup
  config validation is the only place that fails fast.
- **Parameterized SQL only** (sqlx bind params) — never string-built queries.
- Formatted with `cargo fmt`; lints clean under `cargo clippy --workspace --all-targets -- -D warnings`.

### Frontend (TypeScript / React)

- **Strict TypeScript** — no `any`, no `@ts-ignore`, no `eslint-disable`. `tsc -b` clean.
- **Feature-modular** under `src/features/*` with barrels; smart logic lives in hooks,
  views stay thin (e.g. `useWorkspaceController` + `WorkspacePage`).
- **State split**: TanStack Query for server state, Zustand for UI state; WebSocket events
  reconcile into the Query cache via `wsQuerySync`.
- **Query keys always go through the `QUERY_KEYS` factory** — never hand-built arrays.
- Formatted with Prettier; lints clean under ESLint (which includes `react-hooks` rules).

### Both

- **Write zero comments.** Prefer names and structure that don't need them; if a comment
  feels necessary, treat it as a sign to refactor.
- Keep changes surgical and consistent with the surrounding code.

## Testing

```bash
# Backend — integration tests against an ephemeral Postgres (+ Redis for realtime)
cd backend && cargo test --workspace

# Frontend — unit/component tests
cd frontend && npm run test

# Frontend — end-to-end (needs the stack running + seeded; set the admin password)
cd frontend && npx playwright install --with-deps
E2E_PASSWORD=<admin password> E2E_BASE_URL=http://localhost:8080 npm run test:e2e
```

Backend integration tests provision a real Postgres per test via `#[sqlx::test]`, run
migrations, and drive the full Axum stack — including the authorization matrix
(member-ok / non-member-forbidden / no-token / not-found) for every endpoint.

## CI

[`.github/workflows/ci.yml`](../.github/workflows/ci.yml) runs on every push to
`main` and every PR, all steps **blocking**:

- **Backend:** `cargo fmt --check`, `clippy -D warnings`, `cargo build`, `cargo test`
  (against live Postgres + Redis service containers), then `cargo audit`.
- **Frontend:** `npm audit --audit-level=high`, `prettier --check`, `eslint`,
  `tsc -b`, `vite build`, `vitest`.

[`.github/dependabot.yml`](../.github/dependabot.yml) opens weekly grouped update
PRs for cargo (`/backend`), npm (`/frontend`), and github-actions.

## Commits & PRs

- Keep commits focused; write a clear subject line describing the *why*.
- A change should leave the build green: run the backend and frontend test/lint steps
  above before opening a PR.
