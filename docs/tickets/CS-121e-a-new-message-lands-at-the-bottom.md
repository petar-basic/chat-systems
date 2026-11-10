# CS-121e — A new message lands at the bottom

**Wave:** 32a — what the 2026-11-10 survey of 4.6.0 found
**Area:** frontend
**Blocked by:** —
**Blocks:** CS-121f

## Problem

**After a channel has loaded more than one page, a new message is shown in the middle of the
history (MSG-14).** A channel's cache keeps its pages newest first — the first fetch is
`pages[0]`, each older page is appended after it — and `messagesOldestFirst` reverses the pages
for display. Every insert into a channel cache asks for `'lastPage'`, which `upsertMessage`
resolves to `cache.pages.length - 1`: the oldest page. Five places do it: the optimistic send
and its success in `useMessages.ts`, `message.new` in `wsQuerySync.ts`, *Also send to channel*
in `useThreads.ts`, and `messagePatch.ts`. Thread caches have the same layout and use
`'firstPage'` (`threadCache.ts`).

With one page loaded the two edges are the same page, which is why it has not shown up in the
E2E suite. With three — opening a channel with 120 unread fetches back to the *New* line,
scrolling up past 50 messages, opening an old permalink — your reply, and every message after
it, appears about a hundred messages up until a window refocus refetches the channel. It is
deterministic in the code and has not been reproduced in the app, so the ticket starts with the
reproduction.

## Approach

- First a failing test: a unit test through `upsertMessage` and `messagesOldestFirst` with two
  pages, and an E2E test that loads 60 messages, scrolls to the top, sends, and expects the
  message to be the last row.
- Inserts go into the newest page. If, once every caller is checked, all caches put their
  newest page first (channels and threads both do), `InsertEdge` and the parameter go and
  `upsertMessage` always uses `pages[0]`; otherwise the five channel call sites change to
  `'firstPage'`.

## Acceptance

- [ ] With several pages loaded, a sent message, an arriving message and an *Also send to
  channel* reply each appear as the last row, before and after the server confirms them.

## Tests

`messageCache.test.ts` with two pages through `messagesOldestFirst`; the E2E test above.
