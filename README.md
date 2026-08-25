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

- **Real-time messaging** — channels, threads, pins, reactions, editing with history, full-text search
- **Direct messages** — 1:1 and group conversations (up to nine) with reactions and read state
- **Mentions** — `@person`, `@group`, `@channel` / `@here` / `@everyone`, with custom emoji
- **Scheduled send** — queue a message for a channel or conversation and send it later
- **Huddles** — WebRTC voice/video with screen sharing (mesh, small groups)
- **Presence & typing** — live, multi-tab and multi-node aware
- **File sharing** — uploads served through the authenticated API (local disk or S3/MinIO)
- **Multi-workspace** — one instance, many teams; the client can connect to several instances
- **Role-based access** — Instance Admin, Workspace Owner/Admin, Channel Admin, Member, Guest
- **Invite-only onboarding** — email invites, password reset, no open sign-up
- **Webhooks, bots and slash commands** — incoming (Slack-compatible `{"text":...}` → channel), outgoing (SSRF-hardened, HMAC-signed), and synchronous `/commands`
- **Enterprise auth** — SSO (OIDC), TOTP with recovery codes, SCIM deprovisioning
- **Compliance** — audit log, retention policies, per-user export and erasure
- **Notifications** — in-app, desktop, and Web Push to a closed app, with mention highlighting and favicon/app-icon badges
- **Installable PWA** — standalone window, app-icon unread badge, desktop notifications

## Coming from Slack

Most of the daily surface is here: public and private channels, threads, DMs and group DMs
(up to nine people), reactions, pins, editing with history, search across channels and DMs,
file sharing, `@person` / `@group` / `@channel` / `@here` mentions, custom emoji, scheduled
send, huddles with screen sharing, slash commands, incoming and outgoing webhooks, SSO
(OIDC), TOTP, SCIM deprovisioning, retention policies and GDPR-style export.

What a team moving from Slack should expect to be missing or different:

| Missing / different | Where it stands |
|---|---|
| **No mobile app.** The UI is desktop-first; on a phone it is usable but not designed for it. The PWA installs and receives push. | Responsive layout is the next planned work ([CS-038](docs/tickets/CS-038-mobile-client.md)) |
| **No Slack import.** History does not come with you. | Planned ([CS-036](docs/tickets/CS-036-slack-import-export.md)) |
| **Huddles are peer-to-peer mesh** — comfortable to six or eight people, not a 30-person all-hands. No recording. | SFU planned ([CS-037](docs/tickets/CS-037-huddle-sfu.md)); recording deliberately out of scope |
| **No threads inside DMs.** Threads are a channel feature. | Not planned yet |
| **No message forwarding, saved items, or channel bookmarks.** | Not planned yet |
| **No manually set status.** Presence is derived from whether you hold a connection: online, away, offline. There is no "In a meeting 🍕". | Deliberate — a second, manual state nothing else reads |
| **Reminders have no UI.** The API and delivery exist; there is no `/remind`. | Not planned yet |
| **Search covers message text**, not file names or file contents. | Not planned yet |
| **No app directory, workflow builder, or shared channels across organisations.** | Not planned |

The trade for all of that: every message, file and audit row stays on hardware you control,
and the whole thing is one `docker compose` command. See
[docs/RUNBOOK.md](docs/RUNBOOK.md#what-self-hosting-actually-costs) for what running it
actually costs in machine and in time.

## Architecture at a glance

Three Rust binaries plus a React SPA. The API is stateless and the realtime gateway
fans messages out across nodes via Redis pub/sub, so both scale horizontally. Redis
pub/sub delivers to every subscriber, so the background consumers live in their own
single-replica process rather than inside each API replica — otherwise a second replica
would send every webhook and every notification twice.

| Component         | Technology                                                        |
|-------------------|------------------------------------------------------------------|
| **chat-api**      | Rust (Axum) — stateless REST API                                 |
| **chat-worker**   | Rust — background consumers (webhooks, reminders, notifications, scheduled messages) |
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

## Install as an app (PWA)

The web app is an installable PWA — no separate desktop build, no installers, no
unsigned-binary warnings, and it updates the moment you deploy:

- **Chrome / Edge** — open your instance, then click the install icon in the address
  bar (or menu → *Install app*). You get a standalone window, dock/taskbar icon, and
  an unread-count badge on the app icon.
- **Safari (macOS)** — *File → Add to Dock*.
- **iOS / Android** — *Share → Add to Home Screen* (the UI is desktop-first; mobile
  layout is not a goal yet).

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
- **Notifications arrive only while the app is open** (an installed PWA counts as
  open while its window runs). Web Push for closed-app delivery is planned.

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
