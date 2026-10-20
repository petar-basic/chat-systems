# =============================================================================
# Chat Systems — Backend Dockerfile
# =============================================================================
# Builds three binaries: chat-api (HTTP), chat-worker (background consumers)
# and chat-realtime (WS gateway). Use ARG SERVICE to select which one to run.
#
# Build:  docker build -t chatsys-backend .
# Run:    docker run -e SERVICE=chat-api chatsys-backend
# =============================================================================

# ── Stage 1: Build ──────────────────────────────────────────
# cargo-chef splits the build in two: `cook` compiles every dependency from a
# recipe derived from the manifests alone, so that layer is reused until
# Cargo.lock changes; only the workspace crates are rebuilt on a source change.
FROM lukemathwalker/cargo-chef:0.1.78-rust-1.98-bookworm AS chef

WORKDIR /app

RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

# No database is reachable during an image build. Every query is a
# compile-time-checked sqlx macro, so the build reads the committed .sqlx/
# cache instead (`cargo sqlx prepare`, see docs/CONTRIBUTING.md).
ENV SQLX_OFFLINE=true

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN cargo build --release

# ── Stage 2: Runtime ───────────────────────────────────────
# NOTE: pin by digest for reproducible builds once you have one, e.g.:
#   FROM debian:bookworm-slim@sha256:<digest>
# Leaving the tag here as no digest is pinned in this repo yet.
FROM debian:bookworm-slim

# ca-certificates for outbound TLS; libssl3 because web-push links OpenSSL
# dynamically; curl for the HEALTHCHECK probe below.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3 curl \
 && rm -rf /var/lib/apt/lists/*

# Run as an unprivileged, system (no-login) user instead of root.
RUN useradd -r -u 10001 -m -d /home/appuser -s /usr/sbin/nologin appuser

WORKDIR /app

# Copy binaries (readable + executable by appuser)
COPY --from=builder --chown=appuser:appuser /app/target/release/chat-api      ./chat-api
COPY --from=builder --chown=appuser:appuser /app/target/release/chat-worker    ./chat-worker
COPY --from=builder --chown=appuser:appuser /app/target/release/chat-realtime  ./chat-realtime
COPY --from=builder --chown=appuser:appuser /app/target/release/chat-import    ./chat-import

# Copy migrations (used by chat-api at startup)
COPY --from=builder --chown=appuser:appuser /app/migrations/ ./migrations/

# Local storage path (LOCAL_STORAGE_PATH) — must be writable by appuser when the
# Local storage backend is in use. Declared as a volume so docker-compose can
# mount a persistent volume here; ensure that volume is appuser-owned (uid 10001).
RUN mkdir -p /data/files && chown -R appuser:appuser /data /app
VOLUME ["/data/files"]
ENV LOCAL_STORAGE_PATH=/data/files

USER appuser

# SERVICE env var selects which binary to run (set in docker-compose)
ARG SERVICE=chat-api
ENV SERVICE=${SERVICE}

# Liveness probe — the app exposes /livez on $PORT (defaults to 3000).
HEALTHCHECK --interval=30s --timeout=3s --start-period=20s --retries=3 \
  CMD curl -fsS "http://127.0.0.1:${PORT:-3000}/livez" || exit 1

# Exec form so SIGTERM reaches the binary directly (graceful shutdown).
# /bin/sh -c is still required to expand ${SERVICE}; `exec` replaces the shell.
# `"$@"` with the `--` placeholder forwards any arguments, which is what lets
# `docker compose run -e SERVICE=chat-import api --workspace … --export …` work.
ENTRYPOINT ["/bin/sh", "-c", "exec ./${SERVICE} \"$@\"", "--"]
