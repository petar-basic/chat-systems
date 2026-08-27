# CS-041 — Scope the member directory to what a guest may see

**Wave:** 10 — Guest containment and operational readiness
**Area:** backend/api · frontend
**Blocked by:** —
**Blocks:** CS-042
**Audit finding:** HIGH — data exposure

## Problem

[`list_members`](../../backend/api/src/workspace/routes.rs#L254) requires only workspace
membership, and
[`list_members_with_users`](../../backend/api/src/workspace/repo.rs#L221) selects
`u.email` for every member of the workspace. A guest — a contractor, a client, anyone
invited into one channel — receives the full staff directory of the company, with email
addresses, on their first page load.

Every other guest rule in the product is tight: `requires_explicit_membership()` holds them
to the channels they were added to, CS-010 keeps them out of public channels they did not
join, and CS-034 keeps them out of search results they should not see. The directory is the
one door still open, and it is the one that hands over a phishing list.

It also undercuts the compliance story: an instance that ships GDPR-style export and erasure
should not be leaking a personal data set to the least-trusted role in the system.

## Approach

1. **A guest sees the people they share a channel with, and nobody else.** That is the
   Slack rule and it matches the membership model already in place — the set is
   `workspace_members` intersected with the users in the guest's `channel_members` rows.
   One query, not a filter in the handler.
2. **Emails are not part of the directory for a guest**, even for people they do share a
   channel with. A display name and an avatar are what the UI needs to render a message; an
   address is what an attacker needs. Non-guests keep the current payload — the workspace
   already trusts them with it.
3. **One shape, two projections**, rather than two endpoints. `MemberWithUser` gains a
   redaction step applied in the repo query for guests, so a future caller cannot forget it
   the way a handler-level filter would be forgotten.
4. **Everywhere a member list is rendered goes through it.** Audit the callers first:
   the members panel, the mention autocomplete, the DM composer and the channel-members
   panel all populate from this endpoint today, and a guest whose mention list still holds
   the whole company has learned the same thing.
5. **Say what a guest is** in the docs. The role is currently described by what it cannot
   do; after this it needs one line about what it cannot see.

## Acceptance

- [ ] A guest's `GET /workspaces/:id/members` returns only users they share a channel with.
- [ ] A guest's copy of that payload carries no email addresses.
- [ ] A non-guest member sees the workspace as before.
- [ ] Mention autocomplete and the DM composer show a guest no one they cannot already see.
- [ ] The guest role's read scope is documented in README and RUNBOOK.

## Tests

`http_tests/workspace.rs`: seed a workspace with a guest in one of two channels and assert
the guest sees only the shared members, without emails, while an ordinary member sees both
channels' members with emails. Assert the same for a guest with no channels at all — the
list is empty, not the workspace.
