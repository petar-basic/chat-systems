# CS-098 — A project people can find

**Wave:** 28 — what 4.0.0 needs (2026-10-29 feedback on 3.7.0)
**Area:** docs · ops
**Blocked by:** —
**Blocks:** —

## Problem

- **The GitHub page says nothing.** The repository has no description, no website and no
  topics, so it does not come up in GitHub search or topic pages, and a visitor sees a file
  list before they learn what the project is.
- **There is nowhere to send people.** No page shows what the app looks like or what it
  does; the README opens with a quick start.
- **The README names an old release.** The image example says `VERSION=3.3.0` while 3.7.0 is
  out, and it goes stale again with every release.

## Approach

- **A homepage** in `site/`: one static page, no build step, no framework. What it is (a
  self-hosted Slack alternative), screenshots of the app in light and dark taken from a
  seeded instance, the features that matter to a team (channels and threads, huddles, search
  inside files, SSO and SCIM, Slack import, retention and export, connected apps and bots),
  how to run it in three commands, and links to the docs, releases and the repository.
- **Published with GitHub Pages** from `.github/workflows/pages.yml` on every push to `main`
  that touches `site/`, at `https://petar-basic.github.io/chat-systems/`.
- **The repository's About**: a one-sentence description, that homepage as the website, and
  topics (`chat`, `slack-alternative`, `self-hosted`, `rust`, `axum`, `react`, `webrtc`,
  `team-chat`, `postgresql`, `redis`, `docker-compose`).
- **The README** points `VERSION` at the latest release instead of naming one, and the
  release checklist in `docs/CONTRIBUTING.md` says which documents name a version.

## Acceptance

- [ ] The repository has a description, a website and topics.
- [ ] The homepage is live, readable on a phone, and has a working light and dark theme.
- [ ] No document tells a reader to run a release older than the latest.

## Tests

The Pages workflow builds on a pull request without deploying. Manual: the page on desktop
and phone, every link.
