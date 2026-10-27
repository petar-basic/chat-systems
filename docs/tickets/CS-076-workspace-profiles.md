# CS-076 — A profile and notification settings of your own in each workspace

**Wave:** 25 — one person, several workspaces
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** —

## Problem

A person's name, photo, bio and status live on the account (`users`), so they are the
same in every workspace on the instance. Setting *On holiday* in a hobby workspace shows it
at work too, and a custom emoji in a status shows as `:name:` in every workspace that does not
have it. Pausing notifications, the notification schedule and mention emails are on the
account as well, so pausing one workspace silences all of them.

Slack keeps all of these per workspace. What stays with the account is what belongs to the
person or the device: email, password, second factor, SSO, time zone, theme, sound and
desktop notifications, and push subscriptions.

Two tables from the first schema, `user_preferences` and `bookmarked_messages`, are read by
nothing; *Later* replaced the second.

## Approach

### Part 1 — the profile (backend and frontend)

- Migration 73: `workspace_members` gains `display_name`, `avatar_url`, `bio`,
  `status_emoji`, `status_text`, `status_expires_at`. A live status on the account is copied
  into every workspace the person is in, without a custom emoji the workspace lacks; then
  `users` loses the three status columns, and the two unused tables are dropped.
- Name, photo and bio in a workspace fall back to the account's when they are not set, so a
  name changed by SCIM or SSO still reaches every workspace nobody customised. The status
  has no fallback.
- `GET`/`PATCH /workspaces/{id}/profile` read and change one's own name, photo and bio there,
  with an empty value going back to the account's. `PUT`/`DELETE
  /workspaces/{id}/profile/status` replace `PUT`/`DELETE /users/me/status`. A custom emoji in
  a status must exist in that workspace.
- Every read of a person inside a workspace uses the workspace profile: the member list
  (which gains `bio`), push and email names, a workflow's mention of someone, list columns,
  the audit log and the Slack import's user mapping. An avatar set in a workspace is served
  like an account one, and erasing a person clears their workspace profiles too.
- *Edit profile* edits the current workspace's profile and says so; an empty name, photo or
  bio shows the account's. *Preferences → Account* edits the account's name, photo and bio.
  The sidebar's own row, the hover card (which shows the bio) and every other place read the
  member list. The status emoji picker offers the current workspace's emoji.

### Part 2 — notifications (backend and frontend)

- `notification_schedules` and the DND columns (`dnd_until`, `dnd_override_until`) move to
  the workspace member, and `mention_emails` with them; the migration copies today's values
  into every workspace the person is in.
- `notifications_paused_until(user, now)` becomes per workspace, and every caller (push,
  email digests, the member list, the notification stream) passes the workspace.
- `/notifications/dnd`, `/notifications/schedule` and the mention email setting take the
  workspace in the path. *Preferences → Notifications* names the workspace it changes, and
  the pause menu pauses the current workspace.

## Acceptance

- [ ] Different statuses in two workspaces, each seen only in its own.
- [ ] A name, photo or bio set in one workspace does not change another; one not set shows
      the account's.
- [ ] Pausing notifications or a schedule in one workspace leaves the others notifying.
- [ ] After the upgrade nobody has lost a status, a pause or a schedule.

## Tests

Backend: the migration's copy, with a custom-emoji status kept only where the emoji
exists; the profile routes, their fallback and their validation, including a status with a
custom emoji the workspace lacks; the member list in two workspaces; a guest's view; names in
push, workflows, lists and the audit log; avatar access; erasure. Part 2: pause, schedule
and email per workspace, and the digest and push paths honouring only their own workspace.
Frontend: *Edit profile* per workspace with its fallbacks, *Preferences → Account*, the
status picker. E2E: two workspaces, two statuses.
