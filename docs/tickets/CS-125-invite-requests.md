# CS-125 — Asking an admin to invite someone

**Wave:** 33 — channels and the people in them (2026-11-13)
**Area:** backend · frontend
**Blocked by:** CS-124
**Blocks:** —

## Problem

Only workspace admins can invite, and a member who wants a colleague in has to find an admin
and ask in a DM. The admin then retypes the address. Slack has a setting for who may invite.
When invites are admin-only, members ask through the app and an admin approves or declines.

## Approach

- **A rule** in *Workspace settings → General*: `inviters`, `WhoMay` like the others.
  - `members`: any member invites directly, up to their own role, as an admin does today.
  - `admins`, the default: a member's *Invite people* sends a request instead.
  - Guests never invite and never request.
- **A request** is an invite that waits for an admin:
  - It holds the addresses, the role (member or guest), the guest's channels and end date
    from CS-124, and an optional reason.
  - The requester sees it under *Pending* until an admin decides.
  - The same address twice, or one that is already a member, is refused.
- **Admins:**
  - They get an Activity item and an email per request.
  - They decide in *Workspace settings → Members → Requests*: approve, edit then approve, or
    decline with an optional note.
  - Approving sends the invite as the approving admin. The language follows CS-121q, and the
    email names the requester too.
  - A request nobody decides on in 30 days expires.
- **The requester** gets an Activity item for each answer.
- **Audit:** `invite_request.created`, `.approved`, `.declined`, `.expired`.

## Acceptance

- [ ] With `inviters = admins`, a member requests an invite for two addresses with a reason.
  Each admin sees it in Activity and under *Requests*.
- [ ] An admin approves; the invite email goes out and the requester is told.
- [ ] An admin declines with a note; the requester sees the note.
- [ ] With `inviters = members`, the same dialog invites directly and no request is made.
- [ ] A guest has no *Invite people*. The request route answers `403` to them, and to anyone
  outside the workspace.

## Tests

`http_tests`:
- the rule both ways;
- a request's lifecycle: created, approved with edits, declined, expired by the loop;
- duplicates refused;
- the authorization matrix;
- the email naming the requester.

E2E: a member requests and an admin approves.
