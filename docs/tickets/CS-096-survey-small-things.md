# CS-096 — The survey's small things

**Wave:** 27 — what the 2026-10-28 survey of 3.6.0 found
**Area:** backend · frontend · realtime
**Blocked by:** CS-093
**Blocks:** —

## Problem

Smaller findings, confirmed in the code unless marked *check first*:

- **Guest huddles.** Starting a DM huddle checks only that the partner is in the workspace, not
  `require_reachable`, so a guest can ring a member they could not message.
- **Search results without a place.** A hit from a public channel you are not in has no
  channel name: `placeOf` knows only your own channels and conversations.
- **A thread with its last reply deleted.** `find_root` returns a deleted root only while it
  has replies, so once the last reply goes, the thread still shows a composer and every reply
  fails.
- **Paused people get huddle Activity.** The *Incoming huddle* notification row is written
  before the pause check, so it lands in Activity while the workspace is paused.
- **Export ranges.** `since`/`until` limit messages and files, not `message_edits`,
  `reactions`, polls or votes.
- **Invite email failures.** `email_sent: false` covers both "no SMTP" and "sending failed",
  and *Members* says email is not set up in both cases.
- **Presence on joining** *(check first)*: someone who joins a workspace does not appear online
  to its members until their next presence change.
- **Threads view composer** *(check first)*: no scheduling and no *Also send to channel*,
  which the thread panel has.

## Approach

- DM huddles use `require_reachable`.
- Search returns the channel name with each hit, and the panel uses it.
- A deleted root with no replies left answers 404 for replies, and the client shows the thread
  read-only.
- The pause check comes before the Activity row.
- Edits, reactions, polls and votes follow the message's date range.
- The invite answer says why no email went (`email: "sent" | "not_configured" | "failed"`),
  and *Members* words each.
- Presence and the Threads composer: checked first, fixed if they hold, dropped from this
  ticket if they do not.

## Acceptance

- [ ] Each item above holds as described in *Approach*, or the ticket says why it was dropped.

## Tests

One test per item, at the layer it is fixed in.
