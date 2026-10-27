# CS-086 — Getting people in

**Wave:** 26
**Area:** frontend · backend
**Blocked by:** CS-077
**Blocks:** —

## Problem

- Without SMTP, inviting by email says *Invite sent*, and the link is nowhere to be found.
- Signing in with SSO from an invite link drops the invite: a new SSO user lands in no
  workspace.
- *Forgot password* for someone invited but not yet registered sends a reset that never lets
  them in; SSO-only accounts get password resets; the reset token is used up before the new
  password is validated, so a too-short password burns the link.
- Pending invites cannot be seen or revoked in the app.

## Approach

- Every invite offers *Copy invite link*; without SMTP the dialog says email is not set up
  instead of *sent*.
- The invite token is kept across the SSO round trip and the join completed after it; a
  pending account is activated by it.
- A reset for a pending user re-sends the invite; SSO-only accounts are told to sign in with
  SSO; the password is validated before the token is consumed.
- *Workspace settings → Members* lists pending invites with *Copy link* and *Revoke* (on top
  of CS-077's checks).

## Acceptance

- [ ] An instance without SMTP can invite someone end to end.
- [ ] SSO from an invite link lands in the workspace.
- [ ] A short password does not burn a reset link.

## Tests

API: reset for pending and SSO users, validation order. E2E: invite without SMTP, revoke from
the list, SSO invite flow against the dev OIDC provider.
