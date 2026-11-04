# CS-118 — Passkeys

**Wave:** 31 — signing in (2026-11-04), released as 4.3.0
**Area:** backend · frontend · desktop
**Blocked by:** —
**Blocks:** CS-119

## Problem

An account signs in with a password, and a careful one adds a TOTP code from an app. Both
can be phished, both get typed on a phone with a thumb, and a team that has moved to
passkeys everywhere else finds a chat app that asks for a password old-fashioned. Slack
signs in with a passkey and keeps the password for those who want it.

## Approach

- WebAuthn with `webauthn-rs`. The relying party is the instance's public origin from the
  config, so a passkey belongs to one server. Challenges live in Redis for two minutes and
  are spent on first use.
- A `passkeys` table: the credential, a name the person gives it, when it was added and last
  used. Up to ten per account.
- *Preferences → Security* lists them, adds one (the browser's own sheet), renames and
  removes one. Adding or removing asks for the password or a TOTP code first, as turning
  TOTP off does.
- *Sign in with a passkey* on the sign-in page, with discoverable credentials, so no email is
  typed. A passkey verified by the device (user verification required) is a second factor in
  itself: such a sign-in does not then ask for a TOTP code, and it satisfies
  `require_admin_totp` for an instance admin.
- After a password, a passkey can stand in for the TOTP code wherever the code is asked.
- An SSO account is not offered passkeys; its identity provider is where its sign-in
  belongs. A suspended account's passkey signs in to nothing.
- The desktop app cannot run the ceremony in its own window, since a webview is not trusted
  for an arbitrary server's origin. It runs it in the system browser and comes back through
  the deep link, the same way SSO does (CS-109).
- `passkey.added`, `passkey.removed` and `passkey.used` go to the audit log.

## Acceptance

- [ ] A passkey added in one browser signs in from the sign-in page with no email typed, and
  an account with TOTP is not then asked for a code.
- [ ] With a password and TOTP enrolled, a passkey works in place of the code.
- [ ] Removing the last passkey leaves the account signing in with its password.
- [ ] In the desktop app, *Sign in with a passkey* finishes in the system browser and the
  app is signed in.
- [ ] A challenge used twice, or after two minutes, is refused.

## Tests

Backend: registration and sign-in against a software authenticator, a replayed and an
expired challenge, user verification required, the TOTP step skipped only after a passkey,
SSO accounts refused. Frontend: the Security list, the sign-in button. E2E: Playwright's
virtual authenticator adds a passkey, signs out, signs in with it.
