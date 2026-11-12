# CS-121o — Short links that say nothing about who they are for

**Wave:** 32b — email (2026-11-12)
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** CS-121p

## Problem

Seen in MailHog on 4.7.0:

- **The reset link is 426 characters.** `forgot_password` sends
  `{public_url}/reset-password?token=<JWT>`. The token is a signed access-token-shaped JWT
  (`generate_reset_token`): a header with `kid`, the user's id, an empty `email`, `iat`,
  `exp`, `jti`, `token_type` and `invite_id`, plus an EdDSA signature. The JWT does nothing a
  database row does not already do: the `jti` is stored in `password_reset_tokens` and deleted
  on use.
- **The invite link is about 500 characters and carries the invitee's address.**
  `invite_url` sends `{public_url}/invite/<JWT>` with `email`, `sub` and `invite_id` in a
  payload anyone can read. The link ends up in browser history, in proxy logs, and in a chat
  when an admin copies it from *Invite people*. The email's *Button not working?* line prints
  all of it.
- A plain-text client wraps either link over three or four lines, and some break it there.

## Approach

- A link token is 32 random bytes, base64url, 43 characters. Only its SHA-256 is stored, so
  a database read does not hand out working links.
- **Reset:** `password_reset_tokens` is keyed by the token's hash instead of a `jti`. Single
  use, one hour, and the revocation on use stay as they are. The migration drops the rows it
  finds; they live an hour.
- **Invite:** a new table `invite_links (token_hash, invite_id, user_id, created_at)`, its rows
  going with the invite and with the person. Expiry, uses and revocation stay on the invite and
  are checked where they are now (`open_email_invite`, `redeem_email_invite`). Every link sent
  for an invite keeps working while the invite does, as today.
- `registration_invite` becomes a lookup, and the `/auth/invites/{token}` routes,
  `/auth/complete-registration` and the SSO join read it. The `reset` and `registration`
  token types and `Claims.invite_id` go.
- The URLs keep their paths: `/reset-password?token=…` and `/invite/{token}`.
- **Upgrading:** links emailed before the upgrade stop working. A reset link lived an hour. A
  person whose invite is still open gets a new link from *Forgot password* or *Email me a
  code*, which send the invite again (`resend_invite`), or from a new invite. The CHANGELOG
  says so.

## Acceptance

- [ ] A reset link and an invite link are the public URL plus at most 70 characters.
- [ ] Neither link carries an address or an id, and the database holds only hashes.
- [ ] A reset link works once and an hour; a second use is refused and sessions are revoked
  as before.
- [ ] An invite link opens the invite, registers, joins an existing account and joins through
  SSO; it stops working when the invite is revoked, expires, is used up or its person is
  removed.
- [ ] A link from 4.7.0 is refused with *Invalid or expired invite* or the reset page's own
  error.
- [ ] Erasing a person deletes their invite links; retention clears expired reset rows as now.

## Tests

`http_tests`: link length; only the hash stored; reset replay; each invite route with a revoked,
expired and used-up invite; a JWT-shaped token refused; erase. E2E: the invite flow in
`qa-flow2.spec.ts` reads the new link.
