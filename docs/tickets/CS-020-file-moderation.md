# CS-020 — File moderation and attachment lifecycle

**Wave:** 4 — Governance
**Area:** backend/api
**Blocked by:** CS-009 (file ownership model), CS-018 (audit)
**Blocks:** CS-030
**Audit finding:** compliance (MEDIUM)

## Problem

Two gaps in the lifecycle of an uploaded file.

**No moderation path.** [`delete_file`](../../backend/api/src/files/routes.rs#L197) allows
deletion only by the uploader:

```rust
if record.user_id != auth.user_id {
    return Err(AppError::Forbidden("Can only delete your own files".into()));
}
```

A workspace admin cannot remove a file somebody posted — not a leaked credential, not an
inappropriate image, not a document posted by mistake. The only remedy is `psql` plus
manual object-store surgery.

**Deleting a message leaves its attachment.** `delete_message` soft-deletes the row; the
`files` record and the object in MinIO both survive, and with CS-009's ownership model the
file remains readable by whoever had access to the message. "Delete that message" does not
delete the thing the message was about.

## Approach

1. **Extend deletion rights.** Allowed to delete a file: the uploader; a channel moderator
   of the channel the file is attached to; a `WorkspaceRole::Admin`. Resolve through
   `authz` — the file's owning channel or conversation is already known from CS-009's
   columns — and audit every non-owner deletion with `AuditAction::FileDeleted` including
   the actor.
2. **Soft-delete files, do not hard-delete.** Add `deleted_at` to `files`, filter it out of
   every read (`find_by_id`, `find_by_storage_key`, `list_by_workspace_for_user`) and have
   the API stop serving the object immediately. The object itself is purged by the
   retention job in CS-030, not synchronously. Two reasons: an accidental deletion is
   recoverable for the retention window, and the object store call cannot fail the request.
   This also aligns files with how messages already behave.
3. **Cascade from message deletion.** `delete_message` and the conversation equivalent
   soft-delete the attached file rows in the same transaction. Reuse the ownership columns
   from CS-009 rather than re-parsing the message body — the body may have been edited
   since the file was linked.
4. **Editing a message that removes an attachment link** should soft-delete the orphaned
   file too. Compare the file keys before and after the edit, delete the difference. Keep
   this in `files::service` next to the linking logic from CS-009 so link and unlink live
   together.
5. **Orphan sweeper.** Files with no owner and no access for longer than a configured
   window (default 7 days) are abandoned drafts. The retention job in CS-030 purges them;
   this ticket only has to make them identifiable, which CS-009's `NULL`-owner state
   already does.
6. **Surface it.** The file list in the workspace panel gets a delete affordance for users
   who are allowed, and the message view shows "attachment removed" rather than a broken
   card.

## Acceptance

- [ ] A workspace admin and a channel moderator can delete another user's file.
- [ ] Every non-owner deletion is audited with the actor.
- [ ] Deleting a message makes its attachments inaccessible immediately.
- [ ] Editing an attachment out of a message releases the file.
- [ ] Deleted files are excluded from every read path.
- [ ] Objects are purged by the retention job, not on the request path.

## Tests

`http_tests/files.rs`: delete another user's file as admin → 200 and audited; as an
unrelated member → 403. Delete a message with an attachment, assert the download returns
404 for a user who previously had access. Edit a message to drop an attachment, assert the
file is released. Assert soft-deleted files never appear in `list_files`.
