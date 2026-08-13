#!/usr/bin/env bash
# Writes a throwaway .env for a CI run of the full stack. Secrets are generated
# per run and never leave the runner — nothing here is a credential you could
# reuse against a real deployment.
set -euo pipefail

cd "$(dirname "$0")/../.."

# Refuse to clobber a developer's .env: the generated POSTGRES_PASSWORD would no
# longer match the password already baked into their postgres volume.
if [ -f .env ] && [ -z "${CI:-}" ]; then
  echo "refusing to overwrite an existing .env outside CI" >&2
  exit 1
fi

rand() { openssl rand -hex 24; }

cat > .env <<ENV
JWT_SECRET=$(rand)
POSTGRES_PASSWORD=$(rand)
MINIO_ROOT_PASSWORD=$(rand)
ADMIN_EMAIL=admin@dev.local
ADMIN_PASSWORD=$(rand)
INSTANCE_NAME=Chat Systems CI
PUBLIC_URL=http://localhost:8080
CORS_ORIGINS=http://localhost:8080
# The suite logs in repeatedly from a single runner IP; the production defaults
# would throttle it into flakes.
LOGIN_ATTEMPTS_PER_EMAIL=1000
LOGIN_ATTEMPTS_PER_IP=1000
LOGIN_ATTEMPTS_WINDOW_SECS=900
# The SSO suite drives a real provider (the `oidc` service), so the flow it
# exercises is the redirect and the code exchange rather than a stub. The issuer
# is the compose service name because the same string has to resolve from inside
# the network and from the browser.
OIDC_ISSUER=http://oidc:8090/chat
OIDC_CLIENT_ID=chat-systems
OIDC_CLIENT_SECRET=dev-secret
OIDC_PROVISIONING=domain_allowlist
OIDC_ALLOWED_DOMAINS=dev.local
ENV

# The E2E job needs the admin password to drive the suite; hand it back through
# the step output rather than re-reading the file in every later step.
if [ -n "${GITHUB_ENV:-}" ]; then
  grep -E '^(ADMIN_EMAIL|ADMIN_PASSWORD)=' .env >> "$GITHUB_ENV"
fi

echo "wrote .env with generated CI secrets"
