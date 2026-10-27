# CS-080 — Threads that keep up

**Wave:** 26
**Area:** frontend · backend
**Blocked by:** CS-079
**Blocks:** —

## Problem

- The thread panel loads the first fifty replies and no more.
- `reply_count` only grows; a deleted reply still counts. An open thread does not show other
  people's reactions or deletions until it is reopened.
- When retention or an erasure removes a thread's parent, its replies drop into the channel
  timeline as if they were top-level messages.
- There is no *Also send to #channel*, and the thread composer cannot schedule.

## Approach

- Replies page from the newest backwards (infinite query, *Show earlier*).
- Deleting a reply decrements `reply_count` in the same statement and publishes the parent
  update; thread caches take message, reaction and delete events through the same patch as
  the channel.
- A removed parent stays as a tombstone ("This message was deleted"), so its replies stay a
  thread. Retention removes a thread as a unit once its last reply is past the period
  (decision to confirm before this part is built).
- *Also send to #channel* (`also_to_channel`, shown in the channel with a "replied to a
  thread" line) and scheduling from the thread composer.

## Acceptance

- [ ] A thread of 200 replies can be read to the top.
- [ ] Counts and live updates match what is in the thread.
- [ ] No reply ever shows up as a top-level message.

## Tests

Pagination; count after delete; retention on a thread with an old parent and a new reply;
erasure of a parent; also-to-channel delivery and notifications.
