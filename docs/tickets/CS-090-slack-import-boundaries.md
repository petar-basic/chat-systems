# CS-090 — A Slack import that respects what is already here

**Wave:** 27 — what the 2026-10-28 survey of 3.6.0 found
**Area:** backend
**Blocked by:** —
**Blocks:** —

## Problem

- **Private channels merge by name.** `place_channel` reuses an existing channel of the same
  name and type (CS-077 stopped private-into-public, not private-into-private). A private
  Slack `#hr` lands in the local private `#hr`: its Slack members are added and read the local
  history, and the local members read Slack's.
- **A re-import archives an open channel.** An archived Slack channel merged into a local one
  leaves it open (CS-088), but the mapping it records is then reused by a second import or a
  *Start again*, and that path queues every mapped archived channel for archiving — the local
  channel included.
- **A returning leaver cannot get in.** Someone who had left Slack is created suspended with
  no password. Unsuspending them gives an active account with no password and no SSO link:
  login refuses it, *Forgot password* sends an SSO reminder, and an invite finds the account
  already there.

## Approach

- Private Slack channels never merge: each gets a channel of its own, `<name>-slack` (then
  `-2`, …) when the name is taken, and the report lists it. Public channels merge as before.
- `slack_channels` records whether the import created the channel (migration);
  only channels it created are archived, on the first run and on any later one.
- Reactivating an account (admin or SCIM) that has no password and no SSO link sets it to
  `pending`, so the ordinary invite flow lets the person set a password.

## Acceptance

- [ ] An import never adds anyone to a private channel that existed before it.
- [ ] Running the same import twice, or starting a failed one again, leaves a merged channel
      open.
- [ ] A leaver imported suspended, then reactivated and invited, can register.

## Tests

Import fixtures: same-named private channel; second run and retry over a merged archived
channel. Auth: reactivate a password-less imported account, invite, complete registration.
