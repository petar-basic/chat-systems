# CS-093 — A failed send stays where you sent it

**Wave:** 27 — what the 2026-10-28 survey of 3.6.0 found
**Area:** frontend
**Blocked by:** —
**Blocks:** CS-096

## Problem

CS-081 kept failed messages in the channel and the thread panel. Three paths still lose them:

- **Threads view.** The card's composer sends with `mutate`, so `MessageInput` never sees the
  failure and does not put the text back; the failed copy goes to the thread panel's cache,
  which the card does not show.
- **A delivered message beside its failed copy.** If `message.new` arrives before the request
  fails (a timeout after the server saved it), the WS handler drops the unsent copy first and
  `onError` keeps it afterwards; `withUnsent` dedupes by id, and the delivered one has the
  server's.
- **Reload and scheduling.** The unsent store lives in memory, so a reload loses a failed
  message. Scheduling sends only files that finished uploading and then clears the tray, so
  one that failed is dropped without a word.

## Approach

- The Threads card sends with `mutateAsync` and shows the thread's unsent replies under it,
  with *Retry* and *Delete* as in the panel.
- Delivered client ids are remembered for the session; `keep` ignores one already delivered,
  and `withUnsent` dedupes on `client_message_id` as well.
- The unsent store persists per instance and user (local storage; files by their uploaded id
  only). Scheduling refuses a tray with a failed file, as sending does.

## Acceptance

- [ ] A reply that fails from the Threads view keeps its text and shows as failed there.
- [ ] A message never shows both delivered and failed.
- [ ] A failed message survives a reload; scheduling with a failed file says so.

## Tests

Unit: unsent store keep-after-delivery, persistence round trip, schedule with a failed file.
E2E: Threads view reply with the network down; reload with a failed message.
