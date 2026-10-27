# CS-084 — Reconnecting, presence and new versions

**Wave:** 26
**Area:** frontend · realtime · ops
**Blocked by:** —
**Blocks:** —

## Problem

- The socket does not reconnect on `online` or `visibilitychange`, so after a laptop sleeps
  it can wait thirty seconds.
- Presence is computed once per connection: after switching workspace, a person shows offline
  there until they reconnect.
- Replay keeps 1,000 events; the README says 10,000. When the gap is too big, the fallback
  refetch does not refresh some unread counts and pins.
- After a deploy, an open tab or the installed app runs old JavaScript: lazy pages and the
  emoji picker fail to load their chunks.

## Approach

- Reconnect at once on `online` and when the tab becomes visible with a dead socket.
- Presence is announced per workspace the connection subscribes to, on switch too.
- Make the README and the replay cap agree (decide the number from Redis memory), and have the
  gap fallback invalidate unread, pins and the sidebar.
- `index.html` is served `no-cache`; a failed chunk import (`vite:preloadError`) reloads once;
  a *New version* banner appears when `/version.json` changes; old chunks are kept for one
  release.

## Acceptance

- [ ] Waking a laptop reconnects within a second or two.
- [ ] Switching workspace shows the person online there at once.
- [ ] An open tab survives a deploy.

## Tests

Frontend unit tests for the reconnect triggers and the preload reload; realtime test for
presence on subscribe; E2E for the banner.
