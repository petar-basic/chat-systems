# Bot API

An **app** is something a workspace builds for itself: a deploy bot, a stand-up reminder, a
bridge to another system. It has a **bot user** with its own name and token, and it calls the
same REST API the web client uses. This page is for whoever writes one.

## Make an app

A workspace admin makes it in *Workspace settings → Apps → Build an app*: a name, an optional
description, and the scopes it needs, which the admin agrees to, in plain words, before the
token exists. Giving the app another scope later asks again. The token is shown once, when the
app is made or given a new one; copy it then. It looks like `cbot-` and 43 more characters,
and the server keeps only its SHA-256.

The bot user appears in the member list with an *APP* label. It cannot sign in, has no
password or address, and is not counted among the instance's accounts.

*New token* replaces the token at once: the old one stops working on the next request.
*Remove* revokes the token, removes the app's slash commands and webhooks, closes its sockets
and takes the bot out of the workspace. What it posted stays.

## Call the API

Send the token as a bearer token. Every route is under `/api/v1` on the instance. Before
5.0.0 it was `/api`, and nothing answers there now: a bot, script or pasted URL from before
5.0.0 has to have the `/v1` added, incoming webhooks and workflow webhooks included.

```bash
curl -s https://chat.example.com/api/v1/bot-apps/me \
  -H "Authorization: Bearer $CHAT_BOT_TOKEN"
```

`GET /bot-apps/me` answers with the app: its `id`, `workspace_id`, `bot_user_id` and
`scopes`. It is the one route that needs no scope, so it is how a bot learns who it is.

A bot token works only on the routes below. Anything else answers `403 This route does not
accept a bot token`, and a token the server does not know answers `401`. The OpenAPI document
at `/api/v1/openapi.json` tags these routes `bot` and lists each one's scope under the `bot`
security scheme.

## Scopes

