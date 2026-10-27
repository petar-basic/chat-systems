# CS-082 — Unread, Activity and the badge agree

**Wave:** 26
**Area:** frontend · backend
**Blocked by:** CS-079
**Blocks:** —

## Problem

Unread and Activity follow different rules for channels and threads: a mention in the
channel already open still shows a badge, and reading a thread does not clear its Activity
row in the same way reading a channel does (to be confirmed case by case). The app icon
badge alternates between one workspace's count (from push) and the total of all of them
(from the app). A mention from workspace B can be added to the open workspace's unread list.
A channel always opens at the bottom, with no *Jump to first unread*, and the unread divider
is missing on a cold load.

## Approach

- One rule, written down in `docs/frontend.md`: a channel notification clears when the
  channel is read, a thread notification when the thread is read; the message open on screen
  counts as read. Verify the matrix (channel, DM, thread × open, other channel, other
  workspace) and fix what does not follow it.
- The badge is always the total across workspaces; push sends the total too.
- Realtime unread updates check the workspace before touching a list.
- Opening a channel with unread messages lands on the first one with the divider, and
  *Jump to first unread* shows while it is above the view.

## Acceptance

- [ ] Every cell of the matrix clears the badge and the Activity row together.
- [ ] The app badge never jumps between two numbers.
- [ ] A channel with unread messages opens on the first of them.

## Tests

Unit tests for the rule, E2E for the matrix's main cells and for the first-unread landing.
