# CS-123 — A group DM that becomes a private channel

**Wave:** 33 — channels and the people in them (2026-11-13)
**Area:** backend · frontend
**Blocked by:** CS-122
**Blocks:** —

## Problem

A group DM that turns into a project has no way to grow up. It cannot be named, cannot take
a new person without losing what was said, and has no topic, bookmarks or channel settings.
Slack converts it into a private channel and keeps the history.

## Approach

- **Who:** anyone in the group DM, while it has three or more people, with
  *Conversation details → Convert to a private channel*. The workspace's
  `private_channel_creators` rule applies, as it does to making a private channel.
- **What happens:**
  - The conversation becomes a private channel in place, keeping its id, messages, threads,
    files, reactions, pins and read positions.
  - The person who converts it picks the name and is its channel admin. The others become
    members.
  - It leaves the *Direct messages* section and lands in *Channels* for everyone in it.
  - It cannot be turned back.
- **Guests** in the group DM stay in the channel. Under CS-124 a single-channel guest already
  in another channel blocks the conversion, and the dialog names who.
- **Notification settings:** each person's per-conversation setting carries over. A group DM
  notifies on every message, so a person with no setting of their own gets *All new messages*
  for the channel instead of the workspace default, and can change it.
- **A system message** (`kind: "converted_to_channel"`) and an audit entry
  `conversation.converted`.
- **Search and export** read the channel as a channel from then on. An export taken before
  still lists it as a group DM.

## Acceptance

- [ ] A member of a three-person group DM converts it. Everyone in it sees the history under
  the new name in *Channels*, and the old row is gone.
- [ ] A new member added afterwards reads the history, as they would in any private channel.
- [ ] A 1:1 DM has no conversion. A person outside the group DM and a member barred by
  `private_channel_creators` get `403`.
- [ ] A name that is taken is refused before anything changes.
- [ ] Unread counts and the *New* line are where they were.

## Tests

`http_tests`:
- the conversion and its authorization matrix;
- the name collision;
- a 1:1 DM refused;
- read positions kept;
- notification levels carried over.

E2E: convert a group DM and add a fourth person.
