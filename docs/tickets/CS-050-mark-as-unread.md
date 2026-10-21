# CS-050 — Mark as unread

**Wave:** 14 — the daily-use gaps a team arriving from Slack notices first
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** CS-051 (the unreads view should honour a channel marked unread)

## Problem

Reading a message marks it read, and there is no way back. "I'll deal with this later" is the
most common reason anyone in Slack uses *Mark unread*, and here the only workaround is
remembering.

## Approach

1. `POST /channels/{ch_id}/unread {message_id}` moves the member's read position to just
   before that message: `last_read_at` / `last_read_message_id` to the previous message,
   `unread_count` recomputed for the messages after it (the counters CS-026 introduced, so a
   single bounded count over one channel), `mention_count` likewise.
2. Announce it to the member's other tabs and devices with the frame the read path already
   sends, so the badge comes back everywhere at once.
3. The open channel must not immediately mark it read again: the client suppresses its
   auto-read until the member scrolls or sends.
4. Frontend: *Mark unread* in the message menu, and Alt+click on a message, as in Slack.
   Works for channels, DMs and thread replies (a thread reply marks its channel's position).

## Acceptance

- [ ] Marking a message unread restores the channel's badge with the right count and keeps
      it until the member reads past it again.
- [ ] Other tabs and devices update without a reload.
- [ ] Non-members get 403; a message from another channel gets 404.

## Tests

`http_tests` for the counter arithmetic (marking the first, a middle and the last message
unread) and the authorization matrix. A frontend test that the open channel does not
re-mark itself read.
