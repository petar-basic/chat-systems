# CS-070 — Workspace admin in one place, with the settings that have no screen

**Wave:** 24 — ready for a company
**Area:** frontend · backend
**Blocked by:** —
**Blocks:** CS-071, CS-075

## Problem

An admin's work is spread over nine entries of the workspace menu (*Members*, *Settings*,
*Apps*, *Integrations*, *Slack import*, *Custom emoji*, *User groups*, *Audit log*, and
*Workflows* for everyone), each a narrow right panel. *Settings* saves with a button while
*Preferences* saves as you change it.

Worse, part of what a company needs exists only as an API:

- the retention policy (`GET`/`PATCH /workspaces/{id}/retention`),
- workspace and per-person exports (`POST /workspaces/{id}/exports`, `POST /users/{id}/exports`,
  `GET /exports/{id}`; nothing lists past ones),
- erasing a person's data (`DELETE /admin/users/{id}/data`),
- SCIM tokens (`/admin/scim/tokens`: list, mint, revoke, rotate), mounted outside the OpenAPI
  router, so the typed client cannot even call them.

An admin who does not read the RUNBOOK does not know they exist, and one who does needs
`curl` and a token to use them.

## Approach

Three parts, each its own PR.

### 1. One dialog

- **Workspace admin** replaces the admin entries of the menu: a large `Modal` with a list on
  the left and one section at a time, built like *Preferences* (CS-059) and saving the same
  way, as you change a setting. Sections: *General* (name, description, icon, link previews,
  who builds workflows), *Members* (with invitations), *User groups*, *Custom emoji*, *Apps*,
  *Integrations*, *Slack import*, *Audit log*. Each is the existing panel's content moved,
  not rewritten.
- The menu keeps *Workflows* (everyone builds), *Invite people* and *Workspace admin* (admins
  only). A deep link, `?admin=<section>`, opens a section, so the RUNBOOK and notifications
  can point at one.
- Deleting the workspace stays a danger zone at the bottom of *General*, behind the existing
  confirmation.

### 2. The settings that have no screen

- **Data retention** (owner only, as the API is): keep messages and files forever or for N
  days, with the dry-run count the worker already logs shown as "about N messages would be
  removed now" before saving.
- **Exports**: start a workspace export, see past ones with their status and download them
  (a new `GET /workspaces/{id}/exports`); an admin starts a person's export from their row in
  *Members*.
- **Erase a person's data**: from their row in *Members*, behind a confirmation that names
  what goes and needs their name typed.
- **SCIM** (instance admins): mint, list, rotate and revoke tokens; a minted token is shown
  once. The token routes move onto the OpenAPI router so the client is typed.

### 3. Retention per channel

The workspace policy is one number for every channel, so a company keeps `#random` as long
as `#legal`. Slack lets an owner allow a channel its own period.

- `retention_policies` gains `channel_overrides` (default off), set by the owner beside the
  policy.
- A new `channel_retention` table (`channel_id` primary key, `message_days`, `NULL` meaning
  forever, `updated_by`, `updated_at`). A channel moderator sets or clears it in *Channel
  settings* (CS-074 later moves it to the *Settings* tab) while overrides are on; the channel gets a system message
  naming who changed it, and the audit log a `retention.changed` row with the channel.
- The purge job and the dry-run count use the channel's period where one is set and the
  workspace's otherwise, in the same `DELETE`, not a query per channel. While overrides are
  off the rows are kept and ignored, and the channel shows the workspace policy.
- Files keep the workspace period.

## Acceptance

- [ ] Every admin task is reachable from one dialog; the workspace menu is short.
- [ ] Retention, exports, erasure and SCIM tokens are usable without the API.
- [ ] Settings save the same way everywhere.
- [ ] An owner can let a channel keep its messages for its own period, and the purge honours
      it.

## Tests

Frontend: the dialog's sections and deep link, each moved section's existing tests, the
retention form (owner only, the preview), exports list and download, the erasure
confirmation, SCIM mint-once. E2E: an owner sets retention and starts an export from the
dialog. Backend: the exports list (an admin sees the workspace's, nobody else's), the
retention preview count, and the SCIM token routes in the OpenAPI document; a channel's
period purged before and after the workspace's, forever kept, overrides ignored while off, a
member who is not a moderator refused.
