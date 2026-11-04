# CS-115 — Time zones and local time

**Wave:** 30 — make it yours (2026-11-04)
**Area:** backend · frontend
**Blocked by:** CS-113
**Blocks:** —

## Problem

`users.timezone` exists and the notification schedule uses it, but nobody can set it and
nobody sees it: it is `UTC` unless something wrote it. Slack shows a person's local time on
their profile and warns before you message someone outside their working hours, which is
what a team spread over time zones relies on. Times are also always shown on a 24-hour
clock.

## Approach

- On sign-in the browser's time zone is saved when the account has none, and when it
  differs, the app asks once whether to update it.
- *Language & region* in *Preferences*: the time zone (searchable list of IANA names), and
  a 12-hour or 24-hour clock as a CS-113 preference. The language picker joins this tab when
  the translation work lands; it is not part of this ticket.
- A person's profile and hover card show their local time, such as "14:05 local time", and
  a DM's composer says when it is late or early for them (before 08:00 or after 21:00 their
  time).
- The notification schedule reads the same time zone, so it is right once it is set.

## Acceptance

- [ ] A new account gets the browser's time zone, and the profile card shows the local time.
- [ ] Changing the time zone moves the notification schedule's hours with it.
- [ ] The 12-hour setting changes every time shown in messages, threads and search.
- [ ] A DM to someone at 23:00 their time says so above the composer.

## Tests

Backend: an unknown time zone is refused. Frontend: local time on the profile card, the
late-hours note, both clock formats.
