# CS-094 — Addresses, workspace photos and hooks that keep working

**Wave:** 27 — what the 2026-10-28 survey of 3.6.0 found
**Area:** backend · frontend
**Blocked by:** CS-089
**Blocks:** —

## Problem

- **Email case.** Nothing lowercases an address except SSO. An invite to `Ana@Firma.com`
  provisions that exact spelling; the provider's `ana@firma.com` then misses it in
  `find_by_email`, so SSO either refuses (invite-only) or makes a second account, and the
  invite's address check fails. Signing in with the password and a different case fails too.
- **Workspace photos expire.** The photo for a workspace profile is uploaded through
  `/files/upload/{ws}` as an ordinary file, so that workspace's file retention deletes it with
  the old attachments — the problem CS-087 fixed for the account photo.
- **A stopped hook stays stopped.** A hook switched off automatically (its creator left or
  lost the channel) can only be deleted and made again; there is no route to turn it back on.

## Approach

- Addresses are stored lowercased. A migration lowercases `users.email` and adds a unique
  index on it; if two accounts differ only in case it stops and names them, and the RUNBOOK
  says how to merge or rename one first. Every lookup and every write lowercases its input.
- Workspace photos go through `POST /workspaces/{id}/profile/avatar`, stored under
  `avatars/{user_id}/{workspace_id}/` with no `files` row and readable by that workspace's
  members; replacing or clearing one deletes the old object. The worker moves existing ones on
  start, as CS-087 did, and `is_avatar` goes.
- `PATCH /hooks/{id}` with `is_active: true` turns a hook back on for someone allowed to make
  it, who becomes its creator; `disabled_reason` clears. *Integrations* offers *Turn on*.

## Acceptance

- [ ] An invite to a mixed-case address works through password registration and SSO, and
      sign-in works in any case.
- [ ] A workspace photo outlives that workspace's file retention.
- [ ] An admin turns a stopped hook back on without making it again.

## Tests

Migration on mixed-case and colliding rows. Auth and OIDC with mixed-case invites. Photo upload,
retention run, download as member and outsider, the move on start. Hook re-enable, ownership
and permission.
