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

CI runs `cargo audit` against `backend/Cargo.lock` on every push. Exactly one
advisory is accepted and tracked in `backend/.cargo/audit.toml`, for a crate that is
present in the lockfile but **not compiled into either binary**:

- **RUSTSEC-2023-0071 (`rsa`)** — the Marvin timing side-channel in RSA key
  operations. `rsa` appears in `Cargo.lock` only as a dependency of `sqlx`'s MySQL
  driver, which this project never enables (it is postgres-only), so it is not in
  the build graph — `cargo tree -i rsa` is empty. Token signing/verification uses
  HMAC (`HS256`) via `EncodingKey/DecodingKey::from_secret`, and `jsonwebtoken` is
  built with the `aws_lc_rs` crypto provider (reusing the `aws-lc-rs` already
  compiled for `rustls`), so the pure-Rust `rsa` code path is never pulled in either.
  No fixed `rsa` version exists upstream; the entry can't be removed while depending
  on the `sqlx` facade crate.

Previously the ignore list also carried `RUSTSEC-2026-0104 / -0098 / -0099`
(`rustls-webpki` 0.101, via the AWS SDK's old `rustls` 0.21 connector). The SDK now
uses `default-https-client` (rustls 0.23 / `rustls-webpki` 0.103), so those crates
are gone from the tree and the stale ignores have been removed.

Review `backend/.cargo/audit.toml` whenever dependencies change — do not add
ignores without recording the reachability rationale here.
