# CS-124 — Guests for one channel or several, until a date

**Wave:** 33 — channels and the people in them (2026-11-13)
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** CS-125

## Problem

A guest is one role. An admin invites the address, waits for it to join, then adds it to
channels one by one, and nothing ends the access when the contract does. Slack has
single-channel guests, multi-channel guests and an end date on both, all set on the invite.

## Approach

- **Two kinds** on `workspace_members`:
  - `guest_kind`: `single` or `multi`, set only for the guest role.
  - `guest_until`: a date, or none.
  - A single-channel guest is in exactly one channel. Adding them to a second is refused,
    and the answer says to make them a multi-channel guest first.
  - Converting a group DM into a private channel (CS-123, `conversations::convert`) counts
    as adding everyone in it. A single-channel guest already in another channel blocks the
    conversion, and the answer and the dialog name who.
- **The invite** for a guest says which channels and, optionally, until when. The admin
  picks the channels in the invite dialog; one only for a single-channel guest. Accepting
  joins them to those channels in the same transaction that makes them a member. A link
  invite for guests is refused, since a link cannot be held to one person.
- **The end date:**
  - A `chat-worker` loop removes a guest whose date has passed, through the path an admin's
    removal takes: sessions for that workspace revoked, sockets closed, hooks and apps of
    theirs stopped (CS-121c).
  - The inviting admin and the workspace admins get an Activity item three days before.
  - An admin can move or clear the date from *Workspace settings → Members*.
- **Members list:** the role column says *Single-channel guest* or *Multi-channel guest* and
  the end date. A filter shows guests ending in the next 30 days.
- **Slack import** keeps a restricted user as a multi-channel guest and an ultra-restricted
  one as a single-channel guest, with no end date.
- **SCIM** is unchanged: it provisions members, not guests.

## Acceptance

- [ ] An admin invites a single-channel guest to `#vendor`. After accepting, the guest is in
  `#vendor` and in nothing else, and cannot be added anywhere else.
- [ ] A multi-channel guest invited to three channels lands in all three.
- [ ] A guest whose end date passes is signed out of the workspace within a minute of the
  loop's next run, and an open tab says why.
- [ ] Admins see the Activity item three days ahead, and can move the date.
- [ ] Guests who exist today become multi-channel guests with no end date, by the migration.

## Tests

`http_tests`:
- a guest invite with channels, and a link invite for guests refused;
- the second channel refused for a single-channel guest;
- a group DM conversion refused while a single-channel guest in it is in another channel;
- expiry through the loop with sessions revoked;
- the three-day notice once only;
- a Slack import of both kinds.

E2E: invite a single-channel guest and accept.
