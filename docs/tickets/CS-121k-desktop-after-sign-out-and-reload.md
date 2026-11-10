# CS-121k — The desktop app after signing out and after a reload

**Wave:** 32a — what the 2026-11-10 survey of 4.6.0 found
**Area:** frontend · desktop
**Blocked by:** —
**Blocks:** CS-121m

## Problem

- **A server you signed out of stays on the rail and in the badge (DSK-1).** Confirmed. The page
  reports its workspaces only from `useDesktopRail` (the one caller of `reportWorkspaces`),
  which lives on the workspace page. Signing out removes the instance and leaves that page, so
  nothing reports again. The shell drops a server's report only in `forget_server`, and
  `badge()` adds up every report it holds. With two servers, signing out of one that has five
  unread (or being signed out by a password reset) leaves 5 in the Dock and the tray, and its
  workspaces on the rail and on Cmd+1..9, each opening the sign-in page, until the app quits.
- **A click on an older notification after a reload runs the wrong handler (DSK-2, check
  first).** Click handlers are kept in the page under keys from a counter that starts at 0 on
  every load (`lib/notifications.ts`), and a banner stays up to an hour
  (`NOTIFICATION_LINGERS_MS`). After the reload that a new version asks for, clicking a banner
  from before it runs the handler of a newer notification with the same key, or nothing. Not yet
  shown: whether `notify-rust` delivers a late click on every OS.

## Approach

- Whenever the page has no signed-in instance it reports no workspaces and no current one, so
  the shell clears that server's rail entries and its share of the badge. A unit test holds
  this for sign-out.
- Notification keys carry an id chosen once per page load. A click whose key the page does not
  know opens the server and does nothing else. Check first that a late click reaches the page
  on macOS and Windows; the change is right either way, but the PR says what was seen.

## Acceptance

- [ ] Signing out of one of two servers removes its workspaces from the rail and its unread
  from the Dock and tray at once.
- [ ] Clicking a notification from before a reload never opens a different message.

## Tests

Unit tests: the empty report after sign-out; an unknown key after a reload.
