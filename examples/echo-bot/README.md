# Echo bot

A bot that answers `echo <text>` in a thread with `<text>`. It is one file, runs on Node 22
with no dependencies, and uses only the [bot API](../../docs/bot-api.md).

1. In *Workspace settings → Apps*, create an app with the scopes `chat:write`,
   `channels:read` and `channels:history`, and copy its token.
2. Add the bot to a channel: `/invite @Echo` there, or *Channel details → Members*.
3. Run it:

   ```bash
   CHAT_API_URL=https://chat.example.com/api CHAT_BOT_TOKEN=cbot-… node echo-bot.mjs
   ```

It polls the channels it is in every three seconds and answers messages posted after it
started. `--once` answers every unanswered `echo` it can see and exits; CI runs it that way
against a seeded instance.

## Socket mode

`echo-bot-socket.mjs` answers a mention instead, and needs no public URL: it opens a socket to
the instance and takes the app's events there.

1. Create an app with `chat:write` and `channels:history`. Open its events (the radio icon),
   tick `app_mention` and *Socket mode*, *Save events*, then *Make an app-level token* and
   copy it.
2. Add the bot to a channel, then run:

   ```bash
   CHAT_API_URL=https://chat.example.com/api CHAT_BOT_TOKEN=cbot-… CHAT_APP_TOKEN=capp-… node echo-bot-socket.mjs
   ```

Mention it with some text and it answers that text in the thread. It acknowledges each event
as it arrives, answers slash commands it owns through their `response_url`, and opens a new
socket when the instance closes one. `--once` exits after the first answer.
