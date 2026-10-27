# CS-087 — A photo that shows everywhere

**Wave:** 26
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** —

## Problem

An account photo is uploaded as a file of the workspace that was open, so other workspaces
get 403 for it, and file retention deletes it with that workspace's old files. (Workspace
photos from CS-076 live in their workspace and are fine.)

## Approach

- Avatars get their own upload (`POST /users/me/avatar`) and storage prefix, served to anyone
  who shares a workspace with the person, and outside retention.
- A migration moves photos referenced by `users.avatar_url` into the new prefix.

## Acceptance

- [ ] An account photo shows in every workspace and survives retention.

## Tests

Upload, access from a second workspace and from a stranger, retention skip, the migration.
