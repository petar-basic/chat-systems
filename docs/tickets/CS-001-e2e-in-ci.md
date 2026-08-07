# CS-001 — Run the Playwright suite in CI

**Wave:** 0 — Safety net and structural groundwork
**Area:** infra
**Blocked by:** —
**Blocks:** everything (this is the regression net for waves 1–7)

## Problem

`frontend/e2e/` holds 12 spec files covering realtime paths, the role matrix, the
invite → registration flow and gateway-restart reconnect. The `frontend` job in
[`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) runs format, lint, `tsc -b`,
build and `npm run test` — and never runs `npm run test:e2e`.

A suite that does not run rots. Within two months half of it will be broken and nobody
will know, and the waves below rewrite exactly the paths those specs cover.

## Why first

Waves 1–7 change authorization, session lifetime, the event pipeline and the message
renderer. Every one of those is a place where a green unit suite and a broken product
coexist happily. The E2E suite is the only thing in the repo that would catch it, so it
has to be running before the first of those tickets merges.

## Approach

Add an `e2e` job to `ci.yml` that boots the real stack, seeds it, and runs Playwright.

1. New job `e2e`, `needs: [backend, frontend]` so it only burns minutes on a green tree.
2. Bring the stack up with the repo's own compose files rather than a bespoke service
   matrix — the specs assume the real topology (nginx in front of `api` and `realtime`),
   and one spec restarts the `realtime` container:
   ```yaml
   - run: cp .env.example .env && ./.github/scripts/ci-env.sh
   - run: docker compose --profile frontend up -d --build
   - run: ./seed.sh
   ```
   `ci-env.sh` writes the CI values (`JWT_SECRET`, `ADMIN_PASSWORD`, `POSTGRES_PASSWORD`,
   `MINIO_ROOT_PASSWORD`) into `.env`. Generate them per run; never commit them.
3. Wait for readiness on `/readyz` for both `api` and `realtime` before starting the
   suite, with a bounded retry loop — not a fixed sleep.
4. Run with the documented env contract:
   ```yaml
   - run: npx playwright install --with-deps chromium
   - run: E2E_PASSWORD=... E2E_BASE_URL=http://localhost:8080 npm run test:e2e
   ```
5. Upload `playwright-report/` and `test-results/` as artifacts on failure, plus
   `docker compose logs` — a red E2E job with no logs is a job people learn to ignore.
6. Set `fail-fast: false` and a job timeout so a hung socket does not eat the runner
   budget.

Keep `E2E_SKIP_THROTTLE_RESET` unset so the global setup clears the login-throttle keys,
which is what it exists for.

## Acceptance

- [ ] `e2e` job runs on every PR and blocks merge on failure.
- [ ] Job is green on `main` at the moment of merge — fix or delete flaky specs, do not
      mark them `.skip` and move on.
- [ ] Failure artifacts (report + container logs) are attached to the run.
- [ ] `docs/CONTRIBUTING.md` CI section lists the job.

## Tests

The ticket is the test. Verify by pushing a branch that deliberately breaks one guarded
behaviour (e.g. remove the auth layer from one router) and confirming the job goes red.
