# CS-119 — Sign in with a code sent by email

**Wave:** 31 — signing in (2026-11-04), released as 4.3.0
**Area:** backend · frontend
**Blocked by:** CS-118
**Blocks:** —

## Problem

Someone who never set a password, or forgot it, has to go through *Forgot password* and
choose a new one only to get in. Slack signs in with a code it emails to you. A code is
also what a phone wants: a link opens the browser, not the app (CS-125).

## Approach

- *Email me a sign-in code* on the sign-in page. `POST /auth/email-code` answers the same
  whoever asks, so it does not tell who has an account; a password or passkey account gets a
  six-digit code, an account still waiting on its invite gets that invite again, and an SSO
  account is told to sign in with SSO, as forgot-password does.
- The code is stored hashed, is good for ten minutes and five tries, and works once. A new
  code replaces the old one. Requests are rate-limited per email and per IP like
  forgot-password.
- `POST /auth/email-code/verify` with the email and the code returns a session. The code
  stands in for the password only: an account with TOTP or a passkey still takes its second
  step (CS-118).
- The instance admin can turn the option off in *Instance admin → Email*. It is off, and
  not shown, while the instance has no SMTP.
- `email_code.sent` and `email_code.used` go to the audit log; a code that runs out of tries
  is audited too.

## Acceptance

- [ ] A person who never set a password signs in with the emailed code.
- [ ] The answer to the request is the same for an address with no account.
- [ ] A sixth try, a code older than ten minutes and a used code are all refused.
- [ ] An account with TOTP is asked for its code after the emailed one.
- [ ] With the option off, or with no SMTP, the sign-in page does not offer it.

## Tests

Backend: the code's lifetime, tries and single use, the same answer for unknown addresses,
the rate limit, the second step kept, the instance switch. E2E: request a code, read it from
MailHog, sign in.
