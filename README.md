# Chat Systems

![CI](https://github.com/petar-basic/chat-systems/actions/workflows/ci.yml/badge.svg)
![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)

A self-hosted team chat platform — channels, threads, DMs, reactions, files, and
real-time presence — built with Rust and React. Run it on your own box, behind
your own domain, with no per-seat pricing and no third party holding your messages.
**[petar-basic.github.io/chat-systems](https://petar-basic.github.io/chat-systems/)** shows what
it looks like.

## What it is

Think of it as a small, self-hostable Slack alternative for a team that wants to
own its data. One instance serves multiple workspaces; users are invite-only; and
the whole thing runs from a single `docker compose` command behind automatic HTTPS.

It's also a deliberately well-engineered reference codebase: a stateless Rust API,
a horizontally-scalable WebSocket gateway, and a strictly-typed React SPA, with an
integration-test suite and a real CI pipeline.

## Features

**Messaging**
- Public and private channels, threads, DMs and group DMs (up to nine people)
- Reactions, pins, editing with history, forwarding, mark as unread
- `@person`, `@group`, `@channel` / `@here` / `@everyone`, and custom emoji
- Files and images attached to a message, with a gallery, local disk or S3 storage
- Link previews, with rich previews from GitHub, Jira, Figma, Google Drive and Confluence
- Scheduled send, and announcement channels where only moderators post

**Notifications**
- Per channel: everything, mentions only, or nothing; mute
- Keywords that notify you like a mention
- A notification schedule and do not disturb
- In-app, desktop and Web Push, with unread badges on the tab and app icon

**Finding things**
- Search across channels and DMs with filters, including file names and the text inside PDF,
  Word, Excel and PowerPoint files
- *All unreads*, *Threads*, *Later* (saved messages and reminders), keyboard shortcuts and
  a quick switcher
- Channel bookmarks, sidebar sections of your own, a status you set yourself, and *away*
  when you choose it or after ten idle minutes

**Structured work**
- Polls
- Lists: tables with typed columns kept in a conversation
- Workflows built without code: triggered by a schedule, a webhook, a reaction, someone
  joining or a shortcut; they post messages, ask with forms and add to lists

**Calls**
- Huddles: voice and video with screen sharing, for up to eight people

**Integrations**
- Incoming webhooks (Slack-compatible), signed outgoing webhooks, slash commands
- A Slack importer that brings your history with you

**Admin and compliance**
- Roles: Instance Admin, Workspace Owner and Admin, Channel Admin, Member, and Guests who
  see only the people they share a channel with
- Invite-only sign-up, SSO (OIDC), TOTP with recovery codes, SCIM deprovisioning
- Audit log, retention policies, workspace and per-person export, erasure
- App tokens and second factors sealed at rest under keys that rotate without anyone
  reconnecting
- Ed25519-signed sessions with rotating keys; the realtime gateway holds only public keys

**Running it**
- One `docker compose` command, automatic HTTPS, several workspaces per instance
- An installable PWA; the client can connect to several instances

## Coming from Slack

Most of the daily surface is here, from threads, huddles and *Later* to notification levels,
keywords, search inside files, polls, lists and workflows (see [Features](#features)). The
Slack importer brings people, channels and messages, threads included.

What a team moving from Slack should expect to be missing or different:

| Missing / different | Where it stands |
|---|---|
| **No native mobile app.** The web app is responsive and installs as a PWA with push, but there is nothing in an app store. | Deliberate — see [ROADMAP.md](docs/ROADMAP.md#wave-11--guest-containment-operational-readiness-and-mobile--shipped); React Native only if push reliability or call ringing proves to be the blocker |
| **Huddles are peer-to-peer mesh**, capped at eight people; not a 30-person all-hands. No recording. | SFU held for 4.0.0 ([CS-037](docs/tickets/CS-037-huddle-sfu.md)); recording deliberately out of scope |
| **Search reads text, not pictures.** Messages, file names and the text inside PDF, Word, Excel, PowerPoint and text files are searched; a scanned page or a screenshot is not (no OCR). | Deliberate — OCR needs a model beside the stack |
| **No app directory or shared channels across organisations.** | Not planned |

The trade for all of that: every message, file and audit row stays on hardware you control,
and the whole thing is one `docker compose` command. See
[docs/RUNBOOK.md](docs/RUNBOOK.md#what-self-hosting-actually-costs) for what running it
actually costs in machine and in time.

## Architecture at a glance

Three Rust binaries plus a React SPA. The API is stateless and the realtime gateway
fans messages out across nodes via Redis, so both scale horizontally. Background consumers
live in their own process rather than inside each API replica — otherwise a second API
replica would send every webhook and every notification twice. Durable events reach that
process through Redis Streams consumer groups, each event to exactly one replica, so
`chat-worker` scales too.

| Component         | Technology                                                        |
|-------------------|------------------------------------------------------------------|
| **chat-api**      | Rust (Axum) — stateless REST API                                 |
| **chat-worker**   | Rust — background consumers (webhooks, reminders, notifications, scheduled messages, email, event outbox relay) |
| **chat-realtime** | Rust (Axum) — WebSocket gateway                                  |
| **Frontend**      | React 19, Vite, React Router, TailwindCSS, Zustand, TanStack Query |
| **Edge (prod)**   | Caddy — automatic HTTPS + reverse proxy                          |
| **Database**      | PostgreSQL 16                                                    |
| **Bus**           | Redis 7 — streams with consumer groups, live pub/sub, presence, rate limits |
| **Storage**       | Local disk, or S3 (RustFS bundled)                               |

```
HTTP request → chat-api → PostgreSQL write + outbox row → Redis stream + publish
            → chat-realtime → WebSocket push          (clients replay the stream on reconnect)
            → chat-worker consumer groups → notifications, webhooks, history
```

## Try it locally

```bash
cp .env.example .env          # then set JWT_SIGNING_KEYS, JWT_PUBLIC_KEYS, SECRETS_KEYS, POSTGRES_PASSWORD
docker compose --profile frontend up -d --build
docker compose logs api | grep "setup token"
```

Open **http://localhost:8080**, paste the setup token, and create the first admin and
workspace. For demo users and channels, set `ADMIN_PASSWORD` in `.env` before the first start
instead and run `ADMIN_PASSWORD=... ./seed.sh`.

To run a published release instead of compiling, add to `.env`, with `VERSION` set to the
number of the [latest release](https://github.com/petar-basic/chat-systems/releases/latest):

```bash
BACKEND_IMAGE=ghcr.io/petar-basic/chat-systems-backend
FRONTEND_IMAGE=ghcr.io/petar-basic/chat-systems-frontend
VERSION=X.Y.Z
```

then `docker compose --profile frontend pull` and `up -d` without `--build`. What each
release changed is in **[CHANGELOG.md](CHANGELOG.md)**.

Full setup — local development, production deployment with HTTPS and backups, and
the contribution workflow — lives in **[docs/CONTRIBUTING.md](docs/CONTRIBUTING.md)**.

## Install as an app (PWA)

The web app is an installable PWA — no separate desktop build, no installers, no
unsigned-binary warnings, and it updates the moment you deploy:

- **Chrome / Edge** — open your instance, then click the install icon in the address
  bar (or menu → *Install app*). You get a standalone window, dock/taskbar icon, and
  an unread-count badge on the app icon.
- **Safari (macOS)** — *File → Add to Dock*.
- **iOS / Android** — *Share → Add to Home Screen*. The layout adapts to a phone, and
  an installed PWA is what receives Web Push on iOS.

## Documentation

- **[Contributing & running](docs/CONTRIBUTING.md)** — dev setup, production deploy, coding standards, testing, CI
- **[Operations runbook](docs/RUNBOOK.md)** — backups, restore, upgrade/rollback, alerts
- **[Backend architecture](docs/backend.md)** — design rationale + REST/WebSocket API reference
- **[Frontend architecture](docs/frontend.md)** — design rationale + components, stores, and data flow
- **[Manual QA script](docs/manual-qa.md)** — end-to-end test checklist
- **[Roadmap & known limitations](docs/ROADMAP.md)** — what's deliberately not done yet, and why
- **[Engineering history](docs/history.md)** — what each shipped wave changed and why

## Known limitations

Honest about the edges, since this is a reference codebase:

- **Real-time delivery is at-least-once, not exactly-once.** Every durable event is
  written through an outbox and a per-workspace Redis Stream, and a reconnecting client
  replays up to 1 000 events from its cursor; a longer gap, or one older than the stream
  keeps (10 000 entries, an hour), falls back to refetching open views, unread counts, pins
  and the sidebar. Consumers and clients are written to tolerate a duplicate.
- **Huddles use a WebRTC mesh**, which is great up to ~6–8 participants, so a huddle is
  capped at eight; large all-hands calls need the SFU planned for 4.0.0.
- **One instance is one trust domain.** SSO, TOTP and SCIM are per instance, not per
  workspace, and there is no cross-organisation sharing.
- **Search reads text, not pictures.** A scanned page or a screenshot is not searched (no
  OCR).

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
