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

Send the token as a bearer token. Every route is under `/api` on the instance:

```bash
curl -s https://chat.example.com/api/bot-apps/me \
  -H "Authorization: Bearer $CHAT_BOT_TOKEN"
```

`GET /bot-apps/me` answers with the app: its `id`, `workspace_id`, `bot_user_id` and
`scopes`. It is the one route that needs no scope, so it is how a bot learns who it is.

A bot token works only on the routes below. Anything else answers `403 This route does not
accept a bot token`, and a token the server does not know answers `401`. The OpenAPI document
at `/api/openapi.json` tags these routes `bot` and lists each one's scope under the `bot`
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
| `users:read` | list the workspace's members |
| `commands` | own slash commands |

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
| POST | `/channels/{ch_id}/messages` | `chat:write` | `{ content, thread_parent_id?, attachment_ids?, client_message_id? }` |
| PATCH | `/messages/{msg_id}` | `chat:write` | `{ content }`, its own messages only |
| DELETE | `/messages/{msg_id}` | `chat:write` | its own messages only |
| POST | `/messages/{msg_id}/reactions` | `reactions:write` | `{ emoji }` |
| DELETE | `/messages/{msg_id}/reactions/{emoji}` | `reactions:write` | |
| POST | `/files/upload/{ws_id}?draft=true` | `files:write` | multipart `file` fields; post the returned ids as `attachment_ids` |
| GET | `/workspaces/{ws_id}/members` | `users:read` | `is_bot` marks other apps |
| POST | `/workspaces/{ws_id}/conversations` | `im:write` | `{ participant_ids }`; one person opens or reuses a DM |

`client_message_id` is a version 4 UUID the bot picks. Sending the same one twice posts once,
so a bot that retries after a timeout does not double-post.

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
curl -s -X POST https://chat.example.com/api/apps/connections.open \
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
               "user_id": "…", "request_id": "…", "response_url": "https://…" } }
```

to be acknowledged like an event and answered through `response_url`. The person who typed it
sees *working on it…* until then, or *could not be reached* straight away when the app has no
socket open. An invocation nobody acknowledges within a minute is dropped.

## Example

[`examples/echo-bot`](../examples/echo-bot/) is a whole bot in one file: it answers
`echo <text>` in a thread. It needs `chat:write`, `channels:read` and `channels:history`. Its
socket-mode twin, `echo-bot-socket.mjs`, listens for `app_mention` instead of reading the
channels and answers the mention in a thread.

```js
const api = 'https://chat.example.com/api';
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

CI runs both against a seeded instance on every pull request that runs the end-to-end suite.
