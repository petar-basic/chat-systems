# CS-021 — Re-authorize scheduled messages at delivery

**Wave:** 5 — Correctness
**Area:** backend/api
**Blocked by:** ~~CS-002~~ ✅ shipped as `authz.rs`, ~~CS-004~~ ✅ shipped as `chat-worker`
**Blocks:** —
**Audit finding:** Q3 (authorization at time of use)

## Problem

[`deliver`](../../backend/api/src/scheduled/executor.rs#L47) inserts the message with no
check that the author may still post there:

```rust
let message = state.message_repo
    .create_message(channel_id, scheduled.user_id, &scheduled.content, None)
    .await?;
```

Authorization happened when the message was scheduled, possibly days earlier. Between
then and delivery the author may have left the company, been removed from the workspace,
been removed from the channel, or the channel may have been archived. The message posts
anyway — and after CS-007 makes removal actually cut access, this becomes the one path
that still writes on behalf of a removed user.

Same for the conversation branch: a scheduled DM delivers to a conversation the sender is
no longer a participant of.

The general rule the codebase already follows everywhere else — check at the moment of
the effect, not at the moment of the request — is missing here because the effect is
detached from the request.

## Approach

1. **Re-run the same predicate the interactive path runs**, at the top of each branch of
   `deliver`:
   - channel: `authz::require_channel_access(state, channel_id, scheduled.user_id)`
   - conversation: `authz::require_conversation_participant(state, conversation_id, scheduled.user_id)`

   Using the same helpers as the HTTP handlers is the point — a future change to
   visibility rules applies to scheduled delivery for free.
2. **Also check the channel is still writable**: not archived, and the workspace not
   soft-deleted. `require_channel_access` returns the `Channel`, so both are one field
   check away.
3. **Failure is terminal, not retried.** The existing `record_failure` writes the reason;
   ensure `sent_at` stays set (the row was claimed) and add a `failure` value the UI can
   render, e.g. `not_authorized`. Retrying an authorization failure will never succeed.
4. **Tell the author.** Emit a notification to `scheduled.user_id` — "your scheduled
   message to #x was not delivered: you no longer have access". A message that silently
   evaporates is worse than one that fails loudly. Reuse the notification pipeline rather
   than inventing a new channel.
5. **Cancel proactively where it is cheap.** When a user is removed from a channel or
   workspace (CS-007 events), cancel their pending scheduled messages for that scope in
   the same transaction. Delivery-time checking stays as the backstop; this just avoids
   the failure notification for the common case.
6. **The same reasoning applies to reminders** — `start_reminder_checker` fires a
   notification referencing a `channel_id` without checking the target still has access.
   Fix both in this ticket; they are the same class and live next to each other after
   CS-004.

## Acceptance

- [ ] A scheduled message from a user removed from the channel is not delivered.
- [ ] A scheduled message to an archived channel is not delivered.
- [ ] A scheduled DM from a removed participant is not delivered.
- [ ] Failures are recorded with a machine-readable reason and surfaced to the author.
- [ ] Removal cancels pending scheduled messages for that scope.
- [ ] Reminders apply the same check.

## Tests

`http_tests/scheduled.rs`: schedule into a channel, remove the author from the channel,
run `deliver_for_test`, assert no message row and a recorded failure. Repeat for workspace
removal, channel archival and conversation removal. Assert the author receives the failure
notification. Assert removal cancels pending rows.
