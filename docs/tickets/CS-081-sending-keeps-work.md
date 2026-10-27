# CS-081 — Sending never loses work

**Wave:** 26
**Area:** frontend · backend
**Blocked by:** —
**Blocks:** —

## Problem

- A failed send keeps the row but *Retry* drops its attachments; a thread has no retry at
  all; a refetch while a row is failed removes it, and the text is gone.
- Attachments in the composer are cleared on switching channel while the draft text stays,
  and an upload that failed is dropped silently when the message is sent.
- A scheduled message that failed looks like it is still waiting; *Cancel* and *Reschedule*
  both answer an error on it.
- Forwarding a message quotes it, and the quote's `@name`, `@channel` and `@here` notify
  again.

## Approach

- Failed rows (channel and thread) keep their attachments and survive a refetch until sent
  or discarded; *Retry* sends the same files, *Discard* puts the text back in the composer.
- Attachments are kept per channel with the draft; a failed upload blocks *Send* with its
  reason until retried or removed.
- The scheduled list shows a failed message with its reason, *Send now* and *Remove*;
  deleting a failed row is allowed.
- A forward stores its source (`metadata.forwarded_from`) and the mention parser skips the
  quoted part.

## Acceptance

- [ ] Pulling the network during a send, with files, loses nothing in a channel or a thread.
- [ ] A failed scheduled message says so and can be sent or removed.
- [ ] Forwarding notifies only the people mentioned in the new text.

## Tests

Frontend: retry with files, refetch while failed, thread retry, channel switch with
attachments. API: delete a failed scheduled message; forward with mentions in the quote.
E2E: send with the network aborted.
