# CS-079 — Opening a message lands on it

**Wave:** 26
**Area:** frontend · backend
**Blocked by:** —
**Blocks:** CS-080

## Problem

A push, an Activity row, a desktop notification or a search hit opens
`/app/{ws}/{channel}/{message}`. The router only knows the channels in the sidebar list, so
a DM (a conversation, not in that list) or a channel the person has not joined redirects to
`#general`, and a thread reply, which is not in the channel timeline, is scrolled to
forever. Push payloads carry no `thread_parent_id`. Search shows twenty results with no
*More*, though the API pages.

## Approach

- `GET /messages/{id}/location` → `{ workspace_id, channel_id, kind: channel | dm | group_dm,
  thread_parent_id?, joined }`, with the caller's read access checked.
- One client resolver, `openMessage(id)`, used by Activity, desktop notifications, the
  service worker (via the message route) and search: a DM opens the conversation, a reply
  opens the thread scrolled to it, an unjoined public channel opens its preview with *Join*.
  The message route itself resolves through it, so old links work.
- Push and the notification frame carry `thread_parent_id`.
- Search gets *Show more* (offset paging) for messages and files.

## Acceptance

- [ ] From each entry point a DM, a thread reply and an unjoined channel open on the message.
- [ ] Search past twenty results.

## Tests

API: location for each kind and a 404 for an unreadable message. E2E: Activity → DM, push
route → thread reply, search → unjoined channel.
