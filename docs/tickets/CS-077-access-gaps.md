# CS-077 — Access that holds: huddles, email invites, private channels

**Wave:** 26 — what the 2026-10-27 survey found
**Area:** backend · realtime · frontend
**Blocked by:** —
**Blocks:** CS-086

## Problem

Four ways past a check that is supposed to hold, each confirmed in the code:

- **Huddles.** `huddle.join` in the gateway authorises against the `channel_id` (or
  `dm_partner_id`) the client sends, not the huddle's own. Anyone in the workspace, a guest
  included, joins a private channel's huddle by sending its `huddle_id` with a channel they
  are in. `GET /workspaces/{id}/active-huddles` lists every channel huddle, private ones
  included, so the id is handed out.
- **Email invites.** The link in an invite email is a registration JWT good for seven days,
  carrying its own role, and never compared with the `workspace_invites` row. Revoking the
  invite, its expiry and its use limit change nothing, and someone removed from the workspace
  rejoins with the role the old link names — admin, if that is what it said.
- **Private channel members.** `GET /channels/{id}/members` checks workspace membership only,
  so a guest who has a private channel's id lists who is in it.
- **Slack import.** A Slack channel is merged into an existing channel of the same name
  whatever their types, so a private Slack channel's history lands in a public channel.

## Approach

- Huddles: the gateway looks up the session (`huddle_sessions`) by `huddle_id` and authorises
  against its stored channel or DM pair: channel membership for a channel huddle, being one of
  the two people for a DM huddle. The client's `channel_id` is ignored. `active-huddles` lists
  only huddles in channels the caller can read.
- Email invites: the token names the invite (`invite_id` claim) and every use of it —
  `/auth/invites/{token}/verify`, `/accept` and `/auth/complete-registration` — loads the row
  and refuses one that is revoked, expired or used up, taking the role from the row and
  claiming a use like a link invite. The JWT expires when the invite does. Links sent before
  the upgrade carry no `invite_id`: they are refused, and the changelog says to re-send them.
- `GET /channels/{id}/members` uses `require_channel_access`.
- Slack import merges only into a channel of the same type; otherwise it creates
  `<name>-slack` (then `-2`, …) and the import report lists it.

## Acceptance

- [ ] A member of the workspace who is not in a private channel cannot join its huddle or
      see it in `active-huddles`.
- [ ] A revoked, expired or used-up email invite answers 401 on every route; a removed
      member's old link no longer rejoins them.
- [ ] A guest cannot list a private channel's members.
- [ ] A private Slack channel never lands in a public one.

## Tests

Realtime: join with a borrowed `channel_id` is refused; DM huddle joined by a third person is
refused. API: `active-huddles` as an outsider to a private channel; invite revoke/expiry/max
uses across verify, accept and complete-registration; role from the row; members listing
for a guest; Slack import with a same-named channel of the other type.
