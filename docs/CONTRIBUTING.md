# Contributing & Running

How to run Chat Systems for development and production, and the standards the
codebase holds itself to. For the *why* behind the design, see
[backend.md](./backend.md) and [frontend.md](./frontend.md).

## Prerequisites

- **Docker** + Docker Compose v2 (the only hard requirement for running the stack)
- **Rust** stable (with `rustfmt` + `clippy`) — for host-run backend development
- **Node** 22.22+ — for host-run frontend development (required by React Router 8)

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

# 3. Backend (three terminals; chat-worker is optional unless you are working on
#    webhooks, reminders, notifications or scheduled messages)
cd backend && cargo run --bin chat-api
cd backend && cargo run --bin chat-worker
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
  api/        library + two binaries — chat-api (stateless REST API) and
              chat-worker (background consumers); feature modules under src/
  realtime/   chat-realtime binary — WebSocket gateway
  shared/     shared crates (common errors/CORS/validation, event envelopes)
  migrations/ SQL migrations (run automatically on api start)
frontend/
  src/        React SPA (features/, components/, hooks/, stores/, lib/, shared/)
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
- **Permission checks come from `authz`.** `backend/api/src/authz.rs` holds the only
  definition of each predicate (`require_workspace_member`, `require_workspace_role`,
  `require_channel_access`, `require_channel_moderator`,
  `require_conversation_participant`). Never re-derive one in a feature module or in
  SQL — the rules drifted that way before and it cost real access-control bugs. A
  query that needs to reproduce channel visibility asks `ChannelAccess`, it does not
  restate the rule.
- **Sessions end through `sessions::revoke`.** Deleting refresh tokens, marking access
  tokens invalid and closing live sockets belong together; the three steps have drifted
  apart before.
- **Every `String` in a request DTO has a validator.** `shared_common::validation` is the
  only place a limit is written down, and a handler calls it before any repo call. A
  column that happens to be wide enough is not validation — it produces a 500 where the
  answer is 400. Deliberate exceptions (login and forgot-password, where an early
  rejection would be an oracle) are listed in the roadmap, not left implicit.
- **A unique violation a user can trigger is a 409.** Use
  `shared_common::errors::is_unique_violation`; never let a raw database string reach the
  wire as a 500.
- **Effects re-check at the moment of the effect.** Anything detached from the request
  that produced it — the scheduled dispatcher, the reminder checker — re-runs the same
  `authz` predicate the interactive handler runs. Authorization granted days ago is not
  authorization now.
- **Realtime delivery is at-least-once.** Anything reading the event log has to tolerate
  seeing an event twice: replay overlaps the live tail on purpose, and consumer groups
  redeliver what was not acknowledged. Handlers upsert by id; anything with an outward side
  effect (a webhook call, an email) claims a row first. Acknowledge in a way that cannot be
  skipped — the worker consumers put the per-event work in an async block precisely so that
  `continue` does not compile.
- **Replay must reuse the live path's routing.** New event types go through
  `handle_event_for`, which takes an `Audience`; never add a second delivery path for
  replayed events. That is where a backlog starts carrying channels the client cannot see.
- **Destructive actions are recorded through `audit::record`.** Anything that deletes,
  removes, grants or reveals takes a `ClientIp` extractor and writes one `AuditEntry`
  with a typed `AuditAction`. Adding a variant to the enum is part of the change, not a
  follow-up — the read side filters on `action`, so a free-string call site is an entry
  nobody will ever find. `record` never returns an error to the handler: the action has
  already happened, and failing the request on the trail turns an audit problem into an
  availability one.
- **New queries use `sqlx::query_as!` / `query!`.** Decided 2026-08-07: the macros are
  the standard for new code, existing runtime queries convert opportunistically when
  their method is touched for another reason, and there is no big-bang rewrite. Run
  `cargo sqlx prepare --workspace -- --all-targets` and commit `.sqlx/` — image builds
  set `SQLX_OFFLINE=true` and have no database, so a macro without a cache entry breaks
  the Docker build. CI enforces this with `cargo sqlx prepare --check`.
- **The tests read `DATABASE_URL` and `REDIS_URL` from `backend/.cargo/config.toml`.** Both
  point at the host-side ports compose publishes (5433 and 6380), so the suite runs with no
  environment prefix. Neither is forced, so exporting either one still wins — which is how
  CI aims them at its own services.
- **One statement per `-- no-transaction` migration.** Postgres wraps a multi-statement
  string in an implicit transaction, so `CREATE INDEX CONCURRENTLY` fails with *cannot run
  inside a transaction block* even in a migration marked `-- no-transaction`. A batched
  backfill cannot live in a migration either — there is nowhere to commit between batches;
  put it in the worker (see `search::backfill`).
- **`#[sqlx::test(migrations = "../migrations")]`, always.** A bare `#[sqlx::test]` gets an
  empty database and every test in the file fails with `relation "users" does not exist` —
  which reads like a broken fixture rather than a missing attribute.
- Formatted with `cargo fmt`; lints clean under `cargo clippy --workspace --all-targets -- -D warnings`.

### Frontend (TypeScript / React)

