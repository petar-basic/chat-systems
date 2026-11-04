# CS-116 — Title and pronouns on the profile

**Wave:** 30 — make it yours (2026-11-04)
**Area:** backend · frontend
**Blocked by:** —
**Blocks:** —

## Problem

A workspace profile has a display name, a photo, a bio and a status. Slack's also has a
title, which is what people read first on a hover card ("who is this?"), and pronouns. In a
company of more than a few dozen, the title is how you find the person in finance.

## Approach

- `title` (up to 100 characters) and `pronouns` (up to 30) on `workspace_members`, next to
  the other profile columns, so they can differ per workspace like the rest of the profile.
  The account-wide profile in *Preferences → Account* gets them too, in the same two columns
  on `users`, as the default for workspaces where none is set.
- *Edit profile* has both fields. The profile card, the hover card and the member directory
  show the title under the name and the pronouns beside it.
- Member search in the directory and in the mention picker matches the title as well as
  the name.

## Acceptance

- [ ] A title and pronouns set in one workspace show on the hover card there and not in
  another workspace that has its own.
- [ ] Searching the member directory for a title finds the people who have it.
- [ ] SCIM and the Slack import leave both empty rather than guessing.

## Tests

Backend: length limits, the per-workspace value over the account default, member search by
title. Frontend: the hover card with and without both fields.
