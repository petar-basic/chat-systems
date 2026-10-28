# CS-097 — A workspace icon everyone sees, and that stays

**Wave:** 27 — what the 2026-10-28 survey of 3.6.0 found (follow-up to CS-094)
**Area:** backend · frontend
**Blocked by:** CS-094
**Blocks:** —

## Problem

- **Nobody else sees the icon.** *Workspace settings → General* uploads the icon through
  `POST /files/upload/{ws}` as an ordinary file and saves its URL with
  `PATCH /workspaces/{ws}`. The file belongs to no message, so `require_file_access` treats
  it as the uploader's draft and every other member gets 403. The sidebar and the switcher
  show them a broken image.
- **Retention can delete it.** It is a row in `files` like any old attachment, so it can go
  with them, the problem CS-087 fixed for account photos and CS-094 for workspace photos.
- **A replaced icon is never deleted.** A new icon or *Remove* leaves the old object in
  storage.

## Approach

- `POST /workspaces/{ws}/icon` (admin) takes a `file` field, shrinks it the way photos are,
  stores it under `icons/{workspace}/` outside `files`, and sets `icon_url`.
- The download route serves an `icons/{workspace}/` key to members of that workspace.
- Replacing or clearing the icon, including `PATCH` with `icon_url: ""`, deletes the old
  stored icon when it is one of this workspace's.
- On start, the worker moves an existing icon that is still an ordinary file into
  `icons/{workspace}/`, as `avatar::migrate` does for photos.
- *General* uploads to the new route.

## Acceptance

- [ ] An icon an admin sets shows for every member, and nobody outside the workspace can
  load it.
- [ ] File retention never removes it.
- [ ] Replacing or removing it deletes the old one from storage.
- [ ] An icon set before the upgrade is moved and keeps showing.

## Tests

API: upload, who sees it, it outlives retention, replace deletes the old object, migration.
Frontend: *General* uploads to the icon route.
