# CS-075 — Who may notify a whole channel, and who may make one

**Wave:** 24 — ready for a company
**Area:** backend · frontend
**Blocked by:** CS-070, CS-074
**Blocks:** —

## Problem

Any member can write `@channel`, `@here` or `@everyone` in any channel they can post in,
and `expand_mentions` notifies every member (or every online one). In a company of fifty
that is the first thing someone misuses, and there is nothing an admin can do about it but
ask. Any Member can also create public and private channels.

The only posting control a channel has is `post_policy` (announcement: moderators post,
everyone reacts). Slack splits this in two: the workspace decides who may notify a channel
and who may create channels, and each channel decides whether it allows `@channel` and
`@here` at all.

## Approach

### Workspace rules

Three new keys in the workspace `settings` JSON, read and written like
`workflow_builders` through `PATCH /workspaces/{id}` (admins):

- `broadcast_mentions`: `members` (default, today's behaviour) or `admins` — workspace
  admins and owners, plus a channel's own admins in that channel;
- `public_channel_creators` and `private_channel_creators`: `members` (default) or
  `admins`. Guests cannot create channels either way, as today.

They appear in CS-070's *General* section under *Permissions*.

### Channel rule

- `ChannelSettings` gains `allow_broadcast_mentions` (default on), set by a channel
  moderator through `PATCH /channels/{id}` and shown in *Channel details → Settings* (CS-074)
  beside the announcement switch.

### Enforcement

- One predicate in `authz`, `can_broadcast(state, channel, user)`, combining the two rules.
- Sending a message, creating a poll and scheduling a message refuse a broadcast mention the
  author may not make with a 403 that says why. Silently dropping it would let someone
  believe they told everyone.
- `expand_mentions` ignores a broadcast the author may not make, which covers a scheduled
  message whose author lost the right before it went out and a workflow whose builder never
  had it. A workflow is checked once more when it is saved, so the builder hears about it
  then.
- `create_channel` checks the creator rule for the channel's type.

### Composer

- `@channel`, `@here` and `@everyone` are not suggested when the author may not use them.
- In a channel of more than 25 members, a broadcast mention asks first, through
  `ConfirmDialog`: "This notifies 140 people. Send anyway?"
- *Create channel* hides the type the person may not create, and the button when they may
  create neither.

## Acceptance

- [ ] An admin can limit `@channel` / `@here` / `@everyone` to admins.
- [ ] A channel can turn broadcast mentions off for everyone in it.
- [ ] An admin can limit who creates public and private channels.
- [ ] Nobody sends a broadcast they are not allowed to without being told.
- [ ] Defaults keep today's behaviour.

## Tests

Backend: `can_broadcast` across both rules and each role; send, poll and schedule refused
with 403; a scheduled message and a workflow run that lost the right post without
notifying; `create_channel` per rule and channel type; the settings keys round-trip and only
an admin changes them. Frontend: the composer's suggestions per rule, the large-channel
confirmation, *Create channel* per rule, the two new controls. E2E: an admin limits
broadcasts to admins and a member's `@channel` is refused with the reason.
