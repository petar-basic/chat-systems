# CS-022 — Scope the client-supplied message id to its conversation

**Wave:** 5 — Correctness
**Area:** backend/api
**Blocked by:** —
**Blocks:** —
**Audit finding:** S14 (LOW)

## Problem

Conversation `send_message` lets the client choose the message id and uses a unique
violation as an idempotency signal
([`conversations/routes.rs:227`](../../backend/api/src/conversations/routes.rs#L227)):

```rust
let id = req.id.unwrap_or_else(Uuid::new_v4);
let message = match state.conversation_repo.create_message(id, conv_id, auth.user_id, &req.content).await {
    Ok(msg) => msg,
    Err(ref e) if is_unique_violation(e) => state.conversation_repo
        .find_message(id).await?
        .ok_or_else(|| AppError::Internal("Message ID conflict".into()))?,
    ...
};
```

`find_message(id)` looks up by primary key alone. It does not verify the row belongs to
`conv_id`, or that the caller is a participant of whatever conversation it does belong to.
A caller who supplies an id that already exists elsewhere gets that message's full row
back — content, author, timestamps.

Exploitability is low: ids are v4 UUIDs, so the attacker must already know one, which
normally means they could already read it. But the handler's correctness does not depend
on that, and the same pattern would be a straightforward leak the moment ids become
predictable or are exposed in a URL. The retry path also silently returns *someone else's*
message as the result of your send, which is wrong regardless of who can see it.

The channel `send_message` path does not accept a client id, so it is unaffected.

## Approach

1. **Scope the lookup.** Add `find_message_in_conversation(id, conversation_id)` to
   `ConversationRepo` and use it in the conflict branch. A mismatch is not an internal
   error — return `AppError::Conflict("Message id already in use")`.
2. **Validate the shape of the supplied id.** Require a v4 UUID and reject nil. A client
   generating ids should not be able to pick `00000000-…`.
3. **Prefer an idempotency key over a primary key.** The cleaner model, and worth doing
   while the code is open: keep the primary key server-generated and add
   `client_message_id UUID` with `UNIQUE (conversation_id, client_message_id)`. The
   conflict branch then looks up by the pair, which is scoped by construction and cannot
   collide across conversations at all. The client already tracks its optimistic id, so the
   frontend change is small — it reads the returned id instead of assuming it.
   If the migration is not worth it now, step 1 closes the hole; note the decision here.
4. **Same treatment for reactions.** `add_reaction` relies on the `UNIQUE (message_id,
   user_id, emoji)` constraint, which is already correctly scoped — no change, but confirm
   it during review so the pattern is consistent.

## Acceptance

- [ ] Reusing an id from another conversation returns 409, never another conversation's
      message.
- [ ] Retrying the same send within the same conversation is still idempotent and returns
      the original message.
- [ ] Nil and non-v4 ids are rejected.
- [ ] The chosen approach (scoped lookup vs `client_message_id`) is recorded here.

## Tests

`http_tests/conversations.rs`: send in conversation A with id X; send in conversation B
with the same X as a different user → 409 and no content disclosed. Send twice with the
same id in the same conversation → one row, identical response. Send with a nil id → 400.
