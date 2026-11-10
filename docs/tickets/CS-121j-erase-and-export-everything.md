# CS-121j — Erase and export that cover everything about a person

**Wave:** 32a — what the 2026-11-10 survey of 4.6.0 found
**Area:** backend
**Blocked by:** —
**Blocks:** —

## Problem

Confirmed in the code:

- **Hard erase leaves what was added after 3.7.0 (DAT-7).** `erase_user_data` does not touch
  `passkeys`, `user_totp` and its recovery codes, `notification_keywords`, or
  `push_subscriptions`, and its `UPDATE users` leaves `preferences` and `timezone`. A poll on a
  root kept as a tombstone keeps its question and options: `erase_writing` empties only
  `content` and `metadata`, and `polls` cascades only when the message row goes. A workspace
  photo from a workspace the person has already left stays, because erase finds the photos
  through membership rows. After a GDPR erase the names of their passkeys, their keywords and
  the text of their polls are still there. Nobody can sign in with any of it (sign-in refuses an
  account that is not active), so this is personal text, not access.
- **A workspace export cannot say who wrote what (EXP-1).** `members.jsonl` is
  `to_jsonb(workspace_members)`: no email, no account name, and no row for anyone who has left;
  messages carry only `user_id`. An owner who exports for a legal request cannot tell who wrote
  a message without the database. Owners already see members' emails in *Members*, so putting
  them in the export shows them nothing new.

## Approach

- `erase_user_data` deletes the person's passkeys, TOTP and recovery codes, keywords and push
  subscriptions, and resets `preferences` and `timezone`; the audit row counts each.
- `erase_writing` deletes the poll of every root it keeps as a tombstone, with its options and
  votes.
- Photos are found by their storage prefix (`avatars/<user id>/`), not through memberships.
- The export writes `users.jsonl`: id, name and email for every member and every author in it,
  including people who have left and erased accounts (as they now read).
- `docs/RUNBOOK.md` lists what an erase removes and what the export contains.

## Acceptance

- [ ] After erasing someone with a passkey, a keyword, a push subscription and a poll on a root
  with replies, none of them is left.
- [ ] A photo from a workspace they had already left is gone from storage.
- [ ] An export names the author of every message, including someone who left before it.

## Tests

Erase with each kind of row present; the photo of a left workspace; an export with a departed
author.
