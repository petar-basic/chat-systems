# CS-091 — An archived channel is read-only all the way

**Wave:** 27 — what the 2026-10-28 survey of 3.6.0 found
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** CS-092

## Problem

CS-085 closed posting, reacting, scheduling and incoming webhooks in an archived channel.
The rest still writes:

- editing a message (`PATCH /messages/{id}`), pinning and unpinning, voting in a poll, adding
  a member to the channel and starting a huddle in it all skip the archive check;
- an archived **private** channel has no way back: the sidebar leaves out archived channels
  and *Browse channels* lists public ones only, so nobody reaches its *Unarchive* bar.

## Approach

- Every writer that takes a channel goes through `require_not_archived`: message edit and
  delete, pin and unpin, poll vote and close, add and remove member, huddle start, bookmarks,
  and anything else the audit of `messaging/`, `polls/`, `huddle/`, `workspace/` turns up.
  The client hides those controls on an archived channel.
- *Browse channels* lists the archived private channels the caller is a member of, with
  *Unarchive* for those allowed to, under the archived public ones.

## Acceptance

- [ ] Nothing in an archived channel changes until it is unarchived.
- [ ] A member of an archived private channel can find it and, if allowed, unarchive it.

## Tests

API: each writer answers 403 on an archived channel. Browse: archived private channel for a
member and not for an outsider. E2E: archive a private channel, find it in Browse, unarchive.
