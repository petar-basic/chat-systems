# CS-095 — Push to the right device, from the right server

**Wave:** 27 — what the 2026-10-28 survey of 3.6.0 found
**Area:** realtime · backend · frontend
**Blocked by:** CS-092
**Blocks:** —

## Problem

- **A closed laptop holds back the phone.** The gateway notices a dead socket only after the
  90-second pong timeout, checked every 30 seconds, and every heartbeat until then renews the
  session's `active` key (60-second TTL). A laptop shut mid-use counts as active for two to two
  and a half minutes, and push to the phone is held back the whole time.
- **The push badge counts workspaces you left.** `unread_count_everywhere` counts every unread
  notification of the person; the app's badge sums only the workspaces they are in.
- **A desktop notification from a second server opens nothing.** The click calls
  `useOpenMessage`, which locates the message through the current server's API; a message on
  another server answers 404 and the toast says it was deleted.

## Approach

- A heartbeat renews `active` only if a pong arrived within the last interval; the client
  sends `active: false` on `pagehide` and when the page is hidden.
- The push badge counts notifications only in workspaces the person is a member of — the
  same number the app shows.
- Desktop notifications carry their instance; the click switches to that instance and
  locates the message there.

## Acceptance

- [ ] With the laptop shut, a mention reaches the phone within about a heartbeat.
- [ ] After leaving a workspace, the push badge and the app's badge agree.
- [ ] A notification from a second server opens its message.

## Tests

Realtime: activity not renewed without a pong. API: badge after leaving a workspace. Frontend:
notification click on another instance.
