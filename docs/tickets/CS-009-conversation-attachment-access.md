# CS-009 — Attachment access control for conversations

**Wave:** 1 — Access control
**Area:** backend/api
**Blocked by:** CS-002
**Blocks:** CS-020
**Audit finding:** S3 (HIGH)

## Problem

Files are uploaded before they are posted, so
[`upload_file`](../../backend/api/src/files/routes.rs#L56) writes the row with
`message_id: None`. The link is made afterwards by
[`link_attachments`](../../backend/api/src/messaging/routes.rs#L513), which scans the
message body for `/api/files/download/` keys and sets `files.message_id`.

`link_attachments` is called from exactly two places, both in the messaging feature:
[`send_message`](../../backend/api/src/messaging/routes.rs#L125) and
[`reply_to_thread`](../../backend/api/src/messaging/routes.rs#L332). The conversations
feature — 1:1 DMs and group DMs — never calls it.

So every file ever attached to a DM keeps `message_id = NULL` forever. And
[`require_file_access`](../../backend/api/src/files/routes.rs#L236) with a `NULL`
`message_id` requires only workspace membership:

```rust
if let Some(message_id) = record.message_id {
    if let Some(channel_id) = state.file_repo.channel_id_for_message(message_id).await? {
        require_channel_membership(state, channel_id, user_id).await?;
    }
}
Ok(())
```

Any member of the workspace can therefore download any file from any private DM. The only
thing standing in the way is that the storage key `{ws_id}/{uuid}/{filename}` is
unguessable — the access-control layer contributes nothing. That is security by
obscurity, and the key travels through message bodies, logs and backups.

The same hole covers abandoned drafts: a file uploaded and never posted is readable by
the whole workspace.

## Approach

`files.message_id` cannot express "belongs to a conversation message" because
conversation messages live in a different table. Model the owner explicitly.

1. **Migration** — add a nullable owner reference alongside the existing one and an
   ownership discriminator:
   ```sql
   ALTER TABLE files
     ADD COLUMN conversation_message_id UUID REFERENCES conversation_messages(id) ON DELETE SET NULL;

   CREATE INDEX idx_files_conversation_message ON files(conversation_message_id);

   ALTER TABLE files
     ADD CONSTRAINT files_single_owner
     CHECK (message_id IS NULL OR conversation_message_id IS NULL);
   ```
   Do not backfill — existing DM attachments cannot be attributed retroactively. They are
   covered by step 4 instead.
2. **Move `link_attachments` out of messaging.** It is now shared by two features, and a
   feature must not call another feature's route helper. Move `extract_file_keys` and the
   linking call into `files::service`, exposing:
   ```rust
   pub async fn link_to_channel_message(state, content, message_id, workspace_id, user_id)
   pub async fn link_to_conversation_message(state, content, message_id, workspace_id, user_id)
   ```
   Messaging and conversations both call it; the SQL stays in `files::repo`. Keep the
   existing `AND user_id = $4 AND message_id IS NULL` guard so pasting somebody else's
   file URL cannot re-parent their file.
3. **Call it from conversations.** `send_message` and `edit_message` in
   [`conversations/routes.rs`](../../backend/api/src/conversations/routes.rs#L227) link
   after a successful insert, mirroring the messaging handlers.
4. **Fail closed on unowned files.** `require_file_access` becomes:
   - `message_id` set → channel access via `authz::require_channel_access`;
   - `conversation_message_id` set → `authz::require_conversation_participant`;
   - neither set → **only the uploader may read it.** A file nobody has posted is a
     draft, and a draft belongs to its author. This closes the abandoned-upload hole and
     retroactively protects every existing DM attachment, since those all have both
     columns `NULL`.
   Step 4 is what makes this ticket safe to ship without a backfill; ship it in the same
   commit, not after.
5. **Scheduled messages** deliver through
   [`scheduled/executor.rs`](../../backend/api/src/scheduled/executor.rs#L58) and must
   link attachments too — they take the same two branches.

## Acceptance

- [ ] A DM attachment is downloadable by conversation participants and by nobody else.
- [ ] A workspace member who is not a participant gets 403, not the file.
- [ ] An uploaded-but-unposted file is readable only by its uploader.
- [ ] Attachments posted through scheduled messages are linked on delivery.
- [ ] `link_attachments` no longer exists in `messaging/routes.rs`.

## Tests

`http_tests/files.rs`: upload as A, post in a DM between A and B, download as B → 200, as
non-participant C → 403. Upload and never post, download as B → 403, as A → 200. Repeat
for a group DM. `http_tests/scheduled.rs`: schedule a message with an attachment, deliver
it, assert the file is linked and access follows the conversation.
