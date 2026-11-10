# CS-121b — Ownership is handed over, never taken

**Wave:** 32a — what the 2026-11-10 survey of 4.6.0 found
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** —

## Problem

The owner role can be taken by the wrong person and never given up by the right one, both
confirmed in the code:

- **An admin makes themselves owner through an invite (SEC-6).** `create_invite` requires Admin
  and passes `CreateInviteRequest.role` through unchecked, `owner` included. `accept_invite`
  calls `add_member_tx`, an upsert (`ON CONFLICT … DO UPDATE SET role = $3`). An admin who
  posts `{"role":"owner","max_uses":1}` and opens the link themselves is an owner: full export
  with private channels and DMs, retention, deleting the workspace, and nobody can demote them.
  `update_member_role` has exactly this rule ("Cannot grant a role above your own level") and a
  test. The upsert also works downwards: an owner or admin who opens a member link quietly
  becomes a member, and a guest who opens one becomes a full member. The web UI only makes email
  invites, so today this is reached through the API.
- **An owner can never step down or leave (WSP-1).** `update_member_role` refuses to demote an
  owner and `remove_member` refuses to remove one, themselves included. *Members* lets an owner
  make somebody else an owner, and then neither of them can step down or leave.
  `LeaveWorkspaceDialog` only says that an owner can't leave. When the person who made the
  workspace leaves the company it keeps them as its owner; once they are suspended, retention,
  export and deleting the workspace are out of everybody's reach unless a second owner was
  made in advance. This is a dead end of *Leave workspace* from 4.1.0.

## Approach

- `create_invite` refuses a role above the inviter's own: an owner may invite an owner, an
  admin anyone up to admin.
- Opening an invite as somebody already in the workspace keeps their role: `accept_invite` uses
  `add_member_if_absent_tx`, as the email path already does, and answers with the existing
  membership.
- An owner may step down to admin, or leave, while another owner remains; the last owner is
  refused with "Make someone else an owner first". `workspaces.owner_id` moves to a remaining
  owner when the one it names goes. Check first what reads it — the Slack import uses it as
  the creator of the channels and emoji it makes, the notification consumer as a fallback —
  and if nothing needs it to be a current member, say so in the PR and leave it.
- An instance admin can make any member of a workspace its owner from *Instance admin →
  Workspaces*, for a workspace whose only owner is gone. It is audited like a role change.
- *Members* and `LeaveWorkspaceDialog` say what an owner can do: step down or leave once there
  is another owner, and who can make one.

## Acceptance

- [ ] An admin's invite with the owner role answers 403; an owner's is accepted.
- [ ] Opening a member link as an owner, an admin or a guest keeps that role.
- [ ] With two owners, either can step down or leave; the last owner is refused.
- [ ] An instance admin makes a member the owner of a workspace whose owner is suspended, and
  the new owner opens *Data retention*.

## Tests

`http_tests`: the invite role matrix for each inviter role; opening a member link as owner,
admin and guest; two owners stepping down and leaving, the last one refused, `owner_id`
afterwards; the instance admin assignment and its audit row. Frontend: the dialog's wording for
the only owner and for one of two.
