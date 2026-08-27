# CS-042 — Restrict who a guest may start a conversation with

**Wave:** 10 — Guest containment and operational readiness
**Area:** backend/api
**Blocked by:** CS-041 (shares the "who does this guest share a channel with" query)
**Blocks:** —
**Audit finding:** MEDIUM — policy gap

## Problem

[`create_conversation`](../../backend/api/src/conversations/routes.rs#L91) checks that
every participant is a member of the workspace and nothing else. A guest can therefore open
a DM with anyone in the company, including people they have never shared a channel with —
the CEO, the finance lead, whoever they can name.

Slack draws this line for a reason: a guest is somebody from outside who was let into a
room, not somebody who joined the company. Once CS-041 stops them from *discovering* the
directory, this is the remaining way to reach it — a guest who guesses or already knows a
user id still gets a private channel to that person.

## Approach

1. **A guest may start a conversation only with people they share a channel with.** Reuse
   the predicate CS-041 introduces rather than writing a second one; the two rules are the
   same rule, applied to reading and to writing.
2. **Existing conversations are not broken.** The restriction is on *creating* one. A
   conversation that already exists — from before the rule, or because the guest was in the
   channel at the time — keeps working, and the participant check that guards reading it is
   unchanged.
3. **A non-guest may still DM anyone in the workspace.** This ticket is about the guest
   role; making the whole workspace opt-in would be a different product.
4. **Refuse with a message that does not confirm the target exists.** `403` with the same
   body whether the user id is real, is not, or is simply out of reach — otherwise the
   endpoint becomes the directory CS-041 just closed.

## Acceptance

- [ ] A guest cannot create a conversation with somebody they share no channel with.
- [ ] A guest can create one with somebody they do.
- [ ] A conversation that already exists still opens for both sides.
- [ ] Members and admins are unaffected.
- [ ] The refusal does not distinguish "no such user" from "not allowed".

## Tests

`http_tests/conversations.rs`: a guest in one channel opens a DM with a channel-mate (200)
and with an unrelated member (403), and the two refusals — unknown id and unreachable
member — are byte-identical. Assert an existing conversation still reads for a guest who
has since left the shared channel.
