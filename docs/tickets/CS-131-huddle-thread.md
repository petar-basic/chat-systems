# CS-131 — A huddle's own thread

**Wave:** 35 — huddles and clips (2026-11-13)
**Area:** backend · frontend · realtime
**Blocked by:** —
**Blocks:** —

## Problem

During a huddle, a link or a command is pasted into the channel itself, between unrelated
messages, or into a DM to one person. When the huddle ends nothing ties what was shared to
it. Slack gives every huddle a thread. Its chat panel writes there, and what was said stays
under the huddle's message in the channel.

## Approach

- **The thread is the `huddle_started` system message's.**
  - The huddle window gets a *Thread* panel that reads and writes replies to that message,
    with the composer of `ThreadPanel` (mentions, files, emoji, slash commands).
  - The huddle bar shows unread replies while the panel is closed.
- **When it ends**, the system message says how long the huddle lasted and who was in it,
  from `huddle_sessions` and the join history, and links its replies.
- **People invited from outside the channel:** `huddles/{id}/invite` admits people who
  cannot read the channel.
  - While they are in the huddle they may read and post in its thread and nowhere else in
    the channel. That is a new `authz` predicate keyed on the huddle's member set, not a
    channel membership.
  - When they leave, the thread is closed to them like the rest of the channel.
  - The panel tells them so when they join.
- **Notifications:**
  - Replies follow the thread rules of CS-080.
  - Everyone in the huddle follows its thread while there, and keeps following if they
    wrote in it.
  - Nobody else in the channel is notified by huddle chat.
- **A DM huddle** gets the same, on the DM's system message.

## Acceptance

- [ ] In a channel huddle, a link posted in the *Thread* panel shows under the huddle's
  message in the channel, for members who were not in the huddle too.
- [ ] After the huddle, the message shows its length, its participants and the replies.
- [ ] A person invited from outside the channel reads and writes the thread while in the
  huddle. After leaving they get `403` on it, and on every other message in the channel
  throughout.
- [ ] A participant who never wrote stops getting the thread's notifications after leaving.

## Tests

`http_tests`:
- the outside invitee's matrix: thread read and post while in the huddle, `403` after,
  `403` for the channel throughout;
- the end summary;
- thread following on join and on leave.

E2E: two people in a huddle, one posts in the panel, a third member sees it in the
channel.
