# CS-092 — Read state that matches what you read

**Wave:** 27 — what the 2026-10-28 survey of 3.6.0 found
**Area:** backend · frontend
**Blocked by:** CS-091
**Blocks:** CS-095

## Problem

- **Mark all read loads the whole channel.** `mark_workspace_read` sets `last_read_msg` to the
  channel's newest message, a thread-only reply included. The list never finds that id among
  channel messages, so it pages back to the start looking for it, and shows no *New* line.
- **A mention sent to the channel stays unread.** A reply with *Also send to channel* bumps the
  channel's counters, but its notification carries `thread_parent_id`, and reading the channel
  clears only notifications without one; the mention email goes out anyway.
- **A reply to a deleted message opens nothing.** A reply's notification points at its root;
  `GET /messages/{id}/location` uses `find_by_id`, which hides tombstones, so it answers 404
  and the click says the message was deleted.
- **A DM opens at an old *New* line.** `/conversations/{id}/read` moves `last_read_at` and the
  counters but not `last_read_msg`, which is what the line is drawn from.
- **Forwarding fires keyword alerts.** Mentions skip a forwarded message's quoted lines
  (`addressed_text`); keyword matching reads the whole text.

## Approach

- One rule for "newest message in a channel" — top level or sent to the channel — used by
  every write of `last_read_msg`. The list stops paging when the boundary is not in the
  channel and shows no line rather than loading everything.
- An *Also send to channel* reply's notification records it (`in_channel`), and reading the
  channel clears those too; the mention email checks the read state as it does now.
- `/location` finds a tombstoned root that still has replies, as `find_root` does.
- DMs read through `POST /channels/{id}/read` with the message id, like channels; the
  conversation read route goes.
- Keyword matching gets the addressed text, quotes of a forward removed.

## Acceptance

- [ ] After *Mark all read*, opening a channel loads one page and no *New* line.
- [ ] Reading the channel clears the mention from an *Also send to channel* reply, and no
      email follows.
- [ ] A reply notification under a deleted root opens the thread.
- [ ] A DM opens at the first message you have not seen.
- [ ] Forwarding a message with someone's keyword in the quote alerts nobody.

## Tests

API: workspace read with a thread-only newest reply; channel read and the mention email for an
in-channel reply; location of a tombstoned root; DM read through the channel route; keyword
on a forward. Frontend: boundary not in the channel stops paging. E2E: mark all read, then open.
