# CS-039 — Custom emoji, user groups, bots and slash commands

**Wave:** 9 — Product parity
**Area:** backend/api · frontend
**Blocked by:** CS-019 (hook scoping model), CS-018 (audit)
**Blocks:** —
**Roadmap:** existing items, grouped

## Problem

Four remaining Slack-parity gaps. Grouped into one ticket because they are independent,
individually small, and none of them blocks anything — split into separate branches when
work starts, but plan them together so the hook and mention models are extended once
rather than four times.

## Scope

### Custom emoji

- `workspace_emojis` table (`workspace_id`, `name`, `storage_key`, `created_by`), image
  through the existing `FileStorage` with the ownership rules from CS-009.
- Register in the picker alongside `emoji-mart` data; resolve `:name:` in the renderer from
  CS-024, which is the natural place now that rendering is a tree walk.
- Name uniqueness per workspace, and reject names that shadow standard shortcodes.
- Size and dimension limits enforced on upload — an emoji is not a file share.

### User groups

- `user_groups` and `user_group_members` per workspace, with a handle (`@backend`).
- Extend the mention parser
  ([`lib/mentions.ts`](../../frontend/src/lib/mentions.ts) and
  [`expand_mentions`](../../backend/api/src/messaging/routes.rs#L607)) to resolve a group
  handle to its member set, reusing the fan-out that `@channel` and `@here` already use so
  there is one broadcast path, not two.
- Group management gated at `WorkspaceRole::Admin`; membership changes audited.
- Rate-limit group mentions the same way broadcast mentions are — a group of 70 is a
  `@channel` with a different name.

### Bots

- `HookType::Bot` exists and is unused. A bot is an identity that posts and reads without
  being a person: a token, a display name and avatar, and workspace scoping.
- Messages posted by a bot must be attributed to the bot, not to its creator. This also
  fixes the incoming-webhook behaviour, where messages currently appear as sent by the
  admin who created the hook
  ([`hooks/routes.rs:310`](../../backend/api/src/hooks/routes.rs#L310)) — do that part
  first, it is the smallest change with the clearest payoff.
- Bot tokens are scoped to channels like outgoing hooks are after CS-019, and every bot
  action is audited.

### Slash commands

- `HookType::SlashCommand` exists and is unused. A command registers a name, an endpoint and
  a scope; invoking it POSTs to the endpoint through the same SSRF-validated,
  HMAC-signed transport the outgoing hooks use
  ([`hooks/executor.rs`](../../backend/api/src/hooks/executor.rs)) — do not build a second
  outbound path.
- Ephemeral responses (visible only to the invoker) need a delivery mode the realtime layer
  does not have yet: an event addressed to one connection. `send_to_user` already exists;
  the missing piece is the client-side rendering of a message that is never persisted.
- Built-in commands (`/away`, `/dnd`, `/invite`, `/topic`) should route through the same
  registry so there is one dispatcher.

## Ordering within the ticket

1. Bot identity for incoming webhooks — smallest, fixes an existing wart.
2. Custom emoji — self-contained, high visible value.
3. User groups — touches the mention path, which CS-024 has already restructured.
4. Slash commands — largest, and the only one needing a new realtime delivery mode.

## Acceptance

- [ ] Custom emoji upload, render and picker integration, with per-workspace name
      uniqueness.
- [ ] Group mentions notify all members through the existing broadcast fan-out.
- [ ] Incoming webhook and bot messages are attributed to the bot identity.
- [ ] Slash commands dispatch through the existing outbound transport and support
      ephemeral responses.
- [ ] All management actions are audited.

## Tests

Per sub-feature, in the corresponding `http_tests` module, each with the standard
authorization matrix. Renderer component tests for custom emoji and group mentions. An E2E
spec for the slash-command round trip including the ephemeral response.