| Scope | Lets the bot |
|-------|--------------|
| `chat:write` | post, reply in a thread, edit and delete its own messages, where it is a member |
| `chat:write.public` | post in a public channel it has not been added to |
| `channels:read` | list the channels it is in |
| `channels:history` | read messages and threads in the public channels it is in |
| `groups:history` | read messages and threads in the private channels and DMs it is in |
| `im:write` | start a DM or a group DM |
| `reactions:write` | add and remove its reactions |
| `files:write` | upload files to attach to its messages |
| `users:read` | list the workspace's members, and the people who have left it |
| `commands` | own slash commands |
| `links:read` | hear about links to [its domains](#link-previews) posted where it is a member |
| `links:write` | show its own previews of those links |
| `workflow.steps:execute` | run the [workflow steps](#workflow-steps) it offers, and define them itself |

A route the token has no scope for answers `403` with the scope's name:
`This bot token does not have the reactions:write scope`. An admin can change an app's scopes
at any time without a new token.

## Routes

| Method | Route | Scope | Notes |
|--------|-------|-------|-------|
| GET | `/bot-apps/me` | — | the app |
| GET | `/workspaces/{ws_id}/channels` | `channels:read` | the channels the bot is in |
| GET | `/channels/{ch_id}/messages` | `channels:history` or `groups:history` | newest first; `?limit=` up to 200, `?cursor=` a message id to page back from |
| GET | `/messages/{msg_id}/thread` | `channels:history` or `groups:history` | the replies to a message |
| POST | `/channels/{ch_id}/messages` | `chat:write` | `{ content, thread_parent_id?, attachment_ids?, client_message_id?, blocks?, attachments? }`; `content` may be empty when [blocks](#blocks) are sent |
| PATCH | `/messages/{msg_id}` | `chat:write` | `{ content, blocks?, attachments? }`, its own messages only |
| DELETE | `/messages/{msg_id}` | `chat:write` | its own messages only |
| POST | `/messages/{msg_id}/reactions` | `reactions:write` | `{ emoji }` |
| DELETE | `/messages/{msg_id}/reactions/{emoji}` | `reactions:write` | |
| POST | `/files/upload/{ws_id}?draft=true` | `files:write` | multipart `file` fields; post the returned ids as `attachment_ids` |
| GET | `/workspaces/{ws_id}/members` | `users:read` | `is_bot` marks other apps |
| GET | `/workspaces/{ws_id}/former-members` | `users:read` | `{ user_id, display_name, avatar_url }` of people who left, to name the author of an old message |
| POST | `/workspaces/{ws_id}/conversations` | `im:write` | `{ participant_ids }`; one person opens or reuses a DM |
| POST | `/views/open` | — | `{ trigger_id, view }`; see [modals](#modals) |
| POST | `/views/push` | — | `{ trigger_id, view }`, with a trigger from inside a view |
| POST | `/views/update` | — | `{ view_id or external_id, hash?, view }` |
| POST | `/views/publish` | — | `{ user_id, view, hash? }`; see [App Home](#app-home-and-shortcuts) |
| POST | `/messages/{msg_id}/unfurls` | `links:write` | `{ unfurls: { <url>: { blocks } } }`; see [link previews](#link-previews) |
| PUT | `/bot-apps/me/workflow-steps` | `workflow.steps:execute` | `{ steps }`, the whole list; see [workflow steps](#workflow-steps) |
| POST | `/workflow-executions/{execution_id}/complete` | `workflow.steps:execute` | `{ outputs }` |
| POST | `/workflow-executions/{execution_id}/fail` | `workflow.steps:execute` | `{ error }` |

`client_message_id` is a version 4 UUID the bot picks. Sending the same one twice posts once,
so a bot that retries after a timeout does not double-post.

## Blocks

A bot can lay a message out with Slack's blocks instead of writing it as text. The same JSON
works on `POST /channels/{ch_id}/messages` and `PATCH /messages/{msg_id}` with a bot token, on
an incoming webhook, and in a slash command's answer, inline or through `response_url`. A
payload written for Slack works unchanged:

```bash
curl -s https://chat.example.com/api/v1/hooks/incoming/$HOOK_TOKEN \
  -H 'Content-Type: application/json' \
  -d '{
    "text": "Build 412 passed",
    "blocks": [
      { "type": "header", "text": { "type": "plain_text", "text": ":white_check_mark: Build 412 passed" } },
      { "type": "section", "fields": [
        { "type": "mrkdwn", "text": "*Branch:*\nmain" },
        { "type": "mrkdwn", "text": "*Commit:*\n<https://github.com/acme/api/commit/4f2a9c1|4f2a9c1>" }
      ] },
      { "type": "actions", "elements": [
        { "type": "button", "text": { "type": "plain_text", "text": "View run" },
          "url": "https://github.com/acme/api/actions/runs/9201" }
      ] }
    ]
  }'
```

| Block | Fields read |
|-------|-------------|
| `header` | `text` (`plain_text`, at most 150 characters) |
| `section` | `text` (at most 3000), `fields` (at most 10, each at most 2000), `accessory` |
| `divider` | — |
| `image` | `image_url`, `alt_text`, `title` |
| `context` | `elements`: text objects and `image` elements, 1 to 10 |
| `actions` | `elements`: 1 to 25 of the elements below, except `image` |

| Element | Fields read |
|---------|-------------|
| `button` | `text` (at most 75), `url`, `style` (`primary` or `danger`), `action_id`, `value` |
| `static_select` | `placeholder`, `options` (1 to 100), `initial_option`, `action_id` |
| `users_select` | `placeholder`, `initial_user`, `action_id` |
| `conversations_select` | `placeholder`, `initial_conversation`, `action_id` |
| `overflow` | `options` (1 to 5), `action_id` |
| `datepicker` | `placeholder`, `initial_date` (`YYYY-MM-DD`), `action_id` |
| `image` | `image_url`, `alt_text`, as a section's accessory |

A message takes at most 50 blocks. `block_id` and `action_id` are kept, at most 255 characters.
Any other type, a field of the wrong kind or a length past the limit is refused with `422` and
the path of what is wrong, such as `blocks[3].text: too long, at most 3000 characters` or
``blocks[1].type: unknown block type `rich_text` ``. A link (`url`, `title_link`) has to be
`http` or `https`. An image has to be on a public address: one that is private, loopback or
link-local is refused, so a block cannot make a reader's browser fetch from the network the
instance sits in.

A button with a `url` opens it in a new tab. Every other element reaches the app when
somebody uses it, if the app has turned on [interactivity](#buttons-menus-and-modals);
otherwise it is shown disabled, with a tooltip saying the app has not set up interactivity.

**Text objects.** `plain_text` is shown as it is, with emoji names such as `:tada:` turned into
the emoji. `mrkdwn` is turned into this app's markdown:

| Slack | Here |
|-------|------|
| `*bold*`, `_italic_`, `~strike~`, `` `code` ``, ```` ```block``` ````, `> quote` | the same marks |
| `<@USER_ID>` | a mention, when the id is a member of the workspace; otherwise `@USER_ID` as text |
| `<#CHANNEL_ID>` | `#name` for a public channel of the workspace, otherwise the label or the id |
| `<https://…\|label>`, `<https://…>` | a link |
| `<!here>`, `<!channel>`, `<!everyone>` | `@here`, `@channel`, `@everyone` |
| `<!date^…\|fallback>` | the fallback |
| `:emoji_name:` | the emoji; a name that is not a standard emoji is left for the workspace's own |
| a line break | a line break |

Ids are this app's: the `id` from `/workspaces/{ws_id}/members` and `/workspaces/{ws_id}/channels`.
Mentions are read from the message's text below, so a mention inside a block notifies only
when the text is flattened from the blocks, and only from a bot: a webhook's or a command's
message notifies nobody it mentions, with or without blocks.

**Legacy attachments.** `attachments`, Slack's older layout that CI tools still send, is read
too: `color` (`good`, `warning`, `danger` or a hex colour) as the bar beside it, `pretext`,
`author_name`, `author_link`, `title`, `title_link`, `text`, `fields` (`title`, `value`, `short`,
at most 20), `image_url`, `thumb_url`, `footer`, `ts` and `blocks`. A message takes at most 20.
`pretext`, `text`, a field's `value` and `footer` are `mrkdwn`; the rest are plain text.

**The text.** `text` stays, and it is what notifications, email, search and the edit history
read. It is required unless blocks or attachments are sent; without it, the text is flattened
from them, a line for each header, section, field, image title and context, and cut at 4000
bytes. With blocks, the message shows the blocks and not `text`; with attachments alone, it
shows `text` above them, as Slack does. `text` itself is stored as it is sent, in this app's
markdown, not converted from `mrkdwn`.

**Editing.** `PATCH` with `blocks` or `attachments` replaces the layout, both parts at once;
`"blocks": []` with no attachments takes it away. `PATCH` with only `content` keeps the layout
and changes the text. People's own messages stay markdown: blocks from a person's session are
refused with `403`, and a message built from blocks is edited only by the bot that sent it.

**Slash commands.** An `in_channel` answer is posted with its blocks. An `ephemeral` answer is
seen only by the person who ran the command, as the text flattened from them.

**Slack import.** A message an integration posted in Slack comes over, under the workspace
owner and with the integration's name and icon, when it carries blocks or attachments this
app can show. One that carries only text, or a layout this app does not know, is skipped as
before and listed in the report.

## Where a bot may go

A bot is a member of its workspace and nothing more. It reads and writes only in the
channels it has been added to, with `/invite @App` or *Channel details → Members*, even
public ones; `chat:write.public` is the one exception, and only for posting. A DM is a channel
whose members are the people in it, so a bot reads a DM it started.

Everything else that applies to a member applies to it unchanged: an announcement channel
takes posts only from its moderators, an archived channel takes none, and retention deletes
its messages like anybody's. Every request that
changes something is written to the audit log as `bot_app.called`, with the route, under the
app.

## Rate limits

Each token has its own budget. Writes share the limits people have (120 messages and 240
reactions a minute); reads are 300 a minute. Past either, the API answers
`429 Too Many Requests` with a `Retry-After` header in seconds.

## Events

An app hears what happens through **events**. An admin picks them on the app's page in *Apps*, and the
app gets them over HTTP, at a request URL, or over a socket it opens itself (socket mode).

| Event | When | Scope |
|-------|------|-------|
| `message` | somebody posts in a channel the bot is in | `channels:history`, or `groups:history` for a private channel or DM |
| `app_mention` | a message in a channel the bot is in mentions the bot | as `message` |
| `reaction_added`, `reaction_removed` | a reaction changes in a channel the bot is in | as `message` |
| `member_joined_channel` | somebody joins a channel the bot is in, the bot included | `channels:read` |
| `channel_created` | a public channel is made | `channels:read` |
| `channel_archived` | a channel the bot is in is archived | `channels:read` |
| `app_home_opened` | somebody opens the app's [Home tab](#app-home-and-shortcuts) | — |
| `link_shared` | a message in a channel the bot is in links to one of [its domains](#link-previews) | `links:read` |
| `function_executed` | a workflow run gets to one of [its steps](#workflow-steps) | `workflow.steps:execute` |

An app never hears about a channel it is not in. `channel_created` is the one exception, and
it is only sent for public channels, which every member of the workspace can already browse. An
event the app has no scope for is not sent; it is not an error. The bot does not hear its own
messages or reactions, and the notices the instance posts itself (a channel was archived,
somebody joined) arrive as their own events, not as messages.

Every event arrives in the same envelope:

```json
{
  "event_id": "4f8a…",
  "event_time": "2026-10-29T10:00:00Z",
  "type": "app_mention",
  "workspace_id": "1b2c…",
  "event": {
    "channel_id": "9d0e…",
    "message_id": "77aa…",
    "user_id": "c3d4…",
    "text": "@[Deploy bot](…) ship it",
    "thread_parent_id": null,
    "created_at": "2026-10-29T10:00:00Z"
  }
}
```

| `type` | `event` |
|--------|---------|
| `message`, `app_mention` | `channel_id`, `message_id`, `user_id`, `text`, `thread_parent_id`, `created_at` |
| `reaction_added`, `reaction_removed` | `channel_id`, `message_id`, `user_id`, `emoji` |
| `member_joined_channel` | `channel_id`, `user_id` |
| `channel_created` | `channel_id`, `name`, `created_by` |
| `channel_archived` | `channel_id` |
| `app_home_opened` | `channel_id` (the person's conversation with the app), `user_id`, `tab` (`home`), `view` (the Home last published for them, or `null`) |
| `link_shared` | `channel_id`, `message_id`, `user_id` (who posted it), `links` (`[{ domain, url }]`, the links not answered yet) |
| `function_executed` | `execution_id`, `run_id`, `workflow` (`{ id, name }`), `step` (`{ callback_id, title, position }`), `inputs`, `expires_at` |

Delivery is at least once and in order within a channel: the next event in a channel waits
until the one before it is delivered or given up on. A retry carries the same `event_id`, so an
app that has to do something exactly once keeps the ids it has seen.

### Over HTTP

Each event is a `POST` of the envelope to the app's request URL, signed like an outgoing
webhook: `X-ChatSystems-Signature` is `sha256=` and the hex HMAC-SHA256 of the body, keyed with
the app's **signing secret** (*Show signing secret* on the app's page). Check it before trusting
the body, and compare `event_time` with your clock to turn away an old request sent again.

Any 2xx answer delivers the event; answer quickly and do the work afterwards. Anything else,
or no answer in ten seconds, is retried after 5 seconds, 30 seconds, 2, 5, 10 and then every 15
minutes, and a retry carries `X-Chat-Retry-Num` (1 for the first retry). An event nothing takes
within an hour is dropped, and the app is shown as **failing** in *Apps*, with the last error,
until an event is delivered again.

The URL is checked when it is saved. The instance posts

```json
{ "type": "url_verification", "challenge": "…", "app_id": "…" }
```

signed the same way, and saves the URL only if the answer is a 2xx whose body is the challenge,
or JSON with a `challenge` field equal to it. The URL is held to the same rules as an outgoing
webhook's: no private addresses unless the instance allows them.

### Over a socket

An app behind a firewall, or one that would rather not run a web server, turns on **socket
mode** on the app's page in *Apps* and makes an **app-level token** there. The token starts with
`capp-`, is shown once, and does one thing: it opens sockets. A new one replaces it and closes
every socket the old one opened.

```bash
curl -s -X POST https://chat.example.com/api/v1/apps/connections.open \
  -H "Authorization: Bearer $CHAT_APP_TOKEN"
```

answers `{ "url": "wss://chat.example.com/ws/apps?ticket=…" }`. The ticket is good for one
connection, within 30 seconds. The socket says hello first:

```json
{ "type": "hello", "app_id": "…", "connection_id": "…" }
```

and then sends each event as

```json
{ "envelope_id": "…", "type": "event", "retry_attempt": 0, "payload": { "event_id": "…", "type": "message", … } }
```

The app acknowledges it by sending back `{ "envelope_id": "…" }` within three seconds;
otherwise it is sent again, to that socket or another, with `retry_attempt` one higher. An app
may open several sockets, from one process or several, and the events are shared out between
them. Before the instance closes a socket on purpose, because the app-level token was replaced
or revoked, socket mode was turned off or the app was removed, it sends
`{ "type": "disconnect", "reason": "token_revoked" }`. Open a new one with
`connections.open`. A socket that answers no ping for a minute is closed, and its
unacknowledged events go to the app's other sockets.

## A slash command of the app's own

A command is registered as a `slash_command` hook (see *Slash commands* in
[backend.md](./backend.md#hooks)) with `app_id` set to the app, which needs the `commands`
scope. The command is removed with the app. Its requests are signed and may be answered later
through `response_url`, as for any command.

An app in socket mode needs no `url` for its command. Each invocation arrives on a socket as

```json
{ "envelope_id": "…", "type": "slash_command", "retry_attempt": 0,
  "payload": { "command": "deploy", "text": "api", "channel_id": "…", "workspace_id": "…",
               "user_id": "…", "request_id": "…", "response_url": "https://…",
               "trigger_id": "…" } }
```

to be acknowledged like an event and answered through `response_url`. The person who typed it
sees *working on it…* until then, or *could not be reached* straight away when the app has no
socket open. An invocation nobody acknowledges within a minute is dropped.

## Buttons, menus and modals

An app turns on **Interactivity** on its page in *Apps*. From then on, the buttons, menus and
date pickers in the messages it posts, and in the answers to its own commands, reach it when
somebody uses them. With it off they stay disabled. An element reaches only the app whose
message it is in, and no scope is needed for it. Each payload sent is written to the audit log
as `bot_app.interaction`.

A payload goes where the app's events go: a signed `POST` to its request URL, or a frame on
one of its sockets. It is sent once and never retried. An app that does not answer within
three seconds leaves the element as it was, and the person sees *The app did not respond*.

### What the app is sent

A button or menu in a message:

```json
{
  "type": "block_actions",
  "app_id": "…", "workspace_id": "…",
  "user": { "id": "…", "name": "Ana" },
  "trigger_id": "…",
  "container": { "type": "message", "message_id": "…", "channel_id": "…" },
  "channel": { "id": "…", "name": "deploys" },
  "message": { "id": "…", "user_id": "…", "text": "…", "blocks": [ … ], "attachments": [ … ],
               "thread_parent_id": null, "created_at": "…" },
  "response_url": "https://chat.example.com/api/v1/interactions/responses/…",
  "actions": [
    { "type": "button", "block_id": "decision", "action_id": "approve", "value": "api@production",
      "text": { "type": "plain_text", "text": "Approve" }, "action_ts": "…" }
  ]
}
```

An action carries what was chosen: `value` for a button, `selected_option` for a
`static_select` or `overflow`, `selected_user`, `selected_conversation` or `selected_date`. A
`block_id` or `action_id` the app left out is made up when the message is posted, so every
element can be told apart; give your own to recognise them.

An element inside a modal sends the same `block_actions`, with
`"container": { "type": "view", "view_id": "…" }` and the `view` with its `state` in place of
the channel, message and `response_url`. Submitting a modal sends

```json
{
  "type": "view_submission",
  "app_id": "…", "workspace_id": "…",
  "user": { "id": "…", "name": "Ana" },
  "trigger_id": "…",
  "view": {
    "id": "…", "root_view_id": "…", "previous_view_id": null, "hash": "…",
    "type": "modal", "callback_id": "deploy-request", "private_metadata": "…",
    "title": { … }, "blocks": [ … ],
    "state": { "values": {
      "service": { "name": { "type": "plain_text_input", "value": "api" } },
      "target": { "env": { "type": "radio_buttons",
                           "selected_option": { "text": { … }, "value": "production" } } }
    } }
  }
}
```

and closing one with `"notify_on_close": true` sends `view_closed`, with `is_cleared` true when
the whole stack was closed. Over HTTP the payload is the body, signed with
`X-ChatSystems-Signature` like an event. Over a socket it arrives as

```json
{ "envelope_id": "…", "type": "interactive", "accepts_response_payload": true, "payload": { … } }
```

### What the app answers

A `block_actions` needs only an acknowledgement: any 2xx over HTTP, or
`{ "envelope_id": "…" }` on the socket. The app then answers through `response_url`, or opens a
view with `trigger_id`.

A `view_submission` is answered in the same acknowledgement: the HTTP body, or `payload` beside
`envelope_id` on the socket. With nothing, the view is closed.

| `response_action` | |
|-------------------|-|
| `errors` | `{ "errors": { "<block_id>": "Say which laptop" } }` shows each message under its input, and the view stays open |
| `update` | `{ "view": { … } }` replaces the submitted view |
| `push` | `{ "view": { … } }` puts a view on top of it |
| `clear` | closes every view in the stack |

A required input left empty and a text past its `max_length` or short of its `min_length` are
refused before the app is asked, with the message under the input. A choice that is not one of
the options counts as none.

### response_url

`response_url` takes up to five `POST`s within 30 minutes, each with the body of a message:

| Field | |
|-------|-|
| `text`, `blocks`, `attachments` | the answer, as in [Blocks](#blocks) |
| `replace_original: true` | replaces the message the element was in |
| `delete_original: true` | deletes it |
| `response_type: "in_channel"` | posts the answer in the channel, from the app's bot |
| otherwise | only the person who used the element sees it, as a command's answer is seen |

`replace_original` and `delete_original` work only on a message the app posted: the
`response_url` of a [message shortcut](#app-home-and-shortcuts) used on somebody else's message
answers `403 An app changes only its own messages`. A sixth use, a late one, and one after the app was turned off are refused.

### Modals

The view routes take the app's bot token and answer `409` while its interactivity is off.
`POST /views/open` opens a modal for whoever caused `trigger_id`: somebody
who used one of the app's elements, or ran one of its commands, in the last three seconds. A
trigger opens one view; a used or expired one is refused with `422`. A trigger from an element
inside a view can `POST /views/push` a view on top of it, up to three; and
`POST /views/update` replaces a view by `view_id` or `external_id`, refused with `409` when a
`hash` is sent and the view has changed since.

```json
{
  "type": "modal",
  "callback_id": "deploy-request",
  "external_id": "request-41",
  "private_metadata": "anything up to 3000 characters, sent back with the view",
  "title": { "type": "plain_text", "text": "Request a deploy" },
  "submit": { "type": "plain_text", "text": "Request" },
  "close": { "type": "plain_text", "text": "Cancel" },
  "notify_on_close": false,
  "clear_on_close": false,
  "blocks": [
    { "type": "input", "block_id": "service", "label": { "type": "plain_text", "text": "Service" },
      "hint": { "type": "plain_text", "text": "Its short name" },
      "element": { "type": "plain_text_input", "action_id": "name", "max_length": 40 } }
  ]
}
```

A view takes the message blocks and `input`, up to 100 blocks. The title, submit and close
labels are `plain_text` of at most 24 characters, and a view with inputs needs a submit label.
An `input` has a `label`, an `element`, an optional `hint` and `optional: true` for one that
may stay empty; its element is one of the selects, a `datepicker`, or:

| Element | Fields read |
|---------|-------------|
| `plain_text_input` | `placeholder`, `initial_value`, `multiline`, `min_length`, `max_length` (at most 3000), `action_id` |
| `checkboxes` | `options` (1 to 10), `initial_options`, `action_id` |
| `radio_buttons` | `options` (1 to 10), `initial_option`, `action_id` |

Each answers with the view as it now is, with its `id`, `root_view_id` and `hash`. The person
sees the top view; *Back* returns to the one below with what was typed in it, and the corner
closes them all.

## App Home and shortcuts

### Home

An app's conversation with each person can have a **Home** tab beside *Messages*: a page of
blocks the app keeps for that person, such as their open requests or their settings. An admin
turns *Home tab* on on the app's page in *Apps*. While it is on, the conversation opens on
*Home*; an app that has not published one yet shows its name and description there.

`POST /views/publish` takes the bot token and publishes one person's Home:

```json
{
  "user_id": "c3d4…",
  "hash": "…",
  "view": {
    "type": "home",
    "callback_id": "helpdesk-home",
    "private_metadata": "…",
    "blocks": [
      { "type": "header", "text": { "type": "plain_text", "text": "Your requests" } },
      { "type": "actions", "block_id": "home", "elements": [
        { "type": "button", "action_id": "new_request", "text": { "type": "plain_text", "text": "New request" } }
      ] }
    ]
  }
}
```

A Home takes the message blocks, up to 100, and no `input`. Publishing again replaces it, and the
person's open tab changes without a reload. It answers with the Home, its `id` and a new `hash`;
sent with the `hash` it last got, a publish is refused with `409` when the Home has changed
since. Each person's Home is theirs: nobody else can read it. The route refuses a `user_id`
that is not a person in the workspace (`422`) and a Home tab that is off (`409`); the token of
an app that was turned off answers `401`, as everywhere.

An app that subscribes to `app_home_opened` hears when somebody opens the tab, with the Home it
last published for them, and can publish a fresh one. The buttons, menus and date pickers in a
Home reach the app, with *Interactivity* on, as a `block_actions` with
`"container": { "type": "view", "view_id": "<the Home's id>" }` and the `view` with its
`state`, and no `response_url`. Its `trigger_id` opens a modal.

### Shortcuts

An admin gives an app up to five shortcuts of each kind under *Shortcuts* on its page, each a
name, a description and a `callback_id` the app recognises it by. A shortcut needs
*Interactivity* on and reaches the app as a payload, like a button, within three seconds.

- A **global** shortcut is listed in the composer's shortcut menu, next to the workflows, and
  in the quick switcher. It sends

  ```json
  {
    "type": "shortcut",
    "app_id": "…", "workspace_id": "…",
    "callback_id": "new_request",
    "user": { "id": "…", "name": "Ana" },
    "channel": { "id": "…", "name": "desk" },
    "trigger_id": "…",
    "action_ts": "…"
  }
  ```

  with `channel` the one it was used in, or `null` from the quick switcher.

- A **message** shortcut is listed under *More actions* on every message in a channel, private
  channel or conversation the bot is a member of, and sends `"type": "message_action"` with the
  same fields, the `channel` the message is in, the `message` as in `block_actions`, and a
  `response_url`. Through that `response_url` the app answers like an element's, and changes
  only its own messages.

The app usually answers with a modal through `trigger_id`. A guest sees no app shortcuts.
Turning an app off takes its shortcuts and its Home tab away from everybody without a reload,
and nothing more is sent to it; turning off its interactivity takes away the shortcuts, and
turning off its Home tab takes away the tab.

## Link previews

An app can draw the previews of links to its own tools, such as an internal tracker or wiki,
which the instance cannot fetch itself.

- **Domains.** An admin lists up to five under *Link previews* on the app's page, and the app
  needs `links:read` and `links:write` for it.
  - A domain covers its subdomains: `example.com` claims `tracker.example.com` too. Where two
    apps' domains both cover a link, the longer one wins.
  - No two apps of a workspace claim the same domain (`409`).
  - A built-in preview's domain is refused while that integration is on (`409`): `github.com`,
    `atlassian.net`, `docs.google.com` and `drive.google.com`, `figma.com`. A domain claimed
    before the integration was turned on goes back to the integration while it is on.
- **The event.** A link to a claimed domain gets no page preview of its own.
  - When a message links there in a channel the bot is a member of, an app subscribed to
    `link_shared` is sent the message and those links.
  - Where the bot is not a member, the link gets no preview and the app hears nothing.
  - An edit that adds a link sends the new links only. A preview already given stays, and a
    link taken out of the message takes its preview with it.
- **The answer.** Within 30 minutes of the event, the app sends blocks for each link. They
  are drawn under the message, with the app's name and icon, for everybody in the channel and
  without a reload.
  - A preview takes `header`, `section`, `context`, `divider` and `image` blocks, and a
    section's accessory may be an image. Buttons and menus are refused (`422`).
  - Answering again replaces the preview.
- **Refused answers.**
  - A URL the event did not carry, or one edited out of the message since (`422`).
  - A message whose links were not shared with the app in the last 30 minutes (`403`).
  - A message whose author removed its previews (`409`). The app is not asked about that
    message again, edits included.
  - Any answer while the workspace's link previews are off (`409`); no `link_shared` is
    sent then either.

```json
{
  "type": "link_shared",
  "workspace_id": "1b2c…",
  "event": {
    "channel_id": "9d0e…",
    "message_id": "77aa…",
    "user_id": "c3d4…",
    "links": [{ "domain": "tracker.example.com", "url": "https://tracker.example.com/issues/7" }]
  }
}
```

```http
POST /api/v1/messages/77aa…/unfurls
Authorization: Bearer cbot-…

{
  "unfurls": {
    "https://tracker.example.com/issues/7": {
      "blocks": [
        { "type": "header", "text": { "type": "plain_text", "text": "TRK-7 Login fails on Safari" } },
        { "type": "section", "fields": [
          { "type": "mrkdwn", "text": "*Status*\nIn progress" },
          { "type": "mrkdwn", "text": "*Assignee*\nMarko" }
        ] }
      ]
    }
  }
}
```

## Workflow steps

An app can offer steps that people add to their workflows, next to the built-in
ones: open a ticket, look a customer up, page whoever is on call.

- **Definitions.** An admin lists them under *Workflow steps* on the app's page, or the app
  sends the whole list itself with `PUT /bot-apps/me/workflow-steps`. An app offers at most 20.
  - Each has a `callback_id` the app knows it by (unique within the app), a `title` of up to
    80 characters and an optional `description`.
  - `inputs` and `outputs` are up to 10 each: `{ name, title, kind, required }`. A `name` is
    lowercase letters, digits and `_`; `kind` is one of `text`, `number`, `boolean`,
    `person`, `channel`, `date`.
  - Sending the list again replaces it. A step keeps its id while its `callback_id` stays.
- **The builder.** The steps of an app that can run them are listed under the app's name.
  - That means the app is on, has `workflow.steps:execute`, is subscribed to
    `function_executed`, and has a request URL or socket mode.
  - Each input is a fixed value or a variable from an earlier step, of the input's kind:
    a person input takes *Person who ran it* or a form's person question, not a text answer.
  - The outputs become variables for the steps after it and for branches.
- **Running.** When a run gets to the step, the app is sent `function_executed` and the run
  waits for it, as it waits for a form.
  - `inputs` are typed: a number is a JSON number, a boolean `true` or `false`, a date
    `YYYY-MM-DD`, a person or a channel its id. An input left empty is `null`.
  - The app finishes the step with `POST /workflow-executions/{execution_id}/complete` and a
    value for each output, or fails it with `POST /workflow-executions/{execution_id}/fail`
    and a message of up to 500 characters. The run history shows that message, and the
    steps after it don't run.
  - An app that answers neither within an hour (`expires_at`) fails the step as *The app did
    not finish*.
- **Refused answers.**
  - An output the step doesn't declare, a required one missing, or one of the wrong kind
    (`422`): `outputs.ticket_id needs text`. A person must be in the workspace and a channel
    must be one of its channels. The step keeps waiting for a valid answer.
  - Another app's execution, or one that doesn't exist (`404`).
  - A step that already finished or ran out of time (`409`).
- **An app that goes away.** When an admin turns the app off or removes it, each workflow
  that uses its steps tells its owner once, in Activity. The builder warns on the step, and a
  run that gets there fails with the reason: the app was turned off or removed, lost the
  scope or the event, or no longer offers the step.

An *Open ticket* step that takes a title and hands back the ticket's id:

```http
PUT /api/v1/bot-apps/me/workflow-steps
Authorization: Bearer cbot-…

{
  "steps": [{
    "callback_id": "open_ticket",
    "title": "Open ticket",
    "description": "Files a ticket with the help desk",
    "inputs": [
      { "name": "title", "title": "Title", "kind": "text", "required": true },
      { "name": "reporter", "title": "Reporter", "kind": "person" }
    ],
    "outputs": [{ "name": "ticket_id", "title": "Ticket ID", "kind": "text", "required": true }]
  }]
}
```

A workflow runs it, then posts `Ticket {{step.1.ticket_id}} opened`. The app is sent:

```json
{
  "event_id": "5a1c…",
  "type": "function_executed",
  "workspace_id": "1b2c…",
  "event": {
    "execution_id": "e7f0…",
    "run_id": "a2b3…",
    "workflow": { "id": "0c9d…", "name": "Help desk" },
    "step": { "callback_id": "open_ticket", "title": "Open ticket", "position": 1 },
    "inputs": { "title": "Printer on 3 is out", "reporter": "c3d4…" },
    "expires_at": "2026-11-19T11:00:00Z"
  }
}
```

It opens the ticket and answers:

```http
POST /api/v1/workflow-executions/e7f0…/complete
Authorization: Bearer cbot-…

{ "outputs": { "ticket_id": "HD-42" } }
```

The run goes on and posts *Ticket HD-42 opened*. Had the help desk been down, the app would
have sent `POST /api/v1/workflow-executions/e7f0…/fail` with
`{ "error": "The help desk is down" }`.

## Example

[`examples/echo-bot`](../examples/echo-bot/) is a whole bot in one file: it answers
`echo <text>` in a thread. It needs `chat:write`, `channels:read` and `channels:history`. Its
socket-mode twin, `echo-bot-socket.mjs`, listens for `app_mention` instead of reading the
channels and answers the mention in a thread.

```js
const api = 'https://chat.example.com/api/v1';
const call = (method, path, body) =>
  fetch(`${api}${path}`, {
    method,
    headers: { Authorization: `Bearer ${process.env.CHAT_BOT_TOKEN}`, 'Content-Type': 'application/json' },
    body: body && JSON.stringify(body),
  }).then((r) => r.json());

const me = await call('GET', '/bot-apps/me');
const { data: channels } = await call('GET', `/workspaces/${me.workspace_id}/channels`);
for (const channel of channels) {
  const { data: messages } = await call('GET', `/channels/${channel.id}/messages?limit=50`);
  for (const m of messages) {
    const asked = /^echo\s+(.+)/is.exec(m.content);
    if (asked && m.user_id !== me.bot_user_id && m.reply_count === 0) {
      await call('POST', `/channels/${channel.id}/messages`, { content: asked[1], thread_parent_id: m.id });
    }
  }
}
```

[`examples/deploy-approval`](../examples/deploy-approval/) uses all of the above over a socket:
`/request` opens a modal, a button inside it pushes a second view, a bad service name comes
back as an error under its input, and the request is posted with *Approve* and *Reject*, which
the app answers through `response_url` with *Approved by Ana*:

```js
async function decide(payload) {
  const action = payload.actions[0];
  const verdict = action.action_id === 'approve' ? 'Approved' : 'Rejected';
  await fetch(payload.response_url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ replace_original: true, text: `${verdict} by ${payload.user.name}` }),
  });
}
```

[`examples/helpdesk`](../examples/helpdesk/) has a Home and both kinds of shortcut: it
publishes each person's own requests in their Home, opens a modal from the global *New request*
shortcut and the Home's button, and turns a message into a ticket from *Create ticket from
message*:

```js
if (frame.type === 'event' && frame.payload.type === 'app_home_opened') {
  await publishHome(frame.payload.event.user_id);
}
```

[`examples/tracker-previews`](../examples/tracker-previews/) previews links to
`tracker.example.com`: on `link_shared` it answers each issue link with a header and two
fields:

```js
await call(botToken, 'POST', `/messages/${event.message_id}/unfurls`, { unfurls });
```

[`examples/ticketing-step`](../examples/ticketing-step/) adds an *Open ticket*
[workflow step](#workflow-steps): it defines the step when it starts, and on
`function_executed` it opens the ticket and hands its id back:

```js
await call(botToken, 'POST', `/workflow-executions/${id}/complete`, { outputs: { ticket_id: ticketId } });
```

CI runs all six against a seeded instance on every pull request that runs the end-to-end
suite.
