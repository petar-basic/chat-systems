# CS-121 — Channel analytics and quiet channels

**Wave:** 32 — what an admin can see (2026-11-04), released as 4.4.0
**Area:** backend · frontend
**Blocked by:** CS-120
**Blocks:** —

## Problem

A workspace a year old has channels nobody posts in, and they crowd the sidebar and
*Browse channels*. Nobody can tell which ones are dead without opening each, and a channel
admin cannot see whether their channel is read at all.

## Approach

- The rollup of CS-120 counts per channel too: messages, members who posted, members who
  read.
- *Workspace settings → Analytics → Channels*: every channel with members, messages and
  readers in the period and the last message's date, sortable, with *Export CSV*. A private
  channel is listed by name to admins, as it already is in *Channel settings*.
- *Quiet channels*: those with no message in 90 days (an admin can pick 30, 60 or 180),
  with *Archive* on each and on a selection. Archiving asks once for all of them and is
  audited like any archive.
- *Channel details* shows a channel admin their own channel's numbers for the last 30 days.

## Acceptance

- [ ] A channel with no message in 90 days is listed as quiet, and archiving it from there
  archives it as *Channel settings* would.
- [ ] A channel admin sees their channel's numbers and no other channel's.
- [ ] The channel numbers add up to the workspace numbers of CS-120.

## Tests

Backend: per-channel counts, the quiet list for each threshold, bulk archive audited per
channel, permissions. Frontend: the table, the selection and the confirm dialog.
