# CS-121c — What someone built stops when they lose the right to it

**Wave:** 32a — what the 2026-11-10 survey of 4.6.0 found
**Area:** backend · frontend
**Blocked by:** CS-121a
**Blocks:** CS-121h

## Problem

Wave 27 made hooks and workflows stop when the person behind them goes (CS-089). Five things
were left out, each confirmed in the code:

- **A slash command outlives its creator's demotion (SEC-3, the rest).** `invoke` finds the
  command with `find_slash_command`, which filters only on `is_active`, and never calls
  `hook_still_acting`. The webhook paths do (the hook executor, the incoming route), and the
  3.7.0 changelog says commands do too. An admin who registers `/deploy` against their own
  server and is demoted to member keeps receiving everything typed after it, and its answers
  keep landing in channels.
- **Turning a hook back on keeps its credential (SEC-9).** `HookRepo::enable` sets
  `is_active`, clears the reason and moves `created_by`; `config.token` (incoming) and
  `config.secret` (outgoing) stay. A removed admin's incoming webhook is turned off; another
  admin clicks *Turn on* (3.7.0, ONB-7); the URL the removed admin still has posts again, now in
  the new admin's name. `rotate_hook` exists, and taking over a workflow already mints a new
  link (WF-1).
- **An app's token outlives the admin who made it (SEC-10).** `BotRepo::authenticate` checks
  the token hash and that the workspace isn't deleted; `app_events` picks subscribers without
  looking at the creator; `stand_down` stops hooks and workflows, not `bot_apps`, and so do
  suspension and SCIM deactivation. An admin who made an app with `groups:history`, kept its
  token (shown once) or pointed its request URL at their own server, and is then removed or
  suspended, keeps reading every channel the bot is in, and events keep arriving. A weaker
  outgoing webhook in the same place would have stopped.
- **A removed admin comes back through their own link (SEC-11).** `membership::detach` leaves
  `workspace_invites` alone, and `accept_invite` looks only at expiry and uses. A link invite
  lasts up to 168 hours and 100 uses and may carry the admin role.
- **Every member reads every app's request URL (SEC-13).** `list_bot_apps` and `get_bot_app`
  require only membership and return `request_url`, `last_failure`, `created_by` and the
  scopes, guests included. The directory shows members only the host on purpose
  (`host_of`), and the only screen that calls these routes is an admin's (`AppPage` renders
  `Controls` only when `can_manage`).

## Approach

- `invoke` runs `hook_still_acting` before it dispatches a registered command. A command that
  fails it is turned off with its reason, as on the webhook paths, and the caller gets
  `unknown_command`.
- Turning a hook on mints a new incoming token or outgoing secret and returns it once, as
  `rotate_hook` does (`HookSecrets`). Slash commands carry no credential and are unchanged.
- Apps follow the hooks' rule. `stand_down` turns off the apps the person created: the token is
  refused, no events go out, and the app shows why (a migration adds `disabled_reason` to
  `bot_apps`). Turning it back on is an admin's action and mints a new token and signing
  secret, shown once. This is stricter than Slack, which leaves bot tokens alone, and it is the
  rule this project already chose for hooks and workflows: a removed admin with a working token
  is the same hole whichever kind of integration holds it.
- `detach` and suspension delete the link invites the person created. Email invites stay: they
  are bound to somebody else's address.
- `list_bot_apps` and `get_bot_app` require Admin.

## Acceptance

- [ ] A slash command whose creator was demoted answers `unknown_command`, and *Integrations*
  shows why it stopped.
- [ ] After a hook is turned off and on, its old incoming URL answers 401 and outgoing
  deliveries are signed with a new secret.
- [ ] Suspending or removing an app's creator makes its token answer 401 and stops its events;
  turning it on returns a new token, and the old one stays refused.
- [ ] Removing or suspending an admin makes the link invites they made answer 404.
- [ ] A member and a guest get 403 from `GET /workspaces/{id}/bot-apps` and
  `GET /bot-apps/{id}`.

## Tests

`http_tests`: invoke after the creator's demotion; turn off and on for each hook type; an app
after its creator is removed, suspended and SCIM-deactivated, then turned on again; link and
email invites after their creator is removed; both bot-app routes as admin, member and guest.
