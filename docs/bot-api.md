# Bot API

An **app** is something a workspace builds for itself: a deploy bot, a stand-up reminder, a
bridge to another system. It has a **bot user** with its own name and token, and it calls the
same REST API the web client uses. This page is for whoever writes one.

## Make an app

A workspace admin makes it in *Workspace settings → Apps → Built for this workspace*: a name,
an optional description, and the scopes it needs. The token is shown once, when the app is
made or given a new one; copy it then. It looks like `cbot-` and 43 more characters, and the
server keeps only its SHA-256.

The bot user appears in the member list with an *APP* label. It cannot sign in, has no
password or address, and is not counted among the instance's accounts.

*New token* replaces the token at once: the old one stops working on the next request.
*Remove* revokes the token, removes the app's slash commands and webhooks, and takes the bot
out of the workspace. What it posted stays.

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

## A slash command of the app's own

A command is registered as a `slash_command` hook (see *Slash commands* in
[backend.md](./backend.md#hooks)) with `app_id` set to the app, which needs the `commands`
scope. The command is removed with the app. Its requests are signed and may be answered later
through `response_url`, as for any command.

## Example

[`examples/echo-bot`](../examples/echo-bot/) is a whole bot in one file: it answers
`echo <text>` in a thread. It needs `chat:write`, `channels:read` and `channels:history`.

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

CI runs it against a seeded instance on every pull request that runs the end-to-end suite.
