# CS-121d — A Slack import that respects what changed since the last run

**Wave:** 32a — what the 2026-11-10 survey of 4.6.0 found
**Area:** backend
**Blocked by:** —
**Blocks:** —

## Problem

A second run of an import, or *Start again* after a failed one, undoes what admins did in the
meantime. Confirmed in the code unless marked *check first*:

- **People are put back into channels they left or were removed from (IMP-4).** For a channel
  mapped by an earlier run, `import_channels` calls `add_channel_members`, which adds every
  Slack member that has a local account, without asking whether they are still in the channel or
  still in the workspace. `import_users` skips mapped users, so someone removed from the
  workspace is not put back into it — only into its channels, which is the state the comment
  above `remove_member_in` warns about: the realtime gateway looks at `channel_members` alone.
  An admin removes Bojan from private `#leadership`; the next run puts him back, without a trace.
- **A restored channel is archived again.** A channel the import created and archived (it was
  archived in Slack) and an admin then unarchived is archived once more on the next run,
  because the reuse branch looks only at `created_channels`.
- **A failed run keeps the whole archive forever (IMP-5).** `job.rs` deletes the uploaded zip
  only when the run succeeds; there is no *Discard*, no sweeper, and `requeue` has no age limit.
  The archive — the whole Slack history, DMs included — sits in storage outside retention and
  erase. *Start again* months later skips only messages whose `slack_ts` row still exists, so
  messages that a hard erase removed are written back. *Check first* that retention-purged
  messages are purged again by the next retention run, and that `imported_message_ids` is
  what decides.
- **Analytics never counts imported history (ANA-1).** The consumer counts `message.created`
  events, the import publishes none and writes Slack's time as `created_at`, and the backfill
  runs once. A team that installs 4.4 or later and then imports sees zero messages and zero
  active people for its imported months, while *Quiet channels* and `last_message_at` say
  otherwise. The member totals (`people`, `members` in `analytics/repo.rs`) do not filter on
  `users.status`, so the suspended accounts an import makes for people who left Slack, and
  pending ones nobody has claimed, raise *members* and lower the active share.

## Approach

- On the reuse branch, members are added only to a channel this run created. A channel that
  already existed keeps the members it has; the report says how many Slack members were not
  added and why. Nobody is ever added to a channel of a workspace they are not a member of.
- A channel is archived only in the run that created it.
- A failed run's archive is deleted after `SLACK_IMPORT_KEEP_FAILED_DAYS` (default 7) by the
  worker, after which the run is no longer retryable and says so. *Discard* on a failed run
  deletes it at once. A retry skips authors whose account was erased.
- At the end of a run that wrote messages, the import counts the days it wrote into
  `member_daily_activity` for that workspace, the way the backfill does. The member totals
  count active accounts only.
- RUNBOOK's import section says how long a failed archive is kept.

## Acceptance

- [ ] Import, remove a person from a private channel, run again: they are not a member.
- [ ] Import, remove a person from the workspace, run again: they are in none of its channels.
- [ ] A channel the import archived and an admin restored stays open after the next run.
- [ ] A failed run older than the limit has no archive and cannot be retried; *Discard* removes
  it at once.
- [ ] After importing into a fresh instance, *Analytics* shows the imported days, and a
  suspended leaver is not among the members.

## Tests

Import service tests for the three re-run cases; the worker's sweep and *Discard*; a retry with
an erased author; analytics after an import and with a suspended member.
