# CS-088 — A Slack import that keeps what it found

**Wave:** 26
**Area:** backend
**Blocked by:** CS-077
**Blocks:** —

## Problem

The import makes Slack guests and deleted accounts full members, imports archived channels
as active, maps only fifteen standard emoji in reactions, and an import running when the
worker restarts stays `running` for ever.

## Approach

- Slack guests become guests; deleted accounts are created suspended, keeping their name on
  their messages.
- Archived Slack channels are imported archived (CS-085).
- Reactions map through the full emoji-mart shortcode set and the workspace's custom emoji.
- The worker marks a `running` import with no heartbeat for five minutes as failed, and it
  can be started again.

## Acceptance

- [ ] An import of a workspace with guests, a deleted account, an archived channel and
      custom reactions looks like it did in Slack.

## Tests

Fixtures for each case; the stale-run sweep.
