# CS-120 — Workspace analytics

**Wave:** 32 — what an admin can see (2026-11-04), released as 4.4.0
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** CS-121

## Problem

An admin cannot answer the questions a company asks of its chat tool: how many people use
it, whether that is going up, which channels carry the work. *Instance admin* has four
totals (`/admin/stats`), and a workspace admin has nothing. Slack's *Analytics* page is
what an admin there looks at to justify the tool.

## Approach

- The worker keeps a daily rollup per workspace from the workspace stream: messages posted
  (`message.created`), files shared, reactions (`reaction.added`), and members active, a
  member being active on a day they posted, reacted or moved a read position
  (`channel.read_state`, staged on the stream if it is only published to the user today). `workspace_daily_stats` holds the totals and
  `member_daily_activity` one row per member per day they were active, so a week or a month
  counts distinct people without scanning `messages`. A backfill builds the rows from the
  existing messages once.
- *Workspace settings → Analytics*, for admins, over the last 7, 30 or 90 days:
  - members: active per day and per week, against the total, and how many joined;
  - messages per day, in public channels, private channels and direct messages;
  - a table of members with days active, messages posted and the last day active, sortable.
- Counts only, never content, and a direct message is counted, never shown. Guests are
  included and can be filtered out.
- *Export CSV* for each table.

## Acceptance

- [ ] After a week of use the charts show each day's active members and messages, and the
  numbers agree with a count made directly in the database.
- [ ] The member table shows nothing of what anyone wrote.
- [ ] A member who is not an admin gets 403 from every analytics route.
- [ ] The CSV opens in a spreadsheet with the same rows as the table.

## Tests

Backend: the rollup from a stream of events, idempotent when an event comes twice, the
backfill, the 7/30/90-day windows, admin only. Frontend: the charts with empty and full
data, the table sort, the CSV.
