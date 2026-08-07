# CS-026 — Unread counts without per-channel subqueries

**Wave:** 6 — Performance
**Area:** backend/api · frontend
**Blocked by:** —
**Blocks:** —
**Roadmap:** was M7

## Problem

The channel-list query runs an `EXISTS` subquery per channel to decide whether it has
unread messages. At 50+ channels with growing history the cost is paid on every sidebar
render and on every reconnect backfill, and it grows with message volume rather than with
channel count.

The list also cannot show *how many* unread messages there are — only whether there are
any — because a count would make the subquery materially more expensive.

## Approach

Denormalize, and keep the denormalized value honest with a reconciliation path.

1. **Add the counter to `channel_members`:**
   ```sql
   ALTER TABLE channel_members
     ADD COLUMN unread_count INT NOT NULL DEFAULT 0,
     ADD COLUMN last_read_message_id UUID,
     ADD COLUMN mention_count INT NOT NULL DEFAULT 0;
   ```
   `mention_count` separately, because a mention badge and an unread badge are different
   things in the UI and deriving one from the other requires the subquery this ticket is
   removing.
2. **Maintain on write, in the same transaction as the insert.** `create_message`
   increments `unread_count` for every channel member except the author, and
   `mention_count` for the mentioned subset the publisher already computes
   ([`expand_mentions`](../../backend/api/src/messaging/routes.rs#L607)):
   ```sql
   UPDATE channel_members
      SET unread_count = unread_count + 1,
          mention_count = mention_count + CASE WHEN user_id = ANY($2) THEN 1 ELSE 0 END
    WHERE channel_id = $1 AND user_id <> $3
   ```
   One statement, no N+1. It is a wide update on very busy channels; acceptable at this
   scale, and the index on `channel_members(channel_id)` covers it.
3. **`mark_read` resets to zero** and records `last_read_message_id`, replacing the
   current read-state comparison.
4. **Muted channels still count.** Muting affects notification delivery, not unread state —
   keep the two concerns separate or the badge will disagree with the message list.
5. **Push deltas over the socket.** The client currently refetches the channel list to
   update badges. Include the new counts in the `message.new` event so the sidebar updates
   without a round trip, and reconcile into the Query cache through `wsQuerySync` and the
   `QUERY_KEYS` factory as usual.
6. **Reconciliation job.** A denormalized counter drifts — a failed transaction, a manual
   database fix, a restore. Add a nightly job in the worker (CS-004) that recomputes counts
   for channels with activity in the last day and logs corrections. Cheap insurance, and it
   turns a class of "the badge is wrong" bug reports into a metric.
7. **Deleting a message decrements** for members who had not read past it. Soft delete
   makes this straightforward: compare against `last_read_message_id`.

## Acceptance

- [ ] The channel-list query contains no per-channel subquery.
- [ ] Unread and mention counts are exact after send, read, delete and backfill.
- [ ] Badges update from the socket without refetching the channel list.
- [ ] The reconciliation job corrects induced drift and logs it.
- [ ] Muting does not change unread counts.

## Tests

`http_tests/channels.rs`: send N messages, assert the counter for each member and zero for
the author; mark read, assert zero; delete an unread message, assert the decrement.
A test that deliberately corrupts a counter and asserts the reconciliation job fixes it.
Compare the denormalized value against the old subquery result over a seeded dataset in
one test so the two definitions are proven equivalent before the subquery is deleted.
