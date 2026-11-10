# CS-121m — Notifications in the reader's language

**Wave:** 32a — what the 2026-11-10 survey of 4.6.0 found
**Area:** backend · frontend
**Blocked by:** CS-121k
**Blocks:** —

## Problem

**Text written by the server is always English (NOT-9).** Confirmed. The notification titles are
strings in `notifications/consumer.rs` (*You were mentioned*, *New reply in a thread*,
*Incoming huddle*, *Matched "…"*), the digest's subject is in `notifications/email.rs`, and the
backend never reads the language stored in the preferences. `NotificationsPanel` shows
`notification.title` as it came, so a person using the app in Japanese sees English titles in
*Activity*, in desktop banners, in push and in email, after six releases that translated the UI.

## Approach

- A notification carries its `kind` and the data its text needs (who, where, the keyword), and
  the client writes the title from its own translations, in *Activity* and in desktop banners.
  Nothing reads the stored `title` afterwards, so it goes, with no fallback kept for older
  clients.
- Push and email are written in the recipient's language from a small server-side catalog keyed
  the same way, falling back to English.
- New strings go into `en` only; the Japanese is the translator's (cyberlib), as for the rest of
  the UI. The PR lists the new keys for them.

## Acceptance

- [ ] With the language set to Japanese and the Japanese strings in place, *Activity*, a
  desktop banner, a push and a digest email show Japanese titles; without them, English.

## Tests

Consumer tests for `kind` and data on each notification type; the catalog's fallback; frontend
rendering per kind.
