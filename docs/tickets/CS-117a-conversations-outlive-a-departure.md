# CS-117a — Conversations outlive a departure

**Wave:** 30 — make it yours (2026-11-05), a bug found mid-wave, released with 4.2.0
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** —

## Problem

When somebody leaves a workspace, or an admin or SCIM removes them, `membership::detach`
calls `remove_member_in`, which deletes every `channel_members` row they had in that
workspace — including their side of each DM and group DM. Whoever they were talking to is
left with a DM that has only themselves in it: the sidebar shows it as a conversation with
nobody, its name and photo are gone, and the history reads as if they had been talking to
themselves.

When the person comes back, through an invite, SCIM or an admin adding them again,
`find_direct` looks for a DM that has both of them as members, finds none, and
`POST /conversations` creates a new, empty one. The old history stays behind in the DM
nobody can reach any more.

Slack keeps the conversation instead. A member who leaves or is removed stays in every DM,
shown as no longer in the workspace; a DM with them can be read but not written to. When
they come back, the same DM carries on.

## Approach

- Departing a workspace still drops the person from its channels, as now — the comment on
  `remove_member_in` says why: a private channel must not come back on its own. Their
  `channel_members` row in a DM or group DM is moved, in the same transaction, to a new
  `conversation_departures` table (channel, user, when they left, and the row's read position
  and notification level) instead of being deleted. `channel_members` keeps meaning "is in
  this conversation now", so the realtime gateway, the notification fan-out, unread counts and
  every membership check stay as they are, and somebody who left receives nothing from that
  workspace.
- A conversation lists its departed participants beside its current ones (`participant_ids`
  stays the current ones; a new `departed_ids` lists the others). The person's name and photo
  come from their account, since they are no longer among the workspace's members. The
  conversation view says *<name> is no longer in this workspace*. In a DM the composer is
  replaced by that line and `POST` to its messages is a 403; in a group DM the others carry on
  and the line sits above the composer.
- Coming back — invite, admin, SCIM, Slack import — moves every departure of theirs in that
  workspace back into `channel_members`, in the same transaction that adds the membership, and
  stages `channel.member_joined` for each, so both sides see the conversation as it was. Their
  read position and notification level are the ones they left with.
- `find_direct` matches only current participants, so a DM with somebody who left is not
  offered to the composer of *New message*; once they are back, it is found again and
  reused, which is what joins the history up.
- A migration repairs the DMs and group DMs that lost a participant before this change: for
  each one, every author of a message in it who has no `channel_members` row there gets a
  departure dated by their last message there, and is restored at once if they are a member of
  the workspace again by then. A DM that the departure split in two keeps both halves; the
  migration does not merge histories, and `find_direct` picks the older one from then on.
- Deactivating an account (the instance admin's *Suspend*) is a separate thing and is left as
  it is.

## Acceptance

- [ ] Bob leaves the workspace. Alice's DM with Bob still shows Bob's name and photo with
  *Bob is no longer in this workspace*, the history is all there, and she cannot send in it.
- [ ] An admin removes Bob instead: the same.
- [ ] While he is gone, Bob receives no event, notification or email from that DM.
- [ ] Bob is invited back and accepts. Alice and Bob both see the same DM, with everything
  said before, and can write in it again; *New message* to Bob opens that DM, not a new one.
- [ ] In a group DM of Alice, Bob and Carol, Bob leaving leaves Alice and Carol writing to each
  other, with Bob shown as gone; Bob coming back sees the whole conversation.
- [ ] A DM that lost its other side before this change shows that person again after the
  migration.

## Tests

Backend:
- a departure moves the DM and group DM rows to `conversation_departures` and drops the
  channel rows;
- a departed participant gets no fan-out and no unread count;
- a message to a DM with a departed partner is a 403, and a group DM still accepts one;
- rejoining restores the rows and `find_direct` then finds the old DM;
- the migration rebuilds a departed participant from the messages.

Frontend: the conversation view with a departed partner, in a DM and in a group DM.

E2E: leave and come back through an invite, and the same DM carries on.
