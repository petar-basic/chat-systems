# CS-121f — Read when you read it, and a *New* line that knows what you read

**Wave:** 32a — what the 2026-11-10 survey of 4.6.0 found
**Area:** frontend
**Blocked by:** CS-121e
**Blocks:** CS-121g

## Problem

4.6.0 made messages that arrive in the background wait until you come back
(`readOnScreen`, `readOnReturn`). Three paths go around that rule, and the *New* line and the
unread counts read state that nobody keeps current. Confirmed in the code:

- **Marked read while you are away (MSG-16).** Every reconnect invalidates all message queries
  (`backfillAfterReconnect`), and the refetch of the open channel's first page sends
  `mark_read=true` without asking whether the window is visible or focused (`useMessages`).
  `ThreadPanel` marks the thread read on every new reply whatever the focus, and `ThreadsView`
  marks a thread read as soon as a refetch shows it unread. Reading a channel also clears the
  pending mention email (`notifications/email.rs`). The desktop app sits in the tray on
  `#eng`, the Wi-Fi drops and comes back or the server is redeployed: `#eng` is read on the
  server, the phone shows nothing, the badge goes out and the mention email is never sent.
- **The *New* line comes from a cache that nothing updates (MSG-15).** `MessageList` takes the
  boundary from `useUnreadChannels` whenever that query has data, and then never uses the
  page's own `last_read_before`. That query (30 s `staleTime`) is not updated by
  `message.new` (the store is), by `applyReadState` (the store again), or by
  `channel.read_state` (it invalidates another key). With the window focused: Bo writes five
  messages in `#eng` while Ana is in `#general`, she opens `#eng`, the sidebar said 5, there is
  no line; or she opens `#eng` with 3 unread (the line is right), goes to `#general`, comes
  back, and the same line is drawn again above messages she has read, and the list scrolls to
  it. DMs too, so MSG-13's symptom survives.
- **Unread counts double after a replay (RT-1).** `sync.complete` only logs; a replayed
  `message.new` calls `bumpChannelUnread` with no regard for counts loaded since. Wave 27
  dropped this because it corrects itself; the survey adds that the client's position in a
  workspace only moves while it is subscribed to it (`ws.ts`), so coming back to a workspace
  replays everything since you left on top of freshly loaded counts: three messages while you
  were elsewhere show as six until the next refetch. That is every workspace switch, not only a
  laptop waking up.
- **A deleted last-read message loses the line (the rest of MSG-12).** `messageRows` places the
  line after the message whose id is the boundary; when that message is gone, nothing is
  placed, although the boundary carries its time.

## Approach

- One rule for every read: nothing is marked read unless `onScreen` says the place is visible
  and focused. The first page's `mark_read` asks it, the thread panel and the Threads view ask
  it, and `readOnReturn` covers threads as well as channels and DMs, so coming back marks what
  was shown.
- The *New* line's boundary is the `last_read_before` of the first page the server returns
  after the channel is opened — the fetch that marks it read — never a cache filled earlier.
  Until that page arrives, no line is drawn. `MessageList` stops reading `useUnreadChannels`.
- After `sync.complete` with `replayed > 0`, the unread queries are refetched and the store is
  hydrated from them, so a replay never counts on top of loaded counts.
- The line goes before the first message newer than the boundary's time when the boundary's
  message is not in the list.

## Acceptance

- [ ] A reconnect while the tab is hidden leaves the open channel unread on the server, and
  focusing the tab marks it read.
- [ ] A reply arriving in an open thread panel or the Threads view while the window is in the
  background stays unread until you come back.
- [ ] Without a reload: open A, another person writes in B, open B — the line sits above the
  first new message; open B again — there is no line.
- [ ] Leave a workspace, three messages arrive there, come back: the count is 3.
- [ ] With the last-read message deleted, the line is above the first message after it.

## Tests

Unit tests for the refetch's `mark_read`, the thread panel and Threads view effects in a hidden
tab (in the style of `readOnScreen.test.ts`); the boundary taken from the opening fetch; the
counts after hydration then replay of the same message; `messageRows` with a missing boundary.
E2E: the *New* line case above without a reload.
