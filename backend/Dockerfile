# =============================================================================
# Chat Systems — Backend Dockerfile
# =============================================================================
# Builds two binaries: chat-api (monolith) and chat-realtime (WS gateway).
# Use ARG SERVICE to select which binary to run.
#
# Build:  docker build -t chatsys-backend .
# Run:    docker run -e SERVICE=chat-api chatsys-backend
# =============================================================================

# ── Stage 1: Build ──────────────────────────────────────────
FROM rust:1.93-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY shared/common/Cargo.toml     shared/common/Cargo.toml
COPY shared/events/Cargo.toml     shared/events/Cargo.toml
COPY api/Cargo.toml                api/Cargo.toml
COPY realtime/Cargo.toml           realtime/Cargo.toml

# Create dummy source files for dependency caching
RUN mkdir -p shared/common/src     && echo "" > shared/common/src/lib.rs \
 && mkdir -p shared/events/src     && echo "" > shared/events/src/lib.rs \
 && mkdir -p api/src               && echo "fn main() {}" > api/src/main.rs \
 && mkdir -p realtime/src          && echo "fn main() {}" > realtime/src/main.rs

RUN cargo build --release 2>/dev/null || true

# Copy real source code
COPY shared/ shared/
COPY api/ api/
COPY realtime/ realtime/
COPY migrations/ migrations/

# Touch all src files to invalidate cache for real sources only
RUN find shared/ api/ realtime/ -name "*.rs" -exec touch {} +

# Build both binaries
RUN cargo build --release

# ── Stage 2: Runtime ───────────────────────────────────────
# NOTE: pin by digest for reproducible builds once you have one, e.g.:
#   FROM debian:bookworm-slim@sha256:<digest>
# Leaving the tag here as no digest is pinned in this repo yet.
FROM debian:bookworm-slim

# ca-certificates for outbound TLS; curl for the HEALTHCHECK probe below.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && rm -rf /var/lib/apt/lists/*

# Run as an unprivileged, system (no-login) user instead of root.
RUN useradd -r -u 10001 -m -d /home/appuser -s /usr/sbin/nologin appuser

WORKDIR /app

# Copy binaries (readable + executable by appuser)
COPY --from=builder --chown=appuser:appuser /app/target/release/chat-api      ./chat-api
COPY --from=builder --chown=appuser:appuser /app/target/release/chat-realtime  ./chat-realtime

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
ENTRYPOINT ["/bin/sh", "-c", "exec ./${SERVICE}"]
