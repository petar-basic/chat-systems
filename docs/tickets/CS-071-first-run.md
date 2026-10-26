# CS-071 — First run and an empty workspace that says what to do

**Wave:** 24 — ready for a company
**Area:** backend · frontend · ops
**Blocked by:** CS-070
**Blocks:** —

## Problem

A new instance gets its first admin from `ADMIN_EMAIL` and `ADMIN_PASSWORD` in `.env`
([`bootstrap_admin`](../../backend/api/src/auth/service/mod.rs)). Leave them out and the
instance has nobody who can sign in and no way to sign up, since it is invite-only. Set them
and the admin's password stays in plain text in `.env` for the life of the instance.

After signing in, nothing checks that email works, though invitations and password resets
depend on it, and an empty workspace shows an empty `#general`, with no hint to invite
people or create channels.

## Approach

Two parts, each its own PR.

### 1. A setup page instead of a password in `.env`

- While the instance has no instance admin, the API logs a one-time **setup token** on start,
  and `/setup` (the web app) asks for it, then for the first admin's name, email and password,
  and for the first workspace's name. The token keeps a stranger who finds the URL first from
  taking the instance; it is printed again on every start until setup is done, and works once.
- `ADMIN_EMAIL`/`ADMIN_PASSWORD` keep working for scripted installs, and the RUNBOOK says to
  remove the password once the instance is up.
- Setup ends with **Send a test email** to the admin's address, showing the SMTP error in
  words if it fails (`SMTP_HOST` unreachable, TLS mode wrong, authentication refused).
  The same button lives in the instance admin page afterwards.

### 2. Getting a workspace going

- An admin of a workspace with fewer than three members, or no channel but `#general`, sees
  a short checklist above the messages: *Invite your team*, *Create a channel*, *Set an icon*,
  *Connect SSO* (a link to the RUNBOOK section) — each ticked off by doing it, and dismissible.
- An empty channel says what it is for and offers *Add people* and *Add a bookmark*, instead
  of nothing.

## Acceptance

- [ ] An instance started without `ADMIN_*` can be set up from the browser, and only by
      someone who has the setup token.
- [ ] Setup proves email works or says why it does not.
- [ ] A new workspace tells its admin the next three things to do.

## Tests

Backend: setup refused without the token, refused once an instance admin exists, the token
works once; the test email reports a refused connection. Frontend: the setup form, the
checklist's ticking and dismissal. E2E: a fresh stack set up from `/setup`.
