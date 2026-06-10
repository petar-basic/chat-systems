# Chat Systems

![CI](https://github.com/petar-basic/chat-systems/actions/workflows/ci.yml/badge.svg)
![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)

A self-hosted team chat platform — channels, threads, DMs, reactions, files, and
real-time presence — built with Rust and React. Run it on your own box, behind
your own domain, with no per-seat pricing and no third party holding your messages.

## What it is

Think of it as a small, self-hostable Slack alternative for a team that wants to
own its data. One instance serves multiple workspaces; users are invite-only; and
the whole thing runs from a single `docker compose` command behind automatic HTTPS.

It's also a deliberately well-engineered reference codebase: a stateless Rust API,
a horizontally-scalable WebSocket gateway, and a strictly-typed React SPA, with an
integration-test suite and a real CI pipeline.

## Features

- **Real-time messaging** — channels, threads, pins, reactions, editing, full-text search
- **Direct messages** — 1:1 conversations with reactions and read state
- **Presence & typing** — live, multi-tab and multi-node aware
- **File sharing** — uploads served through the authenticated API (local disk or S3/MinIO)
- **Multi-workspace** — one instance, many teams; the client can connect to several instances
- **Role-based access** — Instance Admin, Workspace Owner/Admin, Channel Admin, Member, Guest
- **Invite-only onboarding** — email invites, password reset, no open sign-up
- **Webhooks** — incoming (Slack-compatible `{"text":...}` → channel) and outgoing (SSRF-hardened, HMAC-signed)
- **Notifications** — in-app + native desktop, with mention highlighting and a favicon badge
- **Desktop app** — Electron build with native notifications, dock badge, and `chatsystems://` deep links

## Architecture at a glance

Two Rust binaries plus a React SPA. The API is stateless; the realtime gateway fans
messages out across nodes via Redis pub/sub — so both tiers scale horizontally.

| Component         | Technology                                                        |
|-------------------|------------------------------------------------------------------|
| **chat-api**      | Rust (Axum) — stateless REST API                                 |
| **chat-realtime** | Rust (Axum) — WebSocket gateway                                  |
| **Frontend**      | React 19, Vite, React Router, TailwindCSS, Zustand, TanStack Query |
| **Edge (prod)**   | Caddy — automatic HTTPS + reverse proxy                          |
| **Database**      | PostgreSQL 16                                                    |
| **Cache / PubSub**| Redis 7                                                          |
| **Storage**       | Local disk, or MinIO / S3                                        |

```
HTTP request → chat-api → PostgreSQL write → Redis PUBLISH
            → chat-realtime consumer → WebSocket push to subscribed clients
```

## Try it locally

```bash
cp .env.example .env          # then set JWT_SECRET, ADMIN_PASSWORD, POSTGRES_PASSWORD
docker compose --profile frontend up -d --build
ADMIN_PASSWORD=... ./seed.sh  # optional: a demo workspace + users
```

Open **http://localhost:8080** and log in with your `ADMIN_EMAIL` / `ADMIN_PASSWORD`.

Full setup — local development, production deployment with HTTPS and backups, and
the contribution workflow — lives in **[docs/CONTRIBUTING.md](docs/CONTRIBUTING.md)**.

## Desktop app

The desktop client is an Electron wrapper around the same web app. It connects to
whichever server you point it at — on first launch it asks for your instance URL
(e.g. `https://chat.yourcompany.com`), so one build works for every self-hoster.

**Download & install.** Grab the installer for your OS from the
[Releases](https://github.com/petar-basic/chat-systems/releases) page:

| OS      | File          |
|---------|---------------|
| macOS   | `.dmg`        |
| Windows | `.exe` (NSIS) |
| Linux   | `.AppImage` or `.deb` |

**Opening an unsigned build.** The published builds are *not* code-signed (no Apple
Developer ID or Windows Authenticode cert), so the OS shows a one-time warning you
can dismiss:

- **macOS** — right-click the app → **Open** → **Open**, or clear the quarantine flag:
  `xattr -dr com.apple.quarantine "/Applications/Chat Systems.app"`
- **Windows** — on the SmartScreen "Windows protected your PC" prompt, click
  **More info** → **Run anyway**.
- **Linux** — make the AppImage executable: `chmod +x "Chat Systems-*.AppImage"`.

**Cutting a release (maintainers).** Bump `version` in `frontend/package.json`, then
push a matching tag — `.github/workflows/release.yml` builds the macOS, Windows, and
Linux installers on their respective runners and uploads them to a GitHub Release:

```bash
git tag v1.0.0 && git push origin v1.0.0
```

**Building locally.** `cd frontend && npm run electron:build` produces installers for
the current OS only (into `frontend/release/`). A macOS `.dmg` can only be built on
macOS; use the release workflow to produce all three at once.

## Documentation

- **[Contributing & running](docs/CONTRIBUTING.md)** — dev setup, production deploy, coding standards, testing, CI
- **[Operations runbook](docs/RUNBOOK.md)** — backups, restore, upgrade/rollback, alerts
- **[Backend architecture](docs/backend.md)** — design rationale + REST/WebSocket API reference
- **[Frontend architecture](docs/frontend.md)** — design rationale + components, stores, and data flow
- **[Manual QA script](docs/manual-qa.md)** — end-to-end test checklist
- **[Roadmap & known limitations](docs/ROADMAP.md)** — what's deliberately not done yet, and why

## Known limitations

Honest about the edges, since this is a reference codebase:

- **Real-time delivery is at-most-once.** Events fan out over Redis pub/sub; a client
  that misses messages while disconnected recovers by refetching open views on reconnect,
  not by replaying a gap. Durable delivery (Redis Streams + a per-client cursor) is the
  next planned step — see the [roadmap](docs/ROADMAP.md).
- **Huddles use a WebRTC mesh**, which is great up to ~6–8 participants; large all-hands
  calls would need an SFU.
- **No SSO/2FA yet** — email + password only.
- **Desktop builds are unsigned** (no Apple Developer ID / Windows Authenticode cert).

The full prioritized list lives in **[docs/ROADMAP.md](docs/ROADMAP.md)**.

## License

[MIT](./LICENSE) © 2026 Petar Basic

---

## Support

If you find this project useful and are feeling generous, consider donating to **Svratište** — a day center in Belgrade providing support, meals, and shelter for people experiencing homelessness.

[![Facebook](https://img.shields.io/badge/Facebook-svratistebgd-1877F2?style=flat&logo=facebook&logoColor=white)](https://www.facebook.com/svratistebgd/?locale=sr_RS)
[![Instagram](https://img.shields.io/badge/Instagram-svratistebgd-E4405F?style=flat&logo=instagram&logoColor=white)](https://www.instagram.com/svratistebgd/)
[![Donate](https://img.shields.io/badge/Donate-cim.org.rs-FF6B35?style=flat&logo=heart&logoColor=white)](https://cim.org.rs/donacije/donacija/)

---
