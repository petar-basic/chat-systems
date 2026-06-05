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
