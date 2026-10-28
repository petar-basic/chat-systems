# CS-089 — Gone means gone: SSO, erase, demotion, workflow links

**Wave:** 27 — what the 2026-10-28 survey of 3.6.0 found
**Area:** backend
**Blocked by:** —
**Blocks:** CS-094

## Problem

Four ways a person keeps something they were meant to lose, each confirmed in the code:

- **SSO after suspension or erase.** `link_or_create` (`auth/oidc_routes.rs`) returns an
  account found through an existing `oauth_accounts` link before it checks the status, so a
  suspended or erased person who signs in through the provider gets a fresh access token,
  good for its whole lifetime (the refresh after it is refused). Erasing an account leaves its
  `oauth_accounts` rows.
- **Hard erase leaves their words.** `erase_author` keeps a message as a tombstone when others
  replied to it, but its `message_edits` rows keep every earlier version of the text. Their
  files stay (`files.message_id` is `ON DELETE SET NULL`), objects included, and so do their
  reactions on other people's messages.
- **Demoted admins keep admin hooks.** The hook executor's `still_acting` checks that the
  creator is active, a member and can read the channel, not that they still hold the role
  the hook type needs. An outgoing webhook or slash command made by an admin who is now a
  member keeps sending channel traffic to their endpoint.
- **Workflow take-over keeps the old link.** `set_owner` and the update path keep
  `webhook_token`, so the previous owner can still start a taken-over workflow from its URL,
  and there is no way to rotate it.

## Approach

- `link_or_create` checks the status of the account whichever way it was found; erase deletes
  the person's `oauth_accounts` rows.
- Hard erase, in one transaction, also deletes the `message_edits` of their messages, their
  reactions, and their files (objects first, as retention does); the erase audit row counts
  each. Poll votes and list comments are checked and handled the same way if they name them.
- `still_acting` takes the hook type and stops a hook whose creator no longer holds the role
  to create it (`Stopped::Demoted`, "Its creator is no longer an admin").
- Taking over a workflow mints a new `webhook_token`; `POST /workflows/{id}/rotate` mints one
  on demand, and *Workflows* offers it next to the URL.

## Acceptance

- [ ] A suspended or erased account cannot get a token through SSO.
- [ ] After a hard erase, nothing they wrote, reacted or uploaded can be read or exported.
- [ ] Demoting a hook's creator below the role the hook needs stops it, with the reason shown.
- [ ] A taken-over workflow's old URL answers 404.

## Tests

OIDC: linked account suspended, erased. Erase: tombstone with edit history, files and their
objects, reactions on others' messages, export afterwards. Executor: outgoing hook and slash
command after demotion. Workflows: take-over and rotate retire the old token.
