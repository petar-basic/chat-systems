# CS-029 — Message edit history

**Wave:** 8 — Compliance
**Area:** backend/api · frontend
**Blocked by:** CS-018 (audit), CS-024 (message rendering)
**Blocks:** CS-030, CS-031
**Roadmap:** was M8

## Problem

`update_message` mutates the row in place; only `updated_at` changes. The previous text is
gone.

For a 70-person company that means an employee can silently rewrite what they said, days
later, and the record shows only that an edit happened — not what was changed. Any
investigation that starts with "what did this message originally say" has no answer, and
the data required to answer it was never captured.

The frontend shows an "edited" marker, which makes the gap more visible rather than less:
the product asserts that an edit occurred and cannot say what it was.

## Why before retention and export

CS-030 purges old data and CS-031 exports it. Both need to know whether edits are part of
the record. Introducing history afterwards means revisiting both.

## Approach

1. **Append-only history table:**
   ```sql
   CREATE TABLE message_edits (
       id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
       message_id      UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
       previous_content TEXT NOT NULL,
       edited_by       UUID NOT NULL REFERENCES users(id),
       edited_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
   );
   CREATE INDEX idx_message_edits_message ON message_edits(message_id, edited_at DESC);
   ```
   Mirror it for `conversation_messages` — DMs need the same treatment, and folding both
   into one table with a nullable pair of foreign keys repeats the ownership modelling
   problem CS-009 had to solve. Two tables, one shape.
2. **Write in the same transaction as the update.** `MessageRepo::update_message` becomes a
   transaction that inserts the pre-image and then updates. If the insert fails the edit
   fails — an edit that loses history is worse than a rejected edit.
3. **Who can read it.** Not everyone: showing every prior draft to every reader changes the
   product. Expose prior versions to workspace admins and to the message author, via
   `GET /api/messages/:msg_id/history`, gated through `authz` plus a role check, and audited
   with `AuditAction::MessageEditedByAdmin` when an admin reads somebody else's.
4. **Frontend.** The existing "edited" marker becomes a control for users who may view
   history, opening a diff panel. Everyone else sees the marker as today.
5. **Bound the growth.** An edit loop on a large message could accumulate. Cap stored
   versions per message (e.g. 50, dropping the oldest) and let CS-030's retention purge
   history alongside the message it belongs to.
6. **Deletion keeps the history.** A soft-deleted message retains its edits until retention
   removes both; the `ON DELETE CASCADE` above only fires on a hard delete from the purge
   job, which is the intended coupling.

## Acceptance

- [ ] Every edit writes a pre-image in the same transaction.
- [ ] Both channel messages and conversation messages are covered.
- [ ] Authors and workspace admins can read history; nobody else can.
- [ ] Admin reads of another user's history are audited.
- [ ] Version count per message is capped.

## Tests

`http_tests/messaging.rs`: edit three times, assert three ordered pre-images and that the
current content is not among them. Assert a non-admin, non-author read returns 403. Assert
a failed history insert rolls back the edit. Same set for conversations.
