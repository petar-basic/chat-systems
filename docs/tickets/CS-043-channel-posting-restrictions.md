# CS-043 — Announcement channels: restrict who may post

**Wave:** 10 — Guest containment and operational readiness
**Area:** backend/api · frontend
**Blocked by:** —
**Blocks:** —
**Audit finding:** MEDIUM — missing feature

## Problem

There is no way to make a channel read-only. Anyone who can reach a channel can post in it:
the send path runs
[`require_channel_access`](../../backend/api/src/authz.rs#L78) and nothing else. A company
announcements channel, a release feed, a channel shared with guests where only staff should
speak — none of them exist as a concept.

The schema anticipated it and nobody used it: `channels.settings` is a `JSONB` column
([migration 1](../../backend/migrations/20240305000001_initial_schema.sql)) that **no route
reads or writes today**. It is an empty object on every row.

This is the one gap in this wave that is a missing feature rather than a leak, and it is the
one most likely to be the reason a team says the product is not ready.

## Approach

1. **One setting, not a permission system.** `settings.post_policy` is `everyone` (the
   default, and what every existing channel gets) or `moderators` — the people
   `can_moderate()` already identifies: workspace admins and channel admins. Resist adding
   a per-user posting allowlist; it is a second membership table pretending to be a flag.
2. **Enforce it in `authz`, not in the handler.** `require_channel_access` answers "may
   they see it"; add `require_channel_post` beside it and route every write through it —
   messages, thread replies, reactions on a locked channel are a separate decision (allow
   them; a reaction is not a post), scheduled sends, incoming webhooks and slash commands
   that post `in_channel`. The list of writers is the part that gets missed, so enumerate it
   in the ticket and test each.
3. **The API is the boundary, but the client must not offer what will be refused.** The
   composer is hidden and replaced with a line saying who may post; the realtime path needs
   no change, because delivery is unaffected.
4. **Changing the policy is a moderator action and is audited** (CS-018), with the before
   and after — "who silenced this channel" is exactly the question an audit log is for.
5. **A bot is not an exception.** An incoming webhook posting into an announcement channel
   is the normal case, so the hook's own scoping (CS-019) decides it: a hook attached to
   the channel may post regardless of `post_policy`, and that is stated rather than
   discovered.

## Acceptance

- [ ] A channel can be set to `moderators` and back by a channel admin or workspace admin.
- [ ] A plain member's send, thread reply, scheduled send and `in_channel` slash command are
      all refused in such a channel.
- [ ] Reactions and reads are unaffected.
- [ ] An incoming webhook scoped to the channel still posts.
- [ ] The composer is replaced by an explanation rather than failing on submit.
- [ ] The policy change is audited with before and after.

## Tests

`http_tests/channels.rs`: the full writer matrix — member, channel admin, workspace admin,
guest, webhook — against both policies. `http_tests/scheduled.rs`: a message scheduled
before the policy changed is refused at delivery time, not silently dropped. A component
test that the composer is not rendered for a member in a `moderators` channel.
