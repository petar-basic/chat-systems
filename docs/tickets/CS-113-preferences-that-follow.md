# CS-113 — Preferences that follow you, and how messages look

**Wave:** 30 — make it yours (2026-11-04)
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** CS-114 · CS-115

## Problem

*Preferences* has four tabs where Slack has a dozen, and most of what a person expects to
set about how the app looks and behaves for them is missing. The one display setting there
is, the colour mode, lives in the browser's storage, so it is lost on another browser and
in the desktop app.

## Approach

- One place for a person's own settings across workspaces: a `preferences` JSON column on
  `users`, read with the session and written through one route that takes only known keys,
  each with its type and default. Unknown keys are refused, so the column cannot fill up with
  whatever a client sends.
- The colour mode moves there. The value in the browser's storage is still read on start, so
  the page does not flash the wrong mode before the session arrives, and it is written to
  the server once.
- A *Messages & media* tab:
  - *Clean* or *Compact*: compact puts the name and time on the line of the message and
    tightens the spacing;
  - names shown as the display name or the full name;
  - images and link previews shown in the message, or collapsed until clicked.
- These settings are the same in every workspace, as in Slack. What differs per workspace
  stays on `workspace_members` (notifications, profile, the theme of CS-111).

## Acceptance

- [ ] The colour mode and the *Messages & media* settings are the same after signing in on
  another browser and in the desktop app.
- [ ] Compact mode shows the name and time on the message's line in channels and threads.
- [ ] With previews collapsed, an image or a link preview opens only when clicked.
- [ ] A request with an unknown preference key is refused.

## Tests

Backend: defaults for a new account, a known key saved, an unknown one refused. Frontend:
the message row in both densities, the collapsed preview. E2E: switch to compact, reload.
