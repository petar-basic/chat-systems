# CS-051 — All unreads

**Wave:** 14 — the daily-use gaps a team arriving from Slack notices first
**Area:** backend · frontend
**Blocked by:** ~~CS-050~~ ✅ shipped (a channel marked unread has to appear here)
**Blocks:** —

## Problem

Catching up after a morning of meetings means clicking through every bold channel in the
sidebar one at a time. Slack's *All unreads* puts the new messages from every channel on one
page, channel by channel, with a button to mark each read. The counters already know which
channels have anything new; there is no view that shows it.

## Approach

1. `GET /workspaces/{ws_id}/unreads` returns, for each channel with `unread_count > 0`
   (muted channels last, or excluded by a toggle), the channel and its newest 20 unread
   messages — one query over the counters, then one bounded page per channel. Paginated
   by channel so 200 unread channels do not arrive as one response.
2. Frontend: an *All unreads* entry at the top of the sidebar with the total, opening a
   view grouped by channel, each group with *Mark as read* and a link into the channel.
   Reading the view does not mark anything read; the button does.
3. Keyboard: `Shift+Alt+A` opens it, `Esc` marks the focused group read, as in Slack.

## Acceptance

- [ ] Every channel with unread messages appears, newest activity first.
- [ ] *Mark as read* clears that channel's badge everywhere.
- [ ] Guests see only the channels they belong to; DMs appear alongside channels.
- [ ] At 200 channels with unread messages the first page arrives in under 200 ms
      (`measure-scale.py` gains the scenario).

## Tests

`http_tests` for the grouping, the muted ordering and guest scoping. A frontend test for
mark-as-read from the view. The latency number in RUNBOOK's measured table.
