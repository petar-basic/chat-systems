# CS-103 — Apps with bot users and a Web API

**Wave:** 28 — what 4.0.0 needs (2026-10-29 feedback on 3.7.0)
**Area:** backend · frontend · docs
**Blocked by:** CS-102
**Blocks:** CS-104, CS-105

## Problem

What exists for integrations is a set of separate parts: an incoming webhook can post to one
channel, an outgoing webhook and a slash command call out, and a workflow runs steps. None
of them is a *bot*: something with its own name and token that can read a channel it is in,
answer in a thread, react, upload a file or look a person up. A team cannot build the small
internal tools it is used to building on Slack, and the docs describe no bot API because
there is none.

## Approach

- **An app** belongs to a workspace and is made by an admin in *Workspace settings → Apps*:
  name, icon, description, and the scopes it asks for. Creating it creates its **bot user**,
  an account of kind `bot` that cannot sign in, never counts as a seat and shows *APP* next
  to its name. Its slash commands (CS-102) and webhooks hang off it.
- **A bot token** (`cbot-…`, shown once, stored hashed, rotatable, revoked when the app is
  removed) authenticates `Authorization: Bearer` requests as the bot user.
- **Scopes**: `chat:write`, `chat:write.public`, `channels:read`, `channels:history`,
  `groups:history`, `im:write`, `reactions:write`, `files:write`, `users:read`, `commands`.
  Each Web API route names the scope it needs; a token without it gets 403 naming the scope.
- **The bot is a member like anybody else**: it reads and writes only where it has been added
  (`/invite @bot`), except `chat:write.public` for public channels. Guest rules, archived
  channels, retention and the audit log apply to it unchanged; everything it does is audited
  under the app.
- **The Web API is the existing API**: the routes a bot needs accept a bot token, rather than
  a second set of routes that drifts. The OpenAPI document tags them `bot` with their scope;
  rate limits are per token.
- **`docs/bot-api.md`**: tokens, scopes, the routes, rate limits, and a worked example; a
  small bot in `examples/echo-bot/`.

## Acceptance

- [ ] An admin can make an app, choose its scopes and get a token.
- [ ] With the token the bot can post, reply in a thread, react, upload, read history and
  look people up, only in channels it is in and only within its scopes.
- [ ] Removing the app revokes the token and turns off its commands and webhooks.

## Tests

API: token auth and hashing, each scope allowing and refusing, membership checks, a bot in a
guest's DM, archived channels, rate limits, revocation, audit entries. Frontend: creating an
app, the token shown once, the *APP* label. The example bot runs against a seeded instance in
CI.
