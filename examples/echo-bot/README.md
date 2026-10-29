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