- **Strict TypeScript** — no `any`, no `@ts-ignore`, no `eslint-disable`. `tsc -b` clean.
- **No editor instance for read-only content.** Messages render through
  `MessageContent`, which walks a parsed tree into elements. TipTap belongs in the composer
  and the inline edit form, where something is actually being edited — one instance at a
  time. A `useEditor` call anywhere else is a performance bug (CS-024 measured it at 601× on
  a 500-message channel).
- **No `dangerouslySetInnerHTML` on message content, ever.** The renderer maps parsed nodes
  to elements directly, so the XSS posture is a property of the code rather than of a
  sanitiser's configuration.
- **Feature-modular** under `src/features/*` with barrels; smart logic lives in hooks,
  views stay thin (e.g. `useWorkspaceController` + `WorkspacePage`).
- **State split**: TanStack Query for server state, Zustand for UI state; WebSocket events
  reconcile into the Query cache via `wsQuerySync`.
- **Query keys always go through the `QUERY_KEYS` factory** — never hand-built arrays.
- **No server state inside `MessageContent`.** It renders once per message, and a `useQuery`
  there also makes every test of it need a `QueryClient` to render text. Workspace-wide data
  the renderer needs — custom emoji, your own group ids — is populated into a Zustand store
  by `useWorkspaceController`, the way `useUserCache` already works.
- **Typecheck with `tsc -b`, not `tsc --noEmit`.** The bare form picks up a different
  project in this repo and passes while CI fails.
- Formatted with Prettier; lints clean under ESLint (which includes `react-hooks` rules).

### Both

- **Write zero comments.** Prefer names and structure that don't need them; if a comment
  feels necessary, treat it as a sign to refactor.
- Keep changes surgical and consistent with the surrounding code.

## Formatting

One-time, after cloning:

```bash
git config core.hooksPath .githooks
```

`.githooks/pre-commit` then runs `cargo fmt --all` and `npm run format` whenever a commit
touches a Rust file or a frontend `.ts`/`.tsx`, and re-stages what it changed — the same
two commands CI checks with, so a commit cannot fail CI on formatting alone. A staged file
that also carries unstaged edits is reformatted but left out of the commit, and named, so
partial staging survives. `git commit --no-verify` skips the hook.

## Testing

```bash
# Backend — integration tests against an ephemeral Postgres (+ Redis for realtime)
# cargo-nextest runs each test in its own process and kills one that stops
# making progress after 60s, instead of letting it hang the run.
cd backend && cargo nextest run --workspace

# Frontend — unit/component tests
cd frontend && npm run test

# Frontend — end-to-end (needs the stack running + seeded; set the admin password)
cd frontend && npx playwright install --with-deps
E2E_PASSWORD=<admin password> E2E_BASE_URL=http://localhost:8080 npm run test:e2e
```

The E2E suite drives two logged-in browser contexts against the running stack and
covers the realtime paths (messages, threads, reactions, typing, mentions and
notifications, DMs, unread state), the role matrix, the invite → registration flow,
gateway-restart reconnect/backfill, and hostile-payload rendering. It restarts the
`realtime` container in one test, so run it against a local stack, not a shared one.
A global setup clears Redis login-throttle keys before each run
(`E2E_SKIP_THROTTLE_RESET=1` disables that; raise `LOGIN_ATTEMPTS_PER_IP` instead
when the stack is not reachable via `docker compose`).

Backend integration tests provision a real Postgres per test via `#[sqlx::test]`, run
migrations, and drive the full Axum stack — including the authorization matrix
(member-ok / non-member-forbidden / no-token / not-found) for every endpoint.

## CI

[`.github/workflows/ci.yml`](../.github/workflows/ci.yml) runs on every push to
`main` and every PR, all steps **blocking**:

- **Backend:** `cargo fmt --check`, `clippy -D warnings`, `cargo build`, `cargo nextest run`
  (against live Postgres + Redis service containers), then `cargo audit`.
- **Frontend:** `npm audit --audit-level=high`, `prettier --check`, `eslint`,
  `tsc -b`, `vite build`, `vitest`.

Both jobs are **path-filtered**: a `changes` job resolves which trees the PR touched,
and each language job runs only when its tree changed. A docs-only PR runs neither.
Editing `ci.yml` itself re-runs both, so a broken filter cannot silently disable the
checks it selects.

- **E2E:** boots the real stack with `docker compose --profile frontend`, seeds it,
  and runs the Playwright suite. Runs when anything the stack is built from changes —
  `backend/**`, `frontend/**`, `docker/**`, `docker-compose*.yml`, `seed.sh` — and
  uploads the report, screenshots and container logs on failure.

**`All checks` is the only job that should be a required status check.** It always runs,
treats skipped jobs as passing, and fails if any job failed or was cancelled. Requiring
`Backend (Rust)`, `Frontend (Node)` or `E2E (Playwright)` directly blocks every
docs-only PR forever: a skipped job reports no status at all, so the check stays pending
rather than green. Adding a new job needs no change to branch protection, only an entry
in the gate's `needs`.

[`.github/dependabot.yml`](../.github/dependabot.yml) opens weekly grouped update
PRs for cargo (`/backend`), npm (`/frontend`), and github-actions.

## Commits & PRs

- Keep commits focused; write a clear subject line describing the *why*.
- A change should leave the build green: run the backend and frontend test/lint steps
  above before opening a PR.
