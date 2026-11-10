# CS-121a — A conversation stays between the people in it, and an archived channel stays read-only

**Wave:** 32a — what the 2026-11-10 survey of 4.6.0 found
**Area:** backend
**Blocked by:** —
**Blocks:** CS-121c, CS-121h

## Problem

Four holes in who may change a channel or write into it, each confirmed in the code:

- **A DM can gain a third person (SEC-7).** `add_channel_member` looks at the channel type only
  to tell public from not public: anyone already in a `dm` or `group_dm` passes the "member of
  this channel" check, and a workspace admin passes `can_moderate_channel` on every channel.
  `membership::add_to_channel` checks nothing. A participant can add somebody to a DM with
  `POST /channels/{dm}/members`, and the new person reads the whole history. An admin who is
  not the owner (only the owner's export includes DMs) can add themselves to any DM whose id
  they know; DM ids appear in audit details. Archive, unarchive, `PATCH /channels/{id}`, the
  member role route and member removal do not refuse a conversation either.
- **`/invite` and `/topic` skip their routes' rules (SEC-8, the rest of DAT-2).** `invoke`
  checks only `require_channel_access`, and `builtin::run` does the work before `finish`
  decides whether the answer may be posted. `/invite` calls `add_to_channel` without the Member
  role check (a guest in a private channel adds another guest), without the archive check and
  without an audit row, and it works in DMs. `/topic` changes the topic of any channel the
  caller can read, archived and announcement-only included, while the panel and
  `PATCH /channels/{id}` require a moderator. The comment in `builtin.rs` says the panel lets
  any member set it; it does not.
- **A closed DM still receives (DM-1).** "Nothing new goes into a DM whose other person left"
  lives only in `require_posting`. The scheduled executor's `authorize_channel` checks access,
  archive and `can_post()`, not the departure, and `invoke` takes `may_post` from
  `can_post()`. A message scheduled on Friday lands on Monday in a DM the UI shows as closed,
  and so does an `in_channel` command answer.
- **Retention posts into an archived channel.** `PUT` and `DELETE /channels/{id}/retention`
  write a system message whether or not the channel is archived, against "read-only all the
  way" from CS-091.

## Approach

- The members of a `dm` or `group_dm` are fixed when it is created. `add_to_channel` refuses
  both types, which covers the route, `/invite` and anything that calls it later. The archive,
  unarchive, update, member-role and member-removal routes refuse them too, with one message:
  "A conversation's members can't be changed". Slack does the same: a group DM with somebody
  new in it is a new group DM.
- `/invite` and `/topic` run the same rules as their routes, through functions the route and
  the built-in both call, not copies: Member role and not archived for adding someone,
  moderator and not archived for the topic, and the same audit rows (`ChannelMemberAdded`,
  `ChannelUpdated`). One rule for the topic, the route's; the wrong comment goes.
- `invoke` takes `may_post` from `require_posting`, and the scheduled executor's
  `authorize_channel` calls it as well, so the departure rule holds for every writer. The
  executor fails such a message with a reason of its own, as it does for an archived channel.
- A retention change on an archived channel is saved without a system message.

## Acceptance

- [ ] `POST /channels/{dm}/members` answers 403 for a participant and for a workspace admin
  outside the DM, and the DM still has two members.
- [ ] Archive, unarchive, rename, role change and removal answer 403 on a DM and a group DM.
- [ ] `/invite` from a guest answers 403; `/invite` and `/topic` in an archived channel answer
  403 and change nothing; `/topic` from a non-moderator answers 403; `/invite` writes an
  audit row.
- [ ] A message scheduled into a DM whose other person has left fails with its reason, and an
  `in_channel` command answer there is not posted.
- [ ] Setting or clearing retention on an archived channel posts nothing into it.

## Tests

`http_tests`: the DM and group-DM matrix (participant, admin outside it, owner) on every channel
member and settings route; `/invite` as a guest, in an archived channel and in a DM, with its
audit row; `/topic` as a member, in an archived channel and in an announcement channel; the
scheduled executor and `/shrug` into a departed DM; retention on an archived channel.
