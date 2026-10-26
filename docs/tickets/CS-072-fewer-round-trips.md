# CS-072 — Fewer round trips on start and on switching channels

**Wave:** 24 — ready for a company
**Area:** frontend · backend
**Blocked by:** —
**Blocks:** —

## Problem

Opening the app makes about 30 API requests before the first channel is drawn, two of them
twice (`/users/me` and `/instance/info`). Switching channels makes eight more: members, pins,
bookmarks, lists, workflow shortcuts, channel webhooks, the read mark and the messages. On
the local stack that is under half a second; on a phone on a train it is the difference
between a channel that opens and one that fills in piece by piece.

## Approach

One PR, measured before and after with the same script.

- **No request twice.** `/users/me` and `/instance/info` are fetched once and shared through
  the query cache.
- **A channel's header in one request.** `GET /channels/{id}/context` answers the member
  count, pin count, bookmarks, lists, workflow shortcuts and whether the caller can manage
  webhooks, and seeds the existing query keys, so every component keeps reading what it reads
  today and a later change to one of them still invalidates just that key.
- **The read mark rides along** with the first page of messages rather than as its own call.
- A short `measure-requests` script (Playwright, counting requests to `/api` from sign-in to
  the first message, and per channel switch) records the numbers in the RUNBOOK.

## Acceptance

- [ ] No API request is made twice on start.
- [ ] A channel switch is at most two requests.
- [ ] The before and after numbers are in the RUNBOOK.

## Tests

Backend: `/channels/{id}/context` returns what each separate route returns, and refuses what
they refuse. Frontend: the seeded keys are the ones the components read. E2E: the request
count on a channel switch.
