# CS-062 — Notification schedule

**Wave:** 21 — notifications that fit how a team works
**Area:** backend · frontend
**Blocked by:** CS-060 ✅
**Blocks:** —

## Problem

Do not disturb today is a timer: `/dnd 60` or the YouMenu sets `users.dnd_until`, and it
ends. A working schedule (notify me 9 to 6 on weekdays) has to be set by hand every evening,
so in practice people get pushes at 23:00 or turn push off altogether. Slack calls this the
notification schedule, and it is the setting that makes a phone install tolerable.

## Approach

### 1. The schedule

- `notification_schedules (user_id PRIMARY KEY, days SMALLINT[], start_minute SMALLINT,
  end_minute SMALLINT, enabled BOOLEAN)`: one window, the same each selected day, in the
  user's own `users.timezone` (already stored and validated). That is Slack's "every day" /
  "weekdays" / "custom" with one start and end. Different hours per day are Slack's
  advanced mode; leave them out until somebody asks.
- A window that crosses midnight (22:00–07:00) is allowed and means what it says.
- `GET` / `PUT /users/me/notification-schedule`.

### 2. One definition of "paused"

- `NotificationRepo::is_dnd_active` becomes "the timer is running, or now is outside the
  schedule in the person's timezone", computed in SQL (`AT TIME ZONE users.timezone`). Every
  caller (the push path, the email digest, the presence payload) gets the schedule for free.
- Presence already carries DND for the moon icon; it reports a scheduled pause the same way,
  with the time it ends, so "notifications paused until 9:00" shows on the profile card.
- `/dnd off` during a scheduled pause resumes until the next scheduled start, as in Slack.
  That needs one column, `dnd_override_until`, cleared when the window opens.

### 3. The UI

- **Preferences → Notifications → Schedule**: a switch, the day chips (with Every day and
  Weekdays shortcuts) and two time `Select`s in 30-minute steps, showing the timezone in
  use with a link to change it in the profile.
- **The YouMenu's pause item** says "Paused by your schedule until 9:00" when that is why,
  with Resume.

## Acceptance

- [ ] Outside the window no push, email or desktop notification fires; inside it they do.
      The Activity panel fills either way.
- [ ] The window is evaluated in the person's timezone, including across midnight and on a
      DST change day.
- [ ] Resume during a scheduled pause lasts until the next window starts, then the schedule
      takes over again.
- [ ] The profile card and the YouMenu show a scheduled pause and when it ends.

## Tests

`http_tests` for the route and validation (start equals end refused, unknown day refused).
Repo tests for `is_dnd_active` with a fixed `NOW()`: inside, outside, across midnight, a
non-UTC timezone, the DST transition in `Europe/Belgrade`, the override. Mind the Docker VM
clock: pass the instant in rather than reading the database clock where the test can. A
frontend test for the schedule editor.
