# CS-099 — Away, on purpose and when idle

**Wave:** 28 — what 4.0.0 needs (2026-10-29 feedback on 3.7.0)
**Area:** backend · realtime · frontend
**Blocked by:** —
**Blocks:** —

## Problem

Presence is only ever derived from a socket. Someone is green while any tab is open, even on a
laptop left on a desk overnight, and grey the moment it closes. There is no way to say *I am
away* while staying signed in, so a person heads-down or in a meeting looks reachable, and the
green dot stops meaning anything. The gateway already knows when a person stops using the app
(`active:user:{id}`, CS-095) but uses it only to route push.

## Approach

- **Three states**: `active`, `away` and `offline`. `presence.changed` and the presence
  snapshot carry the state, not a boolean.
- **Away on purpose.** *You → Set yourself away* (and `/away`) sets `users.manual_away`; it
  holds across devices and restarts until the person sets themselves active again, as in
  Slack. `PUT /me/presence { away }`.
- **Away when idle.** With no manual away, a person whose every connection has been idle for
  ten minutes (`activity` frames, CS-095) shows away and turns active on the next input.
- **Who sees it**: the dot everywhere a person is shown (sidebar, member list, profile, DM
  header, mention popover) is filled when active, hollow when away, grey when offline, with
  a text label for screen readers.
- Away changes nothing about delivery: notifications, push and DND keep their own rules.

## Acceptance

- [ ] Setting yourself away shows you away to everybody in every workspace, on every device,
  until you set yourself active.
- [ ] Ten minutes without input shows you away; the next keystroke shows you active.
- [ ] Closing the last tab still shows you offline.

## Tests

API: the route, persistence across a reconnect. Realtime: the state on the snapshot and on
`presence.changed`, idle to away and back, manual away surviving activity. Frontend: the menu
item, the dot's three states and its label.
