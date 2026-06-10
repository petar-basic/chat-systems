# Security Policy

## Supported versions

| Version | Supported |
|---------|-----------|
| 1.0.x   | ✅         |

## Reporting a vulnerability

**Please do not open a public issue for security problems.**

Use GitHub's private vulnerability reporting on this repository:
**Security → Report a vulnerability**
(<https://github.com/petar-basic/chat-systems/security/advisories/new>).

Include steps to reproduce and the affected version/commit. You'll get a
response as soon as possible.

## Operator notes (self-hosting)

- Generate strong secrets — `JWT_SECRET` (`openssl rand -hex 32`), and the
  Postgres / MinIO passwords. The app refuses to start with the insecure
  default `JWT_SECRET`.
- Never commit `.env` (it is gitignored). If a secret is ever exposed, rotate
  it; rewriting git history alone does not make a leaked key safe.
- Serve over HTTPS in production (the provided Caddy setup does this), so auth
  cookies are sent with the `Secure` flag.

## Dependency advisories

CI runs `cargo audit` against `backend/Cargo.lock` on every push. A small set of
advisories are accepted and tracked in `backend/audit.toml`; each is reachable
only through code paths this project does not build or trust:

- **RUSTSEC-2023-0071 (`rsa`)** — pulled in only as part of `sqlx`'s optional
  MySQL driver. The build enables Postgres only, so `rsa` is never compiled into
  either binary (`cargo tree -i rsa` is empty). No fixed version exists upstream.
- **RUSTSEC-2026-0104 / -0098 / -0099 (`rustls-webpki` 0.101)** — pulled in by the
  AWS SDK's legacy TLS connector (`aws-smithy-http-client` → `rustls` 0.21), already
  at the latest `aws-sdk-s3`/`aws-config`. These are X.509 name-constraint /
  CRL-parsing issues on the TLS path to the object store; the S3/MinIO endpoint is
  operator-configured and trusted (MinIO is reached over the internal network), so
  the exposure is negligible. Drop the ignores once a newer AWS SDK moves off
  `rustls` 0.21.

Review `backend/audit.toml` whenever dependencies change — do not add ignores
without recording the reachability rationale here.
