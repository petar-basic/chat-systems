# CS-121i — Emailed codes with a ceiling, and instance roles that apply at once

**Wave:** 32a — what the 2026-11-10 survey of 4.6.0 found
**Area:** backend · frontend · realtime
**Blocked by:** —
**Blocks:** —

## Problem

Confirmed in the code; the guess rate is worked out from the limits, not measured:

- **No ceiling on guessing an emailed code (SEC-12).** A code allows five tries
  (`MAX_TRIES`), a new request resets `attempts` to 0 (`EmailCodeRepo::replace`), an address
  may ask five times in 15 minutes, and verifying shares the password limiter, ten tries per
  address per 15 minutes. That is about 960 guesses a day at a six-digit code, roughly 0.1% a
  day per account without a second factor, with no total after which it stops. It is on by
  default wherever SMTP is set up. Because verifying shares the password limiter, the person
  being guessed at is also locked out of signing in with their password while it goes on.
- **The code request tells an active account from an unknown one by its timing.** For an active
  account `request_email_code` hashes the code with Argon2, stores it and queues the email
  before it answers; for an unknown address it answers at once.
- **A pending person with no live invite gets nothing.** *Email me a code* and *Forgot
  password* call `resend_invite`, which does nothing when there is no live email invite. Both
  screens say an email was sent. The Slack import makes exactly these accounts.
- **A demoted instance admin keeps the admin panel for up to 15 minutes.** Admin routes read
  `is_instance_admin` from the access token's claims, and `update_instance_role` neither
  revokes nor refreshes anything.
- **A new workspace creator does not see *Create workspace* until a reload (ONB-8).** The
  button reads `instance.user.instance_role`, which the client loads only when it starts, and
  `update_instance_role` tells nobody. An admin says "you can now"; the desktop app that has
  lived in the tray for days shows nothing.

## Approach

- Failed codes are counted per account over 24 hours, outside the per-code `attempts`. After 20
  the account cannot sign in by code until the window passes; requests still answer "sent" and
  send nothing. Code verification gets a limiter key of its own, so guessing at codes no longer
  locks the password out.
- `request_email_code` answers once the limits pass and does the lookup, hashing and sending in
  a task of its own, so every answer takes the same time.
- A pending person with no live invite is told by email that their invitation has expired and to
  ask an admin of the workspace for a new one. The answer is still "sent".
- Instance-admin routes read the role from `users`, not from the token.
- A role change sends the person a frame; the client refreshes its session and its instance
  user, so *Create workspace* and the admin entry appear or go at once.

## Acceptance

- [ ] The 21st wrong code in a day is refused even with a fresh code; password sign-in still
  works while codes are being guessed.
- [ ] A code request for an active and an unknown address take the same time.
- [ ] A pending person with an expired invite receives an email saying so.
- [ ] A demoted instance admin gets 403 from admin routes at once.
- [ ] Making someone a workspace creator shows them *Create workspace* without a reload.

## Tests

`http_tests`: the daily ceiling across fresh codes; separate limiters; the expired-invite email;
admin routes after demotion with an old token. Frontend: the instance user refreshed by the
frame. Realtime: the frame reaches every socket of the person.
